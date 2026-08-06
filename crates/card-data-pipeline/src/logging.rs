use std::io::{self, Write};

use card_data_contract::{CardCounts, SCHEMA_VERSION};
use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

/// credential나 request 타입을 받지 않는 secret-safe 실행 이벤트이다.
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub schema_version: u32,
    pub timestamp: String,
    pub level: &'static str,
    pub stage: &'static str,
    pub event: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<CardCounts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<&'static str>,
}

impl Event {
    pub fn started(stage: &'static str) -> io::Result<Self> {
        Self::new("info", stage, "started")
    }

    pub fn completed(stage: &'static str) -> io::Result<Self> {
        Self::new("info", stage, "completed")
    }

    pub fn locale_summary(locale: String, counts: CardCounts) -> io::Result<Self> {
        let mut event = Self::new("info", "locale", "summary")?;
        event.locale = Some(locale);
        event.counts = Some(counts);
        Ok(event)
    }

    pub fn retry(attempt: u8, status_code: Option<u16>) -> io::Result<Self> {
        let mut event = Self::new("warn", "collect", "retry")?;
        event.attempt = Some(attempt);
        event.status_code = status_code;
        Ok(event)
    }

    pub fn image_retry(attempt: u8, status_code: Option<u16>) -> io::Result<Self> {
        let mut event = Self::new("warn", "image_download", "retry")?;
        event.attempt = Some(attempt);
        event.status_code = status_code;
        Ok(event)
    }

    pub fn success() -> io::Result<Self> {
        Self::new("info", "final", "success")
    }

    pub fn failure(error_code: i32) -> io::Result<Self> {
        let mut event = Self::new("error", "final", "failure")?;
        event.error_code = Some(error_code);
        event.message = Some("build failed");
        Ok(event)
    }

    fn new(level: &'static str, stage: &'static str, event: &'static str) -> io::Result<Self> {
        let timestamp = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|error| io::Error::other(error.to_string()))?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            timestamp,
            level,
            stage,
            event,
            locale: None,
            attempt: None,
            status_code: None,
            counts: None,
            error_code: None,
            message: None,
        })
    }
}

pub trait EventSink {
    fn emit(&mut self, event: Event) -> io::Result<()>;
}

pub struct JsonlEventSink<W> {
    writer: W,
}

impl<W: Write> JsonlEventSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> EventSink for JsonlEventSink<W> {
    fn emit(&mut self, event: Event) -> io::Result<()> {
        serde_json::to_writer(&mut self.writer, &event)
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }
}

/// 테스트와 ignored live smoke에서 event 내용을 검증하는 메모리 sink이다.
#[doc(hidden)]
#[derive(Default)]
pub struct VecEventSink {
    pub events: Vec<Event>,
}

impl EventSink for VecEventSink {
    fn emit(&mut self, event: Event) -> io::Result<()> {
        self.events.push(event);
        Ok(())
    }
}
