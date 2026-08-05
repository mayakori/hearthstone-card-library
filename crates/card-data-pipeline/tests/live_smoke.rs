#[cfg(feature = "test-support")]
use std::{
    fs,
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use card_data_contract::{DataVersion, SCHEMA_VERSION};
#[cfg(feature = "test-support")]
use card_data_pipeline::{
    run_build_with_test_client, BlizzardClient, BuildResult, Clock, Event, EventSink, HttpPolicy,
    PipelineError, Sleeper,
};
use card_data_pipeline::{validate_package_directory, BuildRequest, Credentials, VecEventSink};
#[cfg(feature = "test-support")]
use secrecy::SecretString;
#[cfg(feature = "test-support")]
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, Request, Respond, ResponseTemplate,
};

const DATA_VERSION: &str = "36.0.3-build247416-r1";
#[cfg(feature = "test-support")]
const CLIENT_ID: &str = "offline-e2e-client-id";
#[cfg(feature = "test-support")]
const CLIENT_SECRET: &str = "offline-e2e-client-secret";
#[cfg(feature = "test-support")]
const TOKEN: &str = "offline-e2e-token";

#[cfg(feature = "test-support")]
struct NoopSleeper;

#[cfg(feature = "test-support")]
#[async_trait::async_trait]
impl Sleeper for NoopSleeper {
    async fn sleep(&self, _: Duration) {}
}

#[cfg(feature = "test-support")]
struct FixedClock(SystemTime);

#[cfg(feature = "test-support")]
impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

#[cfg(feature = "test-support")]
struct FailingEventSink {
    fail_stage: &'static str,
    fail_event: &'static str,
    events: Vec<Event>,
}

#[cfg(feature = "test-support")]
impl EventSink for FailingEventSink {
    fn emit(&mut self, event: Event) -> io::Result<()> {
        if event.stage == self.fail_stage && event.event == self.fail_event {
            return Err(io::Error::other("injected event sink failure"));
        }
        self.events.push(event);
        Ok(())
    }
}

#[cfg(feature = "test-support")]
struct FixtureApi {
    first_response_is_retry: Mutex<bool>,
}

#[cfg(feature = "test-support")]
impl Respond for FixtureApi {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let mut first = self.first_response_is_retry.lock().expect("retry lock");
        if *first {
            *first = false;
            return ResponseTemplate::new(503);
        }
        drop(first);

        let locale = request
            .url
            .query_pairs()
            .find_map(|(key, value)| (key == "locale").then_some(value.into_owned()))
            .expect("fixture requests include locale");
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/fixtures/card-data-pipeline/v1")
            .join(locale);
        let response = match request.url.path() {
            "/hearthstone/cards" => fixture_root.join("cards-page-1.json"),
            "/hearthstone/metadata" => fixture_root.join("metadata.json"),
            route => fixture_root.join("cards").join(format!(
                "{}.json",
                route
                    .strip_prefix("/hearthstone/cards/")
                    .expect("only known card routes")
            )),
        };
        ResponseTemplate::new(200).set_body_json(
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(response).expect("fixture response exists"),
            )
            .expect("fixture response is JSON"),
        )
    }
}

#[cfg(feature = "test-support")]
fn credentials() -> Credentials {
    Credentials {
        client_id: SecretString::from(CLIENT_ID),
        client_secret: SecretString::from(CLIENT_SECRET),
    }
}

#[cfg(feature = "test-support")]
#[tokio::test]
async fn offline_full_pipeline_packages_fixture_data_through_the_production_orchestration() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": TOKEN,
            "expires_in": 3600,
            "token_type": "bearer"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(FixtureApi {
            first_response_is_retry: Mutex::new(true),
        })
        .mount(&server)
        .await;

    let first_output = tempfile::tempdir().expect("first temporary output root");
    let second_output = tempfile::tempdir().expect("second temporary output root");
    let (first, first_events) = run_fixture_build(&server, first_output.path()).await;
    let (second, second_events) = run_fixture_build(&server, second_output.path()).await;

    for result in [&first, &second] {
        assert_eq!(result.manifest.schema_version, SCHEMA_VERSION);
        result.manifest.validate().expect("manifest is self-valid");
        validate_package_directory(&result.version_directory, &result.manifest)
            .expect("published package self-validates");
        assert_eq!(
            files(&result.version_directory),
            [
                "manifest.json",
                "normalized/en_US.sqlite.zst",
                "normalized/ko_KR.sqlite.zst",
                "raw/en_US.json.zst",
                "raw/ko_KR.json.zst",
            ]
        );
        for locale in ["ko_KR", "en_US"] {
            let raw = decompress(
                &result
                    .version_directory
                    .join(format!("raw/{locale}.json.zst")),
            );
            let raw: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            assert_eq!(raw["collected_at"], "2026-08-05T00:00:00Z");
            assert_sqlite_is_valid(
                &result
                    .version_directory
                    .join(format!("normalized/{locale}.sqlite.zst")),
            );
        }
    }
    assert_no_staging(first_output.path());
    assert_no_staging(second_output.path());
    for asset in [
        "raw/en_US.json.zst",
        "raw/ko_KR.json.zst",
        "normalized/en_US.sqlite.zst",
        "normalized/ko_KR.sqlite.zst",
        "manifest.json",
    ] {
        assert_eq!(
            fs::read(first.version_directory.join(asset)).unwrap(),
            fs::read(second.version_directory.join(asset)).unwrap(),
            "compressed/package bytes differ for {asset}"
        );
    }
    for asset in [
        "raw/en_US.json.zst",
        "raw/ko_KR.json.zst",
        "normalized/en_US.sqlite.zst",
        "normalized/ko_KR.sqlite.zst",
    ] {
        assert_eq!(
            decompress(&first.version_directory.join(asset)),
            decompress(&second.version_directory.join(asset)),
            "uncompressed bytes differ for {asset}"
        );
    }
    assert_event_contract(&first_events, true);
    assert_event_contract(&second_events, false);
}

#[cfg(feature = "test-support")]
async fn run_fixture_build(server: &MockServer, output_root: &Path) -> (BuildResult, VecEventSink) {
    let request = fixture_request(output_root);
    let client = BlizzardClient::for_test_with_clock(
        credentials(),
        HttpPolicy::default(),
        server.uri(),
        Arc::new(NoopSleeper),
        Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(1_785_888_000))),
    );
    let mut events = VecEventSink::default();
    let result = run_build_with_test_client(request, client, &mut events)
        .await
        .expect("offline fixture package");
    assert_eq!(result.manifest.generated_at, "2026-08-05T00:00:00Z");
    (result, events)
}

#[cfg(feature = "test-support")]
fn assert_no_staging(output_root: &Path) {
    let mut output_entries = fs::read_dir(output_root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    output_entries.sort();
    assert_eq!(
        output_entries,
        vec![DATA_VERSION],
        "successful package leaves no staging directory"
    );
}

#[cfg(feature = "test-support")]
fn decompress(path: &Path) -> Vec<u8> {
    zstd::stream::decode_all(fs::File::open(path).unwrap()).expect("decompress package asset")
}

#[cfg(feature = "test-support")]
fn assert_event_contract(events: &VecEventSink, has_retry: bool) {
    let mut expected = vec![("collect", "started", None)];
    if has_retry {
        expected.push(("collect", "retry", None));
    }
    expected.extend([
        ("collect", "completed", None),
        ("normalize", "started", None),
        ("normalize", "completed", None),
        ("locale", "summary", Some("ko_KR")),
        ("normalize", "started", None),
        ("normalize", "completed", None),
        ("locale", "summary", Some("en_US")),
        ("package", "started", None),
        ("package", "completed", None),
        ("final", "success", None),
    ]);
    assert_eq!(
        events
            .events
            .iter()
            .map(|event| (event.stage, event.event, event.locale.as_deref()))
            .collect::<Vec<_>>(),
        expected
    );
    if has_retry {
        let retry = events
            .events
            .iter()
            .find(|event| event.event == "retry")
            .unwrap();
        assert_eq!(retry.attempt, Some(1));
        assert_eq!(retry.status_code, Some(503));
    }
}

#[cfg(feature = "test-support")]
#[tokio::test]
async fn published_package_remains_successful_when_post_publish_logging_fails() {
    for (fail_stage, fail_event) in [("package", "completed"), ("final", "success")] {
        let server = MockServer::start().await;
        mount_fixture_api(&server).await;
        let output = tempfile::tempdir().expect("temporary output root");
        let request = fixture_request(output.path());
        let mut events = FailingEventSink {
            fail_stage,
            fail_event,
            events: Vec::new(),
        };

        let result =
            run_build_with_test_client(request, fixture_client(&server), &mut events).await;
        let published = output.path().join(DATA_VERSION);

        assert!(published.is_dir(), "package was atomically published");
        let result = result.expect("post-publish logging must not reverse build success");
        validate_package_directory(&result.version_directory, &result.manifest)
            .expect("published package remains self-valid");
    }
}

#[cfg(feature = "test-support")]
#[tokio::test]
async fn pre_publish_logging_failure_leaves_no_package_or_staging_directory() {
    let server = MockServer::start().await;
    mount_fixture_api(&server).await;
    let output = tempfile::tempdir().expect("temporary output root");
    let request = fixture_request(output.path());
    let mut events = FailingEventSink {
        fail_stage: "package",
        fail_event: "started",
        events: Vec::new(),
    };

    let result = run_build_with_test_client(request, fixture_client(&server), &mut events).await;

    assert!(matches!(result, Err(PipelineError::Io(_))));
    assert!(
        fs::read_dir(output.path()).unwrap().next().is_none(),
        "pre-publish logging failure must leave no final or staging directory"
    );
}

#[cfg(feature = "test-support")]
async fn mount_fixture_api(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": TOKEN,
            "expires_in": 3600,
            "token_type": "bearer"
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .respond_with(FixtureApi {
            first_response_is_retry: Mutex::new(false),
        })
        .mount(server)
        .await;
}

#[cfg(feature = "test-support")]
fn fixture_request(output_root: &Path) -> BuildRequest {
    BuildRequest {
        data_version: DataVersion::parse(DATA_VERSION).unwrap(),
        output_root: output_root.to_path_buf(),
        credentials: credentials(),
    }
}

#[cfg(feature = "test-support")]
fn fixture_client(server: &MockServer) -> BlizzardClient {
    BlizzardClient::for_test(
        credentials(),
        HttpPolicy::default(),
        server.uri(),
        Arc::new(NoopSleeper),
    )
}

#[tokio::test]
#[ignore = "requires Blizzard credentials and network"]
async fn builds_current_standard_package_for_both_locales() {
    let credentials = Credentials::from_env().expect("Blizzard credentials");
    let output = tempfile::tempdir().expect("temporary output root");
    let request = BuildRequest {
        data_version: DataVersion::parse(DATA_VERSION).unwrap(),
        output_root: output.path().to_path_buf(),
        credentials,
    };
    let mut events = VecEventSink::default();
    let result = card_data_pipeline::run_build(request, &mut events)
        .await
        .expect("live package");

    assert_eq!(result.manifest.schema_version, SCHEMA_VERSION);
    assert_eq!(
        result.manifest.supported_locales,
        vec!["ko_KR".to_owned(), "en_US".to_owned()],
    );
    validate_package_directory(&result.version_directory, &result.manifest).unwrap();
    drop(result);
    output.close().expect("remove live-smoke output");
}

#[cfg(feature = "test-support")]
fn files(root: &Path) -> Vec<String> {
    let mut result = files_from(root, root);
    result.sort();
    result
}

#[cfg(feature = "test-support")]
fn files_from(root: &Path, directory: &Path) -> Vec<String> {
    let mut result = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_dir() {
            result.extend(files_from(root, &entry.path()));
        } else {
            result.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    result
}

#[cfg(feature = "test-support")]
fn assert_sqlite_is_valid(compressed: &Path) {
    let mut database = tempfile::NamedTempFile::new().expect("temporary SQLite");
    database
        .write_all(
            &zstd::stream::decode_all(fs::File::open(compressed).unwrap())
                .expect("decompress SQLite"),
        )
        .expect("write SQLite");
    let connection = rusqlite::Connection::open(database.path()).expect("open SQLite");
    let mut foreign_keys = connection.prepare("PRAGMA foreign_key_check").unwrap();
    assert!(foreign_keys.query([]).unwrap().next().unwrap().is_none());
    assert_eq!(
        connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT banned_from_sideboard FROM cards WHERE id = 1006",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1,
        "current official boolean is normalized into SQLite"
    );
}
