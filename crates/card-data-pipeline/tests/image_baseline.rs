use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use card_data_contract::{CardCounts, DataVersion};
use card_data_pipeline::{
    build_image_candidate, load_image_source, verify_image_candidate, FetchedImage,
    ImageCandidateRequest, ImageContent, ImageDownloadPolicy, ImageDownloader, ImageRequest,
    ImageSource, ImageUnavailable, ImageVariant, PackageBuilder, PackageLocaleInput,
    PackageRequest, PipelineError, Sleeper,
};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, Request, Respond, ResponseTemplate,
};

const DATA_VERSION: &str = "36.0.3-build247416-r1";
const GENERATED_AT: &str = "2026-08-06T00:00:00Z";

#[test]
fn source_loads_every_card_variant_in_stable_order_and_preserves_absent_urls() {
    let root = tempfile::tempdir().unwrap();
    let package = build_package(root.path(), false);

    let source = load_image_source(&package).unwrap();

    assert_eq!(source.data_version, DATA_VERSION);
    assert_eq!(
        source.requests,
        vec![
            request(
                10,
                "ko_KR",
                ImageVariant::Normal,
                Some("https://images.test/ko-10.png")
            ),
            request(
                10,
                "ko_KR",
                ImageVariant::Crop,
                Some("https://images.test/ko-10-crop.jpg")
            ),
            request(20, "ko_KR", ImageVariant::Normal, None),
            request(
                20,
                "ko_KR",
                ImageVariant::Crop,
                Some("https://images.test/shared.webp")
            ),
            request(
                10,
                "en_US",
                ImageVariant::Normal,
                Some("https://images.test/en-10.png")
            ),
            request(10, "en_US", ImageVariant::Crop, None),
            request(
                20,
                "en_US",
                ImageVariant::Normal,
                Some("https://images.test/en-20.jpg")
            ),
            request(
                20,
                "en_US",
                ImageVariant::Crop,
                Some("https://images.test/shared.webp")
            ),
        ]
    );
}

#[test]
fn source_rejects_locale_card_id_drift() {
    let root = tempfile::tempdir().unwrap();
    let package = build_package(root.path(), true);

    assert!(matches!(
        load_image_source(&package),
        Err(PipelineError::Package(message)) if message.contains("card ID parity")
    ));
}

#[derive(Default)]
struct RecordedSleeper(Mutex<Vec<Duration>>);

#[async_trait::async_trait]
impl Sleeper for RecordedSleeper {
    async fn sleep(&self, delay: Duration) {
        self.0.lock().unwrap().push(delay);
    }
}

struct Sequence(Mutex<Vec<ResponseTemplate>>);

impl Respond for Sequence {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        self.0.lock().unwrap().remove(0)
    }
}

#[tokio::test]
async fn downloader_preserves_absent_urls_and_records_verified_png_bytes() {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, b'x'];
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/png")
                .set_body_bytes(PNG),
        )
        .expect(1)
        .mount(&server)
        .await;
    let requests = vec![
        request(
            10,
            "ko_KR",
            ImageVariant::Normal,
            Some(&format!("{}/card.png", server.uri())),
        ),
        request(10, "ko_KR", ImageVariant::Crop, None),
    ];
    let downloader = ImageDownloader::for_test(
        ImageDownloadPolicy::default(),
        Arc::new(RecordedSleeper::default()),
    );

    let fetched = downloader.download(&requests).await.unwrap();

    assert_eq!(fetched.len(), 2);
    let content = fetched[0].content.as_ref().unwrap();
    assert_eq!(content.bytes, PNG);
    assert_eq!(
        content.sha256,
        "1cd153041b3d879295c47092b98f7458eb01d021db67f47e8d7f76d93d8b8ff5"
    );
    assert_eq!(content.media_type, "image/png");
    assert_eq!(content.extension, "png");
    assert!(fetched[1].content.is_none());
}

#[tokio::test]
async fn downloader_honors_retry_after_before_retrying_transient_status() {
    const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, b'x'];
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card.jpg"))
        .respond_with(Sequence(Mutex::new(vec![
            ResponseTemplate::new(503).insert_header("retry-after", "2"),
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/jpeg")
                .set_body_bytes(JPEG),
        ])))
        .expect(2)
        .mount(&server)
        .await;
    let sleeper = Arc::new(RecordedSleeper::default());
    let downloader = ImageDownloader::for_test(ImageDownloadPolicy::default(), sleeper.clone());
    let requests = vec![request(
        10,
        "ko_KR",
        ImageVariant::Normal,
        Some(&format!("{}/card.jpg", server.uri())),
    )];

    let fetched = downloader.download(&requests).await.unwrap();

    assert_eq!(fetched[0].content.as_ref().unwrap().extension, "jpg");
    assert_eq!(*sleeper.0.lock().unwrap(), vec![Duration::from_secs(2)]);
    assert_eq!(
        downloader.take_retry_events(),
        vec![card_data_pipeline::ImageRetryEvent {
            attempt: 1,
            status_code: Some(503),
        }]
    );
}

#[tokio::test]
async fn downloader_uses_bounded_concurrency_without_reordering_results() {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, b'x'];
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/png")
                .set_body_bytes(PNG)
                .set_delay(Duration::from_millis(100)),
        )
        .expect(4)
        .mount(&server)
        .await;
    let requests = (1..=4)
        .map(|card_id| {
            request(
                card_id,
                "ko_KR",
                ImageVariant::Normal,
                Some(&format!("{}/card.png", server.uri())),
            )
        })
        .collect::<Vec<_>>();
    let downloader = ImageDownloader::for_test(
        ImageDownloadPolicy::default(),
        Arc::new(RecordedSleeper::default()),
    );

    let started = tokio::time::Instant::now();
    let fetched = downloader.download(&requests).await.unwrap();

    assert!(started.elapsed() < Duration::from_millis(330));
    assert_eq!(
        fetched
            .iter()
            .map(|image| image.request.card_id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
}

#[tokio::test]
async fn downloader_rejects_non_retryable_status_and_invalid_media() {
    for (content_type, body, expected) in [
        (
            "image/jpeg",
            b"\x89PNG\r\n\x1a\nwrong-header".to_vec(),
            "magic bytes",
        ),
        ("text/html", b"not-an-image".to_vec(), "media type"),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bad"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", content_type)
                    .set_body_bytes(body),
            )
            .expect(1)
            .mount(&server)
            .await;
        let requests = vec![request(
            10,
            "ko_KR",
            ImageVariant::Normal,
            Some(&format!("{}/bad", server.uri())),
        )];
        let result = ImageDownloader::for_test(
            ImageDownloadPolicy::default(),
            Arc::new(RecordedSleeper::default()),
        )
        .download(&requests)
        .await;
        assert!(
            matches!(result, Err(PipelineError::Network(message)) if message.contains(expected))
        );
    }
}

#[tokio::test]
async fn downloader_detects_octet_stream_bytes_and_preserves_unavailable_4xx() {
    const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, b'x'];
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/generic-binary.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/octet-stream")
                .set_body_bytes(JPEG),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/unavailable.png"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;
    let requests = vec![
        request(
            467,
            "ko_KR",
            ImageVariant::Crop,
            Some(&format!("{}/generic-binary.png", server.uri())),
        ),
        request(
            69571,
            "ko_KR",
            ImageVariant::Normal,
            Some(&format!("{}/unavailable.png", server.uri())),
        ),
    ];

    let fetched = ImageDownloader::for_test(
        ImageDownloadPolicy::default(),
        Arc::new(RecordedSleeper::default()),
    )
    .download(&requests)
    .await
    .unwrap();

    let content = fetched[0].content.as_ref().unwrap();
    assert_eq!(content.media_type, "image/jpeg");
    assert_eq!(content.extension, "jpg");
    assert_eq!(fetched[1].content, None);
    assert_eq!(
        fetched[1].unavailable,
        Some(ImageUnavailable {
            reason: "http_status".into(),
            status_code: 403,
        })
    );
}

#[tokio::test]
async fn downloader_rejects_an_image_above_the_configured_byte_cap() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/large.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/png")
                .set_body_bytes(b"\x89PNG\r\n\x1a\ntoo-large"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut policy = ImageDownloadPolicy::default();
    policy.max_bytes = 8;
    let requests = vec![request(
        10,
        "ko_KR",
        ImageVariant::Normal,
        Some(&format!("{}/large.png", server.uri())),
    )];

    let result = ImageDownloader::for_test(policy, Arc::new(RecordedSleeper::default()))
        .download(&requests)
        .await;

    assert!(
        matches!(result, Err(PipelineError::Network(message)) if message.contains("maximum byte"))
    );
}

#[tokio::test]
async fn downloader_accepts_verified_webp_and_production_rejects_http() {
    const WEBP: &[u8] = b"RIFF\x04\x00\x00\x00WEBPdata";
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card.webp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/webp")
                .set_body_bytes(WEBP),
        )
        .expect(1)
        .mount(&server)
        .await;
    let requests = vec![request(
        10,
        "ko_KR",
        ImageVariant::Normal,
        Some(&format!("{}/card.webp", server.uri())),
    )];
    let fetched = ImageDownloader::for_test(
        ImageDownloadPolicy::default(),
        Arc::new(RecordedSleeper::default()),
    )
    .download(&requests)
    .await
    .unwrap();
    assert_eq!(fetched[0].content.as_ref().unwrap().extension, "webp");

    let result = ImageDownloader::new(ImageDownloadPolicy::default())
        .unwrap()
        .download(&requests)
        .await;
    assert!(matches!(
        result,
        Err(PipelineError::Network(message)) if message.contains("HTTPS")
    ));
}

#[tokio::test]
async fn downloader_enforces_timeout_and_redirect_limit() {
    let timeout_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/png")
                .set_body_bytes(b"\x89PNG\r\n\x1a\nslow")
                .set_delay(Duration::from_millis(100)),
        )
        .mount(&timeout_server)
        .await;
    let mut timeout_policy = ImageDownloadPolicy::default();
    timeout_policy.request_timeout = Duration::from_millis(10);
    timeout_policy.max_retries = 0;
    let timeout_result =
        ImageDownloader::for_test(timeout_policy, Arc::new(RecordedSleeper::default()))
            .download(&[request(
                10,
                "ko_KR",
                ImageVariant::Normal,
                Some(&format!("{}/slow.png", timeout_server.uri())),
            )])
            .await;
    assert!(matches!(timeout_result, Err(PipelineError::Network(_))));

    let redirect_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/first"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/second"))
        .mount(&redirect_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/second"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/third"))
        .mount(&redirect_server)
        .await;
    let mut redirect_policy = ImageDownloadPolicy::default();
    redirect_policy.max_redirects = 1;
    redirect_policy.max_retries = 0;
    let redirect_result =
        ImageDownloader::for_test(redirect_policy, Arc::new(RecordedSleeper::default()))
            .download(&[request(
                10,
                "ko_KR",
                ImageVariant::Normal,
                Some(&format!("{}/first", redirect_server.uri())),
            )])
            .await;
    assert!(matches!(redirect_result, Err(PipelineError::Network(_))));
}

#[test]
fn pack_deduplicates_cross_locale_bytes_and_is_exactly_deterministic() {
    let png = b"\x89PNG\r\n\x1a\ndeduplicated".to_vec();
    let source = ImageSource {
        data_version: DATA_VERSION.into(),
        requests: vec![
            request(
                10,
                "ko_KR",
                ImageVariant::Normal,
                Some("https://images.test/ko.png"),
            ),
            request(10, "ko_KR", ImageVariant::Crop, None),
            request(
                10,
                "en_US",
                ImageVariant::Normal,
                Some("https://images.test/en.png"),
            ),
            request(10, "en_US", ImageVariant::Crop, None),
        ],
    };
    let fetched = vec![
        fetched(&source.requests[0], Some(content(&png, "image/png", "png"))),
        fetched(&source.requests[1], None),
        fetched(&source.requests[2], Some(content(&png, "image/png", "png"))),
        fetched(&source.requests[3], None),
    ];
    let first_root = tempfile::tempdir().unwrap();
    let second_root = tempfile::tempdir().unwrap();

    let first = build_image_candidate(candidate_request(
        first_root.path(),
        &source,
        &fetched,
        1024 * 1024,
    ))
    .unwrap();
    let second = build_image_candidate(candidate_request(
        second_root.path(),
        &source,
        &fetched,
        1024 * 1024,
    ))
    .unwrap();

    assert_eq!(candidate_files(&first.root), candidate_files(&second.root));
    let receipt = verify_image_candidate(&first.root).unwrap();
    assert_eq!(receipt.request_count, 4);
    assert_eq!(receipt.absent_count, 2);
    assert_eq!(receipt.unavailable_count, 0);
    assert_eq!(receipt.success_count, 2);
    assert_eq!(receipt.unique_image_count, 1);
    assert_eq!(receipt.packs.len(), 1);
    assert_eq!(receipt.packs[0].member_count, 1);
    let en_map = read_zstd_json(&first.root.join("maps/en_US.json.zst"));
    assert_eq!(
        en_map["cards"][0]["normal"]["pack"],
        "packs/ko_KR-normal-000.tar.zst"
    );
    assert_eq!(en_map["cards"][0]["normal"]["state"], "available");
    assert!(en_map["cards"][0]["crop"].is_null());
}

#[test]
fn map_distinguishes_absent_and_unavailable_assets() {
    let source = ImageSource {
        data_version: DATA_VERSION.into(),
        requests: vec![
            request(
                69571,
                "ko_KR",
                ImageVariant::Normal,
                Some("https://images.test/ko-murloc-scout.png"),
            ),
            request(69571, "ko_KR", ImageVariant::Crop, None),
            request(
                69571,
                "en_US",
                ImageVariant::Normal,
                Some("https://images.test/en-murloc-scout.png"),
            ),
            request(69571, "en_US", ImageVariant::Crop, None),
        ],
    };
    let fetched = vec![
        unavailable(&source.requests[0], 403),
        fetched(&source.requests[1], None),
        unavailable(&source.requests[2], 403),
        fetched(&source.requests[3], None),
    ];
    let output = tempfile::tempdir().unwrap();

    let candidate = build_image_candidate(candidate_request(
        output.path(),
        &source,
        &fetched,
        1024 * 1024,
    ))
    .unwrap();

    assert_eq!(candidate.receipt.absent_count, 2);
    assert_eq!(candidate.receipt.unavailable_count, 2);
    assert_eq!(candidate.receipt.success_count, 0);
    let ko_map = read_zstd_json(&candidate.root.join("maps/ko_KR.json.zst"));
    assert_eq!(ko_map["cards"][0]["normal"]["state"], "unavailable");
    assert_eq!(ko_map["cards"][0]["normal"]["statusCode"], 403);
    assert_eq!(ko_map["cards"][0]["normal"]["reason"], "http_status");
    assert!(ko_map["cards"][0]["crop"].is_null());
}

#[test]
fn verify_rejects_a_mutated_pack_object() {
    let png = b"\x89PNG\r\n\x1a\nverified".to_vec();
    let source = ImageSource {
        data_version: DATA_VERSION.into(),
        requests: vec![
            request(
                10,
                "ko_KR",
                ImageVariant::Normal,
                Some("https://images.test/ko.png"),
            ),
            request(10, "ko_KR", ImageVariant::Crop, None),
            request(
                10,
                "en_US",
                ImageVariant::Normal,
                Some("https://images.test/en.png"),
            ),
            request(10, "en_US", ImageVariant::Crop, None),
        ],
    };
    let fetched = vec![
        fetched(&source.requests[0], Some(content(&png, "image/png", "png"))),
        fetched(&source.requests[1], None),
        fetched(&source.requests[2], Some(content(&png, "image/png", "png"))),
        fetched(&source.requests[3], None),
    ];
    let output = tempfile::tempdir().unwrap();
    let candidate = build_image_candidate(candidate_request(
        output.path(),
        &source,
        &fetched,
        1024 * 1024,
    ))
    .unwrap();
    let pack = candidate.root.join(&candidate.receipt.packs[0].path);
    let mut bytes = fs::read(&pack).unwrap();
    bytes.push(0);
    fs::write(pack, bytes).unwrap();

    assert!(matches!(
        verify_image_candidate(&candidate.root),
        Err(PipelineError::Package(message)) if message.contains("digest mismatch")
    ));
}

#[test]
fn pack_splits_a_hash_ordered_group_under_a_small_test_cap() {
    let first = noisy_png(1, 1200);
    let second = noisy_png(2, 1200);
    let source = ImageSource {
        data_version: DATA_VERSION.into(),
        requests: ["ko_KR", "en_US"]
            .into_iter()
            .flat_map(|locale| {
                [10_u64, 20].into_iter().flat_map(move |card_id| {
                    [
                        request(
                            card_id,
                            locale,
                            ImageVariant::Normal,
                            Some("https://images.test/shared.png"),
                        ),
                        request(card_id, locale, ImageVariant::Crop, None),
                    ]
                })
            })
            .collect(),
    };
    let fetched = source
        .requests
        .iter()
        .map(|request| {
            let bytes = match (request.card_id, request.variant) {
                (10, ImageVariant::Normal) => Some(first.as_slice()),
                (20, ImageVariant::Normal) => Some(second.as_slice()),
                _ => None,
            };
            fetched(
                request,
                bytes.map(|bytes| content(bytes, "image/png", "png")),
            )
        })
        .collect::<Vec<_>>();
    let output = tempfile::tempdir().unwrap();

    let candidate =
        build_image_candidate(candidate_request(output.path(), &source, &fetched, 1800)).unwrap();

    assert_eq!(candidate.receipt.packs.len(), 2);
    assert_eq!(
        candidate.receipt.packs[0].path,
        "packs/ko_KR-normal-000.tar.zst"
    );
    assert_eq!(
        candidate.receipt.packs[1].path,
        "packs/ko_KR-normal-001.tar.zst"
    );
    assert!(candidate
        .receipt
        .packs
        .iter()
        .all(|pack| pack.bytes <= 1800));
}

#[test]
fn pack_rejects_an_unsafe_run_identity_before_writing_candidate_files() {
    let source = ImageSource {
        data_version: DATA_VERSION.into(),
        requests: vec![
            request(10, "ko_KR", ImageVariant::Normal, None),
            request(10, "ko_KR", ImageVariant::Crop, None),
            request(10, "en_US", ImageVariant::Normal, None),
            request(10, "en_US", ImageVariant::Crop, None),
        ],
    };
    let fetched = source
        .requests
        .iter()
        .map(|request| fetched(request, None))
        .collect::<Vec<_>>();
    let output = tempfile::tempdir().unwrap();
    let mut candidate = candidate_request(output.path(), &source, &fetched, 1024);
    candidate.run_id = "../unsafe";

    assert!(matches!(
        build_image_candidate(candidate),
        Err(PipelineError::Package(message)) if message.contains("identity")
    ));
    assert_eq!(fs::read_dir(output.path()).unwrap().count(), 0);
}

fn noisy_png(seed: u32, length: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut state = seed;
    for _ in 0..length {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        bytes.push(state as u8);
    }
    bytes
}

fn candidate_request<'a>(
    output_root: &'a Path,
    source: &'a ImageSource,
    fetched: &'a [FetchedImage],
    pack_limit: usize,
) -> ImageCandidateRequest<'a> {
    ImageCandidateRequest {
        output_root,
        source,
        fetched,
        run_id: "12345",
        run_attempt: 2,
        input_manifest_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        normalized_sha256: BTreeMap::from([
            (
                "en_US".into(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            ),
            (
                "ko_KR".into(),
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".into(),
            ),
        ]),
        pack_limit,
    }
}

fn content(bytes: &[u8], media_type: &str, extension: &str) -> ImageContent {
    ImageContent {
        bytes: bytes.to_vec(),
        sha256: hex::encode(Sha256::digest(bytes)),
        media_type: media_type.into(),
        extension: extension.into(),
    }
}

fn fetched(request: &ImageRequest, content: Option<ImageContent>) -> FetchedImage {
    FetchedImage {
        request: request.clone(),
        content,
        unavailable: None,
    }
}

fn unavailable(request: &ImageRequest, status_code: u16) -> FetchedImage {
    FetchedImage {
        request: request.clone(),
        content: None,
        unavailable: Some(ImageUnavailable {
            reason: "http_status".into(),
            status_code,
        }),
    }
}

fn candidate_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_candidate_files(root, root, &mut files);
    files
}

fn collect_candidate_files(root: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_candidate_files(root, &path, files);
        } else {
            files.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                fs::read(path).unwrap(),
            );
        }
    }
}

fn read_zstd_json(path: &Path) -> serde_json::Value {
    let bytes = zstd::stream::decode_all(fs::File::open(path).unwrap()).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn request(
    card_id: u64,
    locale: &str,
    variant: ImageVariant,
    source_url: Option<&str>,
) -> ImageRequest {
    ImageRequest {
        card_id,
        locale: locale.into(),
        variant,
        source_url: source_url.map(str::to_owned),
    }
}

fn build_package(root: &Path, drift_en_us: bool) -> std::path::PathBuf {
    let input = root.join("input");
    fs::create_dir_all(&input).unwrap();
    let mut locales = BTreeMap::new();
    for locale in ["ko_KR", "en_US"] {
        let sqlite = input.join(format!("{locale}.sqlite"));
        let raw = format!("{{\"locale\":\"{locale}\"}}\n").into_bytes();
        write_sqlite(&sqlite, locale, &raw, drift_en_us && locale == "en_US");
        locales.insert(
            locale.into(),
            PackageLocaleInput {
                raw_bytes: raw,
                sqlite_path: sqlite,
                card_counts: CardCounts {
                    standard: 2,
                    related: 0,
                    class_reference: 0,
                    total: 2,
                },
            },
        );
    }
    PackageBuilder::build(PackageRequest {
        data_version: DataVersion::parse(DATA_VERSION).unwrap(),
        output_root: root.join("output"),
        generated_at: GENERATED_AT.into(),
        locales,
    })
    .unwrap()
    .version_directory
}

fn write_sqlite(path: &Path, locale: &str, raw: &[u8], drift: bool) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "PRAGMA user_version = 1;
             CREATE TABLE catalog_metadata (
               schema_version INTEGER,
               data_version TEXT,
               locale TEXT,
               generated_at TEXT,
               source_raw_sha256 TEXT,
               standard_card_count INTEGER,
               related_card_count INTEGER,
               class_reference_card_count INTEGER,
               total_card_count INTEGER
             );
             CREATE TABLE cards (
               id INTEGER PRIMARY KEY,
               image_url TEXT,
               crop_image_url TEXT
             );",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO catalog_metadata VALUES (?1, ?2, ?3, ?4, ?5, 2, 0, 0, 2)",
            params![
                1,
                DATA_VERSION,
                locale,
                GENERATED_AT,
                hex::encode(Sha256::digest(raw))
            ],
        )
        .unwrap();

    let rows = match locale {
        "ko_KR" => vec![
            (
                10_i64,
                Some("https://images.test/ko-10.png"),
                Some("https://images.test/ko-10-crop.jpg"),
            ),
            (20, None, Some("https://images.test/shared.webp")),
        ],
        "en_US" if drift => vec![
            (10, Some("https://images.test/en-10.png"), None),
            (
                30,
                Some("https://images.test/en-30.jpg"),
                Some("https://images.test/shared.webp"),
            ),
        ],
        "en_US" => vec![
            (10, Some("https://images.test/en-10.png"), None),
            (
                20,
                Some("https://images.test/en-20.jpg"),
                Some("https://images.test/shared.webp"),
            ),
        ],
        _ => unreachable!(),
    };
    for (id, normal, crop) in rows {
        connection
            .execute(
                "INSERT INTO cards VALUES (?1, ?2, ?3)",
                params![id, normal, crop],
            )
            .unwrap();
    }
}
