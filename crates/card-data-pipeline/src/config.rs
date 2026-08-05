use std::{env, time::Duration};

use secrecy::SecretString;

use crate::PipelineError;

/// OAuth 자격증명은 process memory 안에서만 유지한다.
pub struct Credentials {
    pub client_id: SecretString,
    pub client_secret: SecretString,
}

impl Credentials {
    pub fn from_env() -> Result<Self, PipelineError> {
        let client_id = env::var("BLIZZARD_CLIENT_ID")
            .map_err(|_| PipelineError::Config("BLIZZARD_CLIENT_ID is required".into()))?;
        let client_secret = env::var("BLIZZARD_CLIENT_SECRET")
            .map_err(|_| PipelineError::Config("BLIZZARD_CLIENT_SECRET is required".into()))?;
        if client_id.is_empty() || client_secret.is_empty() {
            return Err(PipelineError::Config(
                "Blizzard credentials must not be empty".into(),
            ));
        }
        Ok(Self {
            client_id: SecretString::from(client_id),
            client_secret: SecretString::from(client_secret),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpPolicy {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_retries: u8,
}

impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(60),
            max_retries: 3,
        }
    }
}
