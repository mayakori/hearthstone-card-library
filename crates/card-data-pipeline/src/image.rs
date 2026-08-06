use std::{
    collections::BTreeSet,
    fs::{self, File},
    io,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use card_data_contract::Manifest;
use futures::{stream, StreamExt};
use reqwest::{redirect::Policy, Client};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

use crate::{
    clock::{production_sleeper, Sleeper},
    validate_package_directory, PipelineError,
};

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImageVariant {
    Normal,
    Crop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRequest {
    pub card_id: u64,
    pub locale: String,
    pub variant: ImageVariant,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSource {
    pub data_version: String,
    pub requests: Vec<ImageRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDownloadPolicy {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_retries: u8,
    pub max_bytes: usize,
    pub max_redirects: usize,
    pub concurrency: usize,
}

impl Default for ImageDownloadPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            max_bytes: MAX_IMAGE_BYTES,
            max_redirects: 3,
            concurrency: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageContent {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub media_type: String,
    pub extension: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedImage {
    pub request: ImageRequest,
    pub content: Option<ImageContent>,
    pub unavailable: Option<ImageUnavailable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageUnavailable {
    pub reason: String,
    pub status_code: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageRetryEvent {
    pub attempt: u8,
    pub status_code: Option<u16>,
}

#[derive(Default)]
struct ImageRetryLog(Mutex<Vec<ImageRetryEvent>>);

pub struct ImageDownloader {
    client: Client,
    policy: ImageDownloadPolicy,
    sleeper: Arc<dyn Sleeper>,
    allow_http: bool,
    retry_log: Arc<ImageRetryLog>,
}

impl ImageDownloader {
    pub fn new(policy: ImageDownloadPolicy) -> Result<Self, PipelineError> {
        Self::with_sleeper(policy, production_sleeper(), false)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn for_test(policy: ImageDownloadPolicy, sleeper: Arc<dyn Sleeper>) -> Self {
        Self::with_sleeper(policy, sleeper, true).expect("test image downloader configuration")
    }

    fn with_sleeper(
        policy: ImageDownloadPolicy,
        sleeper: Arc<dyn Sleeper>,
        allow_http: bool,
    ) -> Result<Self, PipelineError> {
        if policy.max_bytes == 0 || policy.concurrency == 0 {
            return Err(PipelineError::Config(
                "image download limits must be greater than zero".into(),
            ));
        }
        let redirect_policy = Policy::custom(move |attempt| {
            if attempt.previous().len() > policy.max_redirects {
                attempt.error("image redirect limit exceeded")
            } else if attempt.url().scheme() != "https"
                && !(allow_http && attempt.url().scheme() == "http")
            {
                attempt.error("image redirect must use HTTPS")
            } else {
                attempt.follow()
            }
        });
        let client = Client::builder()
            .connect_timeout(policy.connect_timeout)
            .timeout(policy.request_timeout)
            .redirect(redirect_policy)
            .build()
            .map_err(|error| {
                PipelineError::Config(format!("image HTTP client configuration: {error}"))
            })?;
        Ok(Self {
            client,
            policy,
            sleeper,
            allow_http,
            retry_log: Arc::new(ImageRetryLog::default()),
        })
    }

    pub fn take_retry_events(&self) -> Vec<ImageRetryEvent> {
        std::mem::take(&mut *self.retry_log.0.lock().expect("image retry log lock"))
    }

    pub async fn download(
        &self,
        requests: &[ImageRequest],
    ) -> Result<Vec<FetchedImage>, PipelineError> {
        let jobs = requests
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, request)| async move {
                let (content, unavailable) = match request.source_url.as_deref() {
                    Some(url) => match self
                        .download_one(url)
                        .await
                        .map_err(|error| with_request_context(error, &request))?
                    {
                        ImageDownload::Available(content) => (Some(content), None),
                        ImageDownload::Unavailable(unavailable) => (None, Some(unavailable)),
                    },
                    None => (None, None),
                };
                Ok::<_, PipelineError>((
                    index,
                    FetchedImage {
                        request,
                        content,
                        unavailable,
                    },
                ))
            });
        let results = stream::iter(jobs)
            .buffer_unordered(self.policy.concurrency)
            .collect::<Vec<_>>()
            .await;
        let mut fetched = Vec::with_capacity(results.len());
        for result in results {
            fetched.push(result?);
        }
        fetched.sort_unstable_by_key(|(index, _)| *index);
        Ok(fetched.into_iter().map(|(_, image)| image).collect())
    }

    async fn download_one(&self, url: &str) -> Result<ImageDownload, PipelineError> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|error| PipelineError::Network(format!("invalid image URL: {error}")))?;
        if parsed.scheme() != "https" && !(self.allow_http && parsed.scheme() == "http") {
            return Err(PipelineError::Network("image URL must use HTTPS".into()));
        }
        let mut attempt = 0_u8;
        'attempts: loop {
            let mut response = match self.client.get(parsed.clone()).send().await {
                Ok(response) => response,
                Err(_) if attempt < self.policy.max_retries => {
                    self.record_retry(attempt + 1, None);
                    self.sleeper.sleep(backoff(attempt)).await;
                    attempt += 1;
                    continue;
                }
                Err(error) => return Err(network_error(error)),
            };
            let status = response.status();
            if (status.as_u16() == 429 || status.is_server_error())
                && attempt < self.policy.max_retries
            {
                let delay = retry_after(&response).unwrap_or_else(|| backoff(attempt));
                self.record_retry(attempt + 1, Some(status.as_u16()));
                self.sleeper.sleep(delay).await;
                attempt += 1;
                continue;
            }
            if !status.is_success() {
                if status.is_client_error() && status.as_u16() != 429 {
                    return Ok(ImageDownload::Unavailable(ImageUnavailable {
                        reason: "http_status".into(),
                        status_code: status.as_u16(),
                    }));
                }
                return Err(PipelineError::Network(format!(
                    "image request returned HTTP {}",
                    status.as_u16()
                )));
            }
            if response.url().scheme() != "https"
                && !(self.allow_http && response.url().scheme() == "http")
            {
                return Err(PipelineError::Network(
                    "image redirect must use HTTPS".into(),
                ));
            }
            if response
                .content_length()
                .is_some_and(|length| length > self.policy.max_bytes as u64)
            {
                return Err(PipelineError::Network(
                    "image exceeds the maximum byte size".into(),
                ));
            }
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(str::trim)
                .map(str::to_owned)
                .ok_or_else(|| PipelineError::Network("image Content-Type is missing".into()))?;
            let mut bytes = Vec::new();
            loop {
                match response.chunk().await {
                    Ok(Some(chunk)) => {
                        if bytes.len().saturating_add(chunk.len()) > self.policy.max_bytes {
                            return Err(PipelineError::Network(
                                "image exceeds the maximum byte size".into(),
                            ));
                        }
                        bytes.extend_from_slice(&chunk);
                    }
                    Ok(None) => break,
                    Err(_) if attempt < self.policy.max_retries => {
                        self.record_retry(attempt + 1, None);
                        self.sleeper.sleep(backoff(attempt)).await;
                        attempt += 1;
                        continue 'attempts;
                    }
                    Err(error) => return Err(network_error(error)),
                }
            }
            let (media_type, extension) = verified_media(&content_type, &bytes)?;
            return Ok(ImageDownload::Available(ImageContent {
                sha256: hex::encode(Sha256::digest(&bytes)),
                bytes,
                media_type: media_type.into(),
                extension: extension.into(),
            }));
        }
    }

    fn record_retry(&self, attempt: u8, status_code: Option<u16>) {
        self.retry_log
            .0
            .lock()
            .expect("image retry log lock")
            .push(ImageRetryEvent {
                attempt,
                status_code,
            });
    }
}

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let value = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?;
    value
        .parse::<u64>()
        .map(Duration::from_secs)
        .ok()
        .or_else(|| {
            httpdate::parse_http_date(value)
                .ok()?
                .duration_since(SystemTime::now())
                .ok()
        })
}

fn backoff(attempt: u8) -> Duration {
    Duration::from_secs((1_u64 << attempt.min(5)).min(30))
}

enum ImageDownload {
    Available(ImageContent),
    Unavailable(ImageUnavailable),
}

fn verified_media(
    content_type: &str,
    bytes: &[u8],
) -> Result<(&'static str, &'static str), PipelineError> {
    let detected = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && bytes[8..12] == *b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    };
    let detected = detected.ok_or_else(|| {
        PipelineError::Network("image media type or magic bytes are invalid".into())
    })?;
    if content_type == "application/octet-stream" || content_type == detected.0 {
        Ok(detected)
    } else {
        Err(PipelineError::Network(
            "image media type or magic bytes are invalid".into(),
        ))
    }
}

fn network_error(error: reqwest::Error) -> PipelineError {
    PipelineError::Network(format!("image request failed: {error}"))
}

fn with_request_context(error: PipelineError, request: &ImageRequest) -> PipelineError {
    let variant = match request.variant {
        ImageVariant::Normal => "normal",
        ImageVariant::Crop => "crop",
    };
    match error {
        PipelineError::Network(message) => PipelineError::Network(format!(
            "{} card {} {variant}: {message}",
            request.locale, request.card_id
        )),
        error => error,
    }
}

pub fn load_image_source(package_root: impl AsRef<Path>) -> Result<ImageSource, PipelineError> {
    let package_root = package_root.as_ref();
    let manifest_bytes = fs::read(package_root.join("manifest.json")).map_err(io_error)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| package_error(format!("parse manifest: {error}")))?;
    validate_package_directory(package_root, &manifest)?;

    let temporary = tempfile::tempdir().map_err(io_error)?;
    let mut requests = Vec::new();
    let mut expected_ids: Option<BTreeSet<u64>> = None;

    for locale in ["ko_KR", "en_US"] {
        let locale_manifest = manifest
            .locales
            .get(locale)
            .ok_or_else(|| package_error(format!("missing locale manifest: {locale}")))?;
        let sqlite_path = temporary.path().join(format!("{locale}.sqlite"));
        let input =
            File::open(package_root.join(&locale_manifest.normalized.path)).map_err(io_error)?;
        let mut decoder = zstd::stream::read::Decoder::new(input)
            .map_err(|error| package_error(format!("decompress image source SQLite: {error}")))?;
        let mut output = File::create(&sqlite_path).map_err(io_error)?;
        io::copy(&mut decoder, &mut output).map_err(io_error)?;
        drop(output);

        let connection =
            Connection::open_with_flags(&sqlite_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(sqlite_error)?;
        let mut statement = connection
            .prepare("SELECT id, image_url, crop_image_url FROM cards ORDER BY id")
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(sqlite_error)?;
        let mut ids = BTreeSet::new();
        for row in rows {
            let (card_id, normal, crop) = row.map_err(sqlite_error)?;
            let card_id = u64::try_from(card_id)
                .map_err(|_| package_error(format!("invalid card ID in {locale}")))?;
            if !ids.insert(card_id) {
                return Err(package_error(format!("duplicate card ID in {locale}")));
            }
            requests.push(ImageRequest {
                card_id,
                locale: locale.into(),
                variant: ImageVariant::Normal,
                source_url: nonempty_url(normal, locale, card_id, "normal")?,
            });
            requests.push(ImageRequest {
                card_id,
                locale: locale.into(),
                variant: ImageVariant::Crop,
                source_url: nonempty_url(crop, locale, card_id, "crop")?,
            });
        }
        if ids.len() as u64 != locale_manifest.card_counts.total {
            return Err(package_error(format!(
                "card count mismatch for {locale} image source"
            )));
        }
        match &expected_ids {
            Some(expected) if expected != &ids => {
                return Err(package_error("locale card ID parity mismatch"));
            }
            None => expected_ids = Some(ids),
            _ => {}
        }
    }

    Ok(ImageSource {
        data_version: manifest.data_version,
        requests,
    })
}

fn nonempty_url(
    value: Option<String>,
    locale: &str,
    card_id: u64,
    variant: &str,
) -> Result<Option<String>, PipelineError> {
    match value {
        Some(value) if value.is_empty() => Err(package_error(format!(
            "empty {variant} image URL for {locale} card {card_id}"
        ))),
        value => Ok(value),
    }
}

fn package_error(message: impl Into<String>) -> PipelineError {
    PipelineError::Package(message.into())
}

fn io_error(error: io::Error) -> PipelineError {
    PipelineError::Io(error.to_string())
}

fn sqlite_error(error: rusqlite::Error) -> PipelineError {
    PipelineError::Package(format!("image source SQLite: {error}"))
}
