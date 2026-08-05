use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use card_data_contract::{CardsPageResponse, MetadataResponse, OfficialCard};
use reqwest::{Client, StatusCode};
use secrecy::ExposeSecret;
use serde_json::Value;

use crate::{
    clock::{production_clock, production_sleeper, Clock},
    collect::{parse_card, parse_metadata, parse_page},
    oauth::TokenProvider,
    Credentials, HttpPolicy, PipelineError, Sleeper,
};

const PRODUCTION_BASE_URL: &str = "https://us.api.blizzard.com";
const PRODUCTION_TOKEN_URL: &str = "https://oauth.battle.net/token";

/// HTTP retry를 JSONL에 안전하게 연결하는 scalar-only 관찰 이벤트이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryEvent {
    pub attempt: u8,
    pub status_code: Option<u16>,
}

trait RetryObserver: Send + Sync {
    fn on_retry(&self, event: RetryEvent);
}

#[derive(Default)]
struct RetryLog {
    events: Mutex<Vec<RetryEvent>>,
}

impl RetryObserver for RetryLog {
    fn on_retry(&self, event: RetryEvent) {
        self.events.lock().expect("retry log lock").push(event);
    }
}

impl RetryLog {
    fn take(&self) -> Vec<RetryEvent> {
        std::mem::take(&mut *self.events.lock().expect("retry log lock"))
    }
}

pub struct BlizzardClient {
    client: Client,
    tokens: TokenProvider,
    api_base_url: String,
    policy: HttpPolicy,
    sleeper: Arc<dyn Sleeper>,
    retry_observer: Arc<dyn RetryObserver>,
    retry_log: Arc<RetryLog>,
    clock: Arc<dyn Clock>,
}

impl BlizzardClient {
    pub fn new(credentials: Credentials, policy: HttpPolicy) -> Result<Self, PipelineError> {
        Self::with_endpoints(
            credentials,
            policy,
            PRODUCTION_BASE_URL.into(),
            PRODUCTION_TOKEN_URL.into(),
            production_sleeper(),
            production_clock(),
        )
    }

    /// 비기본 test-support feature에서만 제공하는 endpoint 주입이다.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn for_test(
        credentials: Credentials,
        policy: HttpPolicy,
        base_url: String,
        sleeper: Arc<dyn Sleeper>,
    ) -> Self {
        let token_url = format!("{}/token", base_url.trim_end_matches('/'));
        Self::with_endpoints(
            credentials,
            policy,
            base_url,
            token_url,
            sleeper,
            production_clock(),
        )
        .expect("test HTTP client configuration")
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn for_test_with_clock(
        credentials: Credentials,
        policy: HttpPolicy,
        base_url: String,
        sleeper: Arc<dyn Sleeper>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let token_url = format!("{}/token", base_url.trim_end_matches('/'));
        Self::with_endpoints(credentials, policy, base_url, token_url, sleeper, clock)
            .expect("test HTTP client configuration")
    }

    fn with_endpoints(
        credentials: Credentials,
        policy: HttpPolicy,
        api_base_url: String,
        token_url: String,
        sleeper: Arc<dyn Sleeper>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, PipelineError> {
        let client = Client::builder()
            .connect_timeout(policy.connect_timeout)
            .timeout(policy.request_timeout)
            .build()
            .map_err(|_| PipelineError::Config("HTTP client configuration is invalid".into()))?;
        let tokens = TokenProvider::new(credentials, token_url, client.clone());
        let retry_log = Arc::new(RetryLog::default());
        Ok(Self {
            client,
            tokens,
            api_base_url: api_base_url.trim_end_matches('/').into(),
            policy,
            sleeper,
            retry_observer: retry_log.clone(),
            retry_log,
            clock,
        })
    }

    pub(crate) fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    /// retry 후 기록된 secret-free scalar event를 반환하고 buffer를 비운다.
    pub fn take_retry_events(&self) -> Vec<RetryEvent> {
        self.retry_log.take()
    }

    pub async fn fetch_cards_page(
        &mut self,
        locale: &str,
        page: u32,
    ) -> Result<CardsPageResponse, PipelineError> {
        let value = self.fetch_cards_page_value(locale, page).await?;
        parse_page(value)
    }

    pub async fn fetch_metadata(
        &mut self,
        locale: &str,
    ) -> Result<MetadataResponse, PipelineError> {
        let value = self.fetch_metadata_value(locale).await?;
        validate_metadata_arrays(&value)?;
        parse_metadata(value)
    }

    pub async fn fetch_card(
        &mut self,
        locale: &str,
        id: i64,
    ) -> Result<OfficialCard, PipelineError> {
        let value = self.fetch_card_value(locale, id).await?;
        parse_card(value)
    }

    pub(crate) async fn fetch_cards_page_value(
        &mut self,
        locale: &str,
        page: u32,
    ) -> Result<Value, PipelineError> {
        self.get_json(
            "/hearthstone/cards",
            &[
                ("locale", locale.into()),
                ("set", "standard".into()),
                ("gameMode", "constructed".into()),
                ("collectible", "0,1".into()),
                ("pageSize", "500".into()),
                ("page", page.to_string()),
            ],
        )
        .await
    }

    pub(crate) async fn fetch_metadata_value(
        &mut self,
        locale: &str,
    ) -> Result<Value, PipelineError> {
        self.get_json("/hearthstone/metadata", &[("locale", locale.into())])
            .await
    }

    pub(crate) async fn fetch_card_value(
        &mut self,
        locale: &str,
        id: i64,
    ) -> Result<Value, PipelineError> {
        self.get_json(
            &format!("/hearthstone/cards/{id}"),
            &[("locale", locale.into())],
        )
        .await
    }

    async fn get_json(
        &mut self,
        path: &str,
        query: &[(&'static str, String)],
    ) -> Result<Value, PipelineError> {
        let url = format!("{}{path}", self.api_base_url);
        let mut retries = 0;
        let mut refreshed_after_401 = false;
        loop {
            let token = self.tokens.token().await?.expose_secret().to_owned();
            let response = self
                .client
                .get(&url)
                .query(query)
                .bearer_auth(token)
                .send()
                .await;
            match response {
                Ok(response) if response.status() == StatusCode::UNAUTHORIZED => {
                    if refreshed_after_401 {
                        return Err(PipelineError::Auth(
                            "API rejected a refreshed access token".into(),
                        ));
                    }
                    self.tokens.force_refresh().await?;
                    refreshed_after_401 = true;
                }
                Ok(response) if retryable_status(response.status()) => {
                    if retries == self.policy.max_retries {
                        return Err(PipelineError::Network(
                            "retryable API response exhausted retries".into(),
                        ));
                    }
                    let delay = retry_after(&response, self.clock.now())
                        .unwrap_or_else(|| exponential_backoff(retries));
                    retries += 1;
                    self.retry_observer.on_retry(RetryEvent {
                        attempt: retries,
                        status_code: Some(response.status().as_u16()),
                    });
                    self.sleeper.sleep(delay).await;
                }
                Ok(response) if response.status().is_success() => match response.bytes().await {
                    Ok(bytes) => {
                        return serde_json::from_slice::<Value>(&bytes).map_err(|_| {
                            PipelineError::ApiStructure("API returned invalid JSON".into())
                        });
                    }
                    Err(_) => {
                        if retries == self.policy.max_retries {
                            return Err(PipelineError::Network(
                                "HTTP response body exhausted retries".into(),
                            ));
                        }
                        let delay = exponential_backoff(retries);
                        retries += 1;
                        self.retry_observer.on_retry(RetryEvent {
                            attempt: retries,
                            status_code: None,
                        });
                        self.sleeper.sleep(delay).await;
                    }
                },
                Ok(response) => {
                    return Err(PipelineError::ApiStructure(format!(
                        "API returned unexpected status {}",
                        response.status().as_u16()
                    )))
                }
                Err(_) => {
                    if retries == self.policy.max_retries {
                        return Err(PipelineError::Network(
                            "HTTP request exhausted retries".into(),
                        ));
                    }
                    let delay = exponential_backoff(retries);
                    retries += 1;
                    self.retry_observer.on_retry(RetryEvent {
                        attempt: retries,
                        status_code: None,
                    });
                    self.sleeper.sleep(delay).await;
                }
            }
        }
    }
}

fn retryable_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504)
}

fn exponential_backoff(retry: u8) -> Duration {
    use rand::RngExt;

    let jitter_millis = rand::rng().random_range(0..=250);
    Duration::from_secs(1_u64 << retry) + Duration::from_millis(jitter_millis)
}

fn retry_after(response: &reqwest::Response, now: std::time::SystemTime) -> Option<Duration> {
    let value = response.headers().get("Retry-After")?.to_str().ok()?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    httpdate::parse_http_date(value)
        .ok()?
        .duration_since(now)
        .ok()
}

fn validate_metadata_arrays(value: &Value) -> Result<(), PipelineError> {
    let object = value
        .as_object()
        .ok_or_else(|| PipelineError::ApiStructure("metadata response must be an object".into()))?;
    for key in [
        "sets",
        "classes",
        "types",
        "rarities",
        "minionTypes",
        "spellSchools",
        "keywords",
    ] {
        if !object.get(key).is_some_and(Value::is_array) {
            return Err(PipelineError::ApiStructure(format!(
                "metadata response is missing required array {key}"
            )));
        }
    }
    Ok(())
}
