use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[async_trait]
pub trait Sleeper: Send + Sync {
    async fn sleep(&self, delay: Duration);
}

pub struct TokioSleeper;

#[async_trait]
impl Sleeper for TokioSleeper {
    async fn sleep(&self, delay: Duration) {
        tokio::time::sleep(delay).await;
    }
}

pub(crate) fn production_sleeper() -> Arc<dyn Sleeper> {
    Arc::new(TokioSleeper)
}

pub(crate) fn production_clock() -> Arc<dyn Clock> {
    Arc::new(SystemClock)
}

pub(crate) fn utc_timestamp(clock: &dyn Clock) -> Result<String, time::error::Format> {
    OffsetDateTime::from(clock.now()).format(&Rfc3339)
}
