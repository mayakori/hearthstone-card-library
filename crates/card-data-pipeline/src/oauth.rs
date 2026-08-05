use std::time::{Duration, Instant};

use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::{Credentials, PipelineError};

struct CachedToken {
    value: SecretString,
    expires_at: Instant,
}

pub struct TokenProvider {
    credentials: Credentials,
    token_url: String,
    client: Client,
    cached: Option<CachedToken>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

impl TokenProvider {
    pub(crate) fn new(credentials: Credentials, token_url: String, client: Client) -> Self {
        Self {
            credentials,
            token_url,
            client,
            cached: None,
        }
    }

    pub(crate) async fn token(&mut self) -> Result<&SecretString, PipelineError> {
        let valid = self.cached.as_ref().is_some_and(|token| {
            token.expires_at.saturating_duration_since(Instant::now()) > Duration::from_secs(300)
        });
        if !valid {
            self.refresh().await?;
        }
        Ok(&self.cached.as_ref().expect("token was refreshed").value)
    }

    pub(crate) async fn force_refresh(&mut self) -> Result<&SecretString, PipelineError> {
        self.cached = None;
        self.refresh().await?;
        Ok(&self.cached.as_ref().expect("token was refreshed").value)
    }

    async fn refresh(&mut self) -> Result<(), PipelineError> {
        let response = self
            .client
            .post(&self.token_url)
            .basic_auth(
                self.credentials.client_id.expose_secret(),
                Some(self.credentials.client_secret.expose_secret()),
            )
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .map_err(|_| PipelineError::Auth("token request failed".into()))?;
        if !response.status().is_success() {
            return Err(PipelineError::Auth(
                "token endpoint rejected client credentials".into(),
            ));
        }
        let token = response.json::<TokenResponse>().await.map_err(|_| {
            PipelineError::Auth("token endpoint returned an invalid response".into())
        })?;
        if token.access_token.is_empty() || token.expires_in == 0 {
            return Err(PipelineError::Auth(
                "token endpoint returned an unusable token".into(),
            ));
        }
        self.cached = Some(CachedToken {
            value: SecretString::from(token.access_token),
            expires_at: Instant::now() + Duration::from_secs(token.expires_in),
        });
        Ok(())
    }
}
