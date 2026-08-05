use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use card_data_pipeline::{
    BlizzardClient, Clock, Collector, Credentials, HttpPolicy, PipelineError, RetryEvent, Sleeper,
};
use secrecy::SecretString;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, Request, Respond, ResponseTemplate,
};

const CLIENT_ID: &str = "fixture-client-id";
const CLIENT_SECRET: &str = "fixture-client-secret";
const TOKEN: &str = "fixture-access-token";

struct RecordedSleeper(Mutex<Vec<Duration>>);

#[async_trait::async_trait]
impl Sleeper for RecordedSleeper {
    async fn sleep(&self, delay: Duration) {
        self.0.lock().expect("sleeper lock").push(delay);
    }
}

struct Sequence {
    responses: Mutex<Vec<ResponseTemplate>>,
}

impl Respond for Sequence {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        self.responses.lock().expect("sequence lock").remove(0)
    }
}

#[derive(Clone, Copy)]
enum FixtureMutation {
    None,
    PageDrift,
    MissingChild,
    ResponseIdMismatch,
    LocaleIdMismatch,
    SmallerRelatedId,
    RelatedClassOverlap,
    TaxonomyReferenceMismatch,
    TaxonomyExistingReferenceMismatch,
    MissingSideboardField,
    ClassReferenceForwardClosure,
    ClassReferenceAlternateHero,
}

struct FixtureApi {
    mutation: FixtureMutation,
}

struct OrderingFixtureApi {
    list_locales: Arc<Mutex<Vec<String>>>,
}

impl Respond for OrderingFixtureApi {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if request.url.path() == "/hearthstone/cards" {
            let locale = request
                .url
                .query_pairs()
                .find_map(|(key, value)| (key == "locale").then_some(value.into_owned()))
                .expect("list request locale");
            self.list_locales.lock().unwrap().push(locale);
        }
        FixtureApi {
            mutation: FixtureMutation::None,
        }
        .respond(request)
    }
}

struct OrderingClock {
    list_locales: Arc<Mutex<Vec<String>>>,
    calls: Mutex<usize>,
}

impl Clock for OrderingClock {
    fn now(&self) -> SystemTime {
        let mut calls = self.calls.lock().unwrap();
        let call = *calls;
        *calls += 1;
        let locales = self.list_locales.lock().unwrap();
        match call {
            0 => assert!(
                locales.is_empty(),
                "ko_KR timestamp must precede its list request"
            ),
            1 => assert_eq!(
                locales.as_slice(),
                ["ko_KR"],
                "en_US timestamp must precede its list request"
            ),
            _ => panic!("collector requested an unexpected timestamp"),
        }
        UNIX_EPOCH + Duration::from_secs(1_785_888_000 + 60 * call as u64)
    }
}

impl Respond for FixtureApi {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let locale = request
            .url
            .query_pairs()
            .find_map(|(key, value)| (key == "locale").then_some(value.into_owned()))
            .unwrap_or_else(|| "ko_KR".into());
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/fixtures/card-data-pipeline/v1")
            .join(&locale);
        let path = request.url.path();
        let file = if path == "/hearthstone/cards" {
            root.join("cards-page-1.json")
        } else if path == "/hearthstone/metadata" {
            root.join("metadata.json")
        } else if (path == "/hearthstone/cards/3003"
            && matches!(
                self.mutation,
                FixtureMutation::ClassReferenceForwardClosure
                    | FixtureMutation::ClassReferenceAlternateHero
            ))
            || (path == "/hearthstone/cards/1000"
                && matches!(self.mutation, FixtureMutation::SmallerRelatedId))
        {
            root.join("cards/2003.json")
        } else if let Some(id) = path.strip_prefix("/hearthstone/cards/") {
            root.join("cards").join(format!("{id}.json"))
        } else {
            return ResponseTemplate::new(404);
        };
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(file).expect("fixture response must exist"))
                .expect("fixture response must be JSON");
        match self.mutation {
            FixtureMutation::PageDrift if locale == "ko_KR" && path == "/hearthstone/cards" => {
                value["page"] = serde_json::json!(2)
            }
            FixtureMutation::ResponseIdMismatch if path == "/hearthstone/cards/2001" => {
                value["id"] = serde_json::json!(2002)
            }
            FixtureMutation::LocaleIdMismatch
                if locale == "en_US" && path == "/hearthstone/cards" =>
            {
                value["cards"][0]["id"] = serde_json::json!(1010)
            }
            FixtureMutation::MissingChild if path == "/hearthstone/cards/2003" => {
                return ResponseTemplate::new(404)
            }
            FixtureMutation::SmallerRelatedId if path == "/hearthstone/cards/2001" => {
                value["childIds"] = serde_json::json!([2003, 1000]);
            }
            FixtureMutation::SmallerRelatedId if path == "/hearthstone/cards/1000" => {
                value["id"] = serde_json::json!(1000);
            }
            FixtureMutation::RelatedClassOverlap if path == "/hearthstone/metadata" => {
                value["classes"][0]["cardId"] = serde_json::json!(2001);
            }
            FixtureMutation::TaxonomyReferenceMismatch
                if locale == "en_US" && path == "/hearthstone/cards" =>
            {
                value["cards"][0]["cardSetId"] = serde_json::json!(999);
            }
            FixtureMutation::TaxonomyExistingReferenceMismatch
                if locale == "en_US" && path == "/hearthstone/cards" =>
            {
                value["cards"][6]["multiTypeIds"] = serde_json::json!([20]);
            }
            FixtureMutation::MissingSideboardField if path == "/hearthstone/cards" => {
                value["cards"][5]["sideboard"]
                    .as_object_mut()
                    .unwrap()
                    .remove("maxSideboardCards");
            }
            FixtureMutation::ClassReferenceAlternateHero if path == "/hearthstone/metadata" => {
                value["classes"][0]["alternateHeroCardIds"] = serde_json::json!([3003]);
            }
            FixtureMutation::ClassReferenceForwardClosure
            | FixtureMutation::ClassReferenceAlternateHero
                if path == "/hearthstone/cards/3001" =>
            {
                value["childIds"] = serde_json::json!([3003]);
            }
            FixtureMutation::ClassReferenceForwardClosure
            | FixtureMutation::ClassReferenceAlternateHero
                if path == "/hearthstone/cards/3003" =>
            {
                value["id"] = serde_json::json!(3003);
            }
            _ => {}
        }
        ResponseTemplate::new(200).set_body_json(value)
    }
}

fn start_truncated_body_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    thread::spawn(move || {
        let response = |body: &str| {
            format!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        };
        let responses = vec![
            response("{\"access_token\":\"token\",\"expires_in\":3600}"),
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 40\r\n\r\n{}".into(),
            response("{\"cards\":[],\"cardCount\":0,\"pageCount\":1,\"page\":1}"),
        ];
        for response in responses {
            let (mut stream, _) = listener.accept().expect("accept test request");
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).expect("read test request");
            stream
                .write_all(response.as_bytes())
                .expect("write test response");
        }
    });
    format!("http://{address}")
}

fn credentials() -> Credentials {
    Credentials {
        client_id: SecretString::from(CLIENT_ID),
        client_secret: SecretString::from(CLIENT_SECRET),
    }
}

fn list_response() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "cards": [], "cardCount": 0, "pageCount": 1, "page": 1
    }))
}

fn assert_exponential_backoff(delays: &[Duration]) {
    assert_eq!(delays.len(), 3);
    for (delay, seconds) in delays.iter().zip([1, 2, 4]) {
        assert!(*delay >= Duration::from_secs(seconds));
        assert!(*delay <= Duration::from_secs(seconds) + Duration::from_millis(250));
    }
}

async fn client(server: &MockServer, sleeper: Arc<RecordedSleeper>) -> BlizzardClient {
    BlizzardClient::for_test(credentials(), HttpPolicy::default(), server.uri(), sleeper)
}

async fn mount_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": TOKEN,
            "expires_in": 3600,
            "token_type": "bearer"
        })))
        .mount(server)
        .await;
}

async fn fixture_collector(server: &MockServer, mutation: FixtureMutation) -> Collector {
    mount_token(server).await;
    Mock::given(method("GET"))
        .respond_with(FixtureApi { mutation })
        .mount(server)
        .await;
    Collector::new(client(server, Arc::new(RecordedSleeper(Mutex::new(vec![])))).await)
}

#[tokio::test]
async fn token_success_then_list_success_keeps_secrets_out_of_errors() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(list_response())
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = client(&server, sleeper.clone()).await;

    client
        .fetch_cards_page("ko_KR", 1)
        .await
        .expect("list succeeds");
    assert!(sleeper.0.lock().expect("sleeper lock").is_empty());
}

#[tokio::test]
async fn one_401_forces_one_new_token_then_repeats_the_same_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(Sequence {
            responses: Mutex::new(vec![
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token":"first-token","expires_in":3600}),
                ),
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"access_token":"second-token","expires_in":3600}),
                ),
            ]),
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(Sequence {
            responses: Mutex::new(vec![ResponseTemplate::new(401), list_response()]),
        })
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = client(&server, sleeper).await;

    client
        .fetch_cards_page("ko_KR", 1)
        .await
        .expect("refresh succeeds");
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn second_401_after_refresh_is_an_auth_error() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = client(&server, sleeper).await;

    assert!(matches!(
        client.fetch_cards_page("ko_KR", 1).await,
        Err(PipelineError::Auth(_))
    ));
}

#[tokio::test]
async fn retry_after_is_preferred_for_a_429() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(Sequence {
            responses: Mutex::new(vec![
                ResponseTemplate::new(429).insert_header("Retry-After", "3"),
                list_response(),
            ]),
        })
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = client(&server, sleeper.clone()).await;

    client.fetch_cards_page("ko_KR", 1).await.unwrap();
    assert_eq!(*sleeper.0.lock().unwrap(), vec![Duration::from_secs(3)]);
}

#[tokio::test]
async fn retry_observer_exposes_only_attempt_and_status_scalars() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(Sequence {
            responses: Mutex::new(vec![ResponseTemplate::new(429), list_response()]),
        })
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = client(&server, sleeper).await;

    client.fetch_cards_page("ko_KR", 1).await.unwrap();

    assert_eq!(
        client.take_retry_events(),
        vec![RetryEvent {
            attempt: 1,
            status_code: Some(429),
        }]
    );
}

#[tokio::test]
async fn http_date_retry_after_is_preferred_for_a_429() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    let retry_at = httpdate::fmt_http_date(std::time::SystemTime::now() + Duration::from_secs(5));
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(Sequence {
            responses: Mutex::new(vec![
                ResponseTemplate::new(429).insert_header("Retry-After", retry_at),
                list_response(),
            ]),
        })
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = client(&server, sleeper.clone()).await;

    client.fetch_cards_page("ko_KR", 1).await.unwrap();
    let delay = sleeper.0.lock().unwrap()[0];
    assert!(
        (3..=5).contains(&delay.as_secs()),
        "unexpected delay {delay:?}"
    );
}

#[tokio::test]
async fn four_retryable_server_failures_exhaust_three_retries() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(Sequence {
            responses: Mutex::new(vec![
                ResponseTemplate::new(500),
                ResponseTemplate::new(502),
                ResponseTemplate::new(503),
                ResponseTemplate::new(504),
            ]),
        })
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = client(&server, sleeper.clone()).await;

    assert!(matches!(
        client.fetch_cards_page("ko_KR", 1).await,
        Err(PipelineError::Network(_))
    ));
    assert_exponential_backoff(&sleeper.0.lock().unwrap());
}

#[tokio::test]
async fn read_timeouts_retry_three_times_before_a_network_error() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    Mock::given(method("GET"))
        .and(path("/hearthstone/cards"))
        .respond_with(Sequence {
            responses: Mutex::new(vec![
                list_response(),
                ResponseTemplate::new(200).set_delay(Duration::from_millis(100)),
                ResponseTemplate::new(200).set_delay(Duration::from_millis(100)),
                ResponseTemplate::new(200).set_delay(Duration::from_millis(100)),
                ResponseTemplate::new(200).set_delay(Duration::from_millis(100)),
            ]),
        })
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let policy = HttpPolicy {
        request_timeout: Duration::from_millis(50),
        ..HttpPolicy::default()
    };
    let mut client = BlizzardClient::for_test(credentials(), policy, server.uri(), sleeper.clone());
    client
        .fetch_cards_page("ko_KR", 1)
        .await
        .expect("cache token");

    assert!(matches!(
        client.fetch_cards_page("ko_KR", 1).await,
        Err(PipelineError::Network(_))
    ));
    assert_exponential_backoff(&sleeper.0.lock().unwrap());
}

#[tokio::test]
async fn post_header_body_read_error_retries_before_successful_json_is_parsed() {
    let base_url = start_truncated_body_server();
    let sleeper = Arc::new(RecordedSleeper(Mutex::new(vec![])));
    let mut client = BlizzardClient::for_test(
        credentials(),
        HttpPolicy::default(),
        base_url,
        sleeper.clone(),
    );

    client
        .fetch_cards_page("ko_KR", 1)
        .await
        .expect("retry after a truncated successful body");
    let delays = sleeper.0.lock().unwrap();
    assert_eq!(delays.len(), 1);
    assert!((Duration::from_secs(1)..=Duration::from_millis(1250)).contains(&delays[0]));
}

#[tokio::test]
async fn collection_closes_relations_and_applies_scope_precedence() {
    let server = MockServer::start().await;
    let mut collector = fixture_collector(&server, FixtureMutation::None).await;

    let locales = collector.collect_all().await.expect("fixture collection");
    for locale in [&locales.ko_kr, &locales.en_us] {
        assert_eq!(locale.standard_cards.len(), 7);
        assert_eq!(locale.related_cards.len(), 3);
        assert_eq!(locale.class_reference_cards.len(), 2);
        assert!(locale.related_cards.contains_key(&2001));
        assert!(locale.related_cards.contains_key(&2002));
        assert!(locale.related_cards.contains_key(&2003));
        assert!(locale.class_reference_cards.contains_key(&3001));
        assert!(locale.class_reference_cards.contains_key(&3002));
        locale.raw.validate().expect("safe canonical raw");
    }
}

#[tokio::test]
async fn raw_related_wrappers_are_sorted_when_a_smaller_id_is_discovered_late() {
    let server = MockServer::start().await;
    let mut collector = fixture_collector(&server, FixtureMutation::SmallerRelatedId).await;

    let locales = collector.collect_all().await.expect("fixture collection");
    for locale in [&locales.ko_kr, &locales.en_us] {
        assert_eq!(
            locale
                .raw
                .related_cards
                .iter()
                .map(|entry| entry.requested_card_id)
                .collect::<Vec<_>>(),
            vec![1000, 2001, 2002, 2003]
        );
        locale.raw.validate().expect("sorted related wrappers");
    }
}

#[tokio::test]
async fn raw_wrapper_partition_follows_final_class_reference_scope() {
    let server = MockServer::start().await;
    let mut collector = fixture_collector(&server, FixtureMutation::RelatedClassOverlap).await;

    let locales = collector.collect_all().await.expect("fixture collection");
    for locale in [&locales.ko_kr, &locales.en_us] {
        assert!(locale.class_reference_cards.contains_key(&2001));
        assert!(!locale.related_cards.contains_key(&2001));
        assert_eq!(
            locale
                .raw
                .related_cards
                .iter()
                .map(|entry| entry.requested_card_id)
                .collect::<Vec<_>>(),
            vec![2002, 2003]
        );
        assert_eq!(
            locale
                .raw
                .class_reference_cards
                .iter()
                .map(|entry| entry.requested_card_id)
                .collect::<Vec<_>>(),
            vec![2001, 3002]
        );
    }
}

#[tokio::test]
async fn collection_rejects_taxonomy_reference_parity_mismatch() {
    let server = MockServer::start().await;
    let mut collector =
        fixture_collector(&server, FixtureMutation::TaxonomyReferenceMismatch).await;

    assert!(matches!(
        collector.collect_all().await,
        Err(PipelineError::ApiStructure(_))
    ));
}

#[tokio::test]
async fn collection_rejects_taxonomy_reference_mismatch_when_both_ids_are_metadata_defined() {
    let server = MockServer::start().await;
    let mut collector =
        fixture_collector(&server, FixtureMutation::TaxonomyExistingReferenceMismatch).await;

    assert!(matches!(
        collector.collect_all().await,
        Err(PipelineError::ApiStructure(_))
    ));
}

#[tokio::test]
async fn collection_rejects_structure_and_locale_parity_mutations() {
    for mutation in [
        FixtureMutation::PageDrift,
        FixtureMutation::MissingChild,
        FixtureMutation::ResponseIdMismatch,
        FixtureMutation::LocaleIdMismatch,
    ] {
        let server = MockServer::start().await;
        let mut collector = fixture_collector(&server, mutation).await;
        assert!(matches!(
            collector.collect_all().await,
            Err(PipelineError::ApiStructure(_))
        ));
    }
}

#[tokio::test]
async fn collection_reports_a_secret_safe_path_for_card_schema_errors() {
    let server = MockServer::start().await;
    let mut collector = fixture_collector(&server, FixtureMutation::MissingSideboardField).await;

    let error = match collector.collect_all().await {
        Ok(_) => panic!("invalid sideboard must fail"),
        Err(error) => error,
    };
    let PipelineError::ApiStructure(message) = error else {
        panic!("expected API structure error");
    };
    assert_eq!(
        message,
        "card list response schema error at cards[5].sideboard.maxSideboardCards: missing_field"
    );
    assert!(!message.contains("fixture"));
    assert!(!message.contains("https://"));
}

#[tokio::test]
async fn collection_does_not_collect_alternate_heroes_from_class_reference_relations() {
    let server = MockServer::start().await;
    let mut collector =
        fixture_collector(&server, FixtureMutation::ClassReferenceAlternateHero).await;

    let locales = collector.collect_all().await.expect("fixture collection");
    for locale in [&locales.ko_kr, &locales.en_us] {
        assert!(locale.class_reference_cards.contains_key(&3001));
        assert!(!locale.related_cards.contains_key(&3003));
        assert_eq!(locale.class_reference_cards[&3001].child_ids, vec![3003]);
    }
}

#[tokio::test]
async fn collection_keeps_non_skin_gameplay_relations_from_class_references() {
    let server = MockServer::start().await;
    let mut collector =
        fixture_collector(&server, FixtureMutation::ClassReferenceForwardClosure).await;

    let locales = collector.collect_all().await.expect("fixture collection");
    for locale in [&locales.ko_kr, &locales.en_us] {
        assert!(locale.class_reference_cards.contains_key(&3001));
        assert!(locale.related_cards.contains_key(&3003));
    }
}

#[tokio::test]
async fn locale_timestamp_is_captured_immediately_before_each_first_list_request() {
    let server = MockServer::start().await;
    mount_token(&server).await;
    let list_locales = Arc::new(Mutex::new(Vec::new()));
    Mock::given(method("GET"))
        .respond_with(OrderingFixtureApi {
            list_locales: list_locales.clone(),
        })
        .mount(&server)
        .await;
    let clock = Arc::new(OrderingClock {
        list_locales,
        calls: Mutex::new(0),
    });
    let client = BlizzardClient::for_test_with_clock(
        credentials(),
        HttpPolicy::default(),
        server.uri(),
        Arc::new(RecordedSleeper(Mutex::new(vec![]))),
        clock,
    );
    let mut collector = Collector::new(client);

    let locales = collector.collect_all().await.expect("fixture collection");

    assert_eq!(locales.ko_kr.raw.collected_at, "2026-08-05T00:00:00Z");
    assert_eq!(locales.en_us.raw.collected_at, "2026-08-05T00:01:00Z");
}
