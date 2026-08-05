use std::{collections::BTreeMap, io, path::PathBuf};

use card_data_contract::{DataVersion, Manifest};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

mod clock;
mod collect;
mod config;
mod error;
mod http;
mod logging;
mod normalize;
mod oauth;
mod package;
mod sqlite;

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use clock::Clock;
pub use clock::Sleeper;
pub use collect::{CollectedLocale, CollectedLocales, Collector};
pub use config::{Credentials, HttpPolicy};
pub use error::PipelineError;
pub use http::{BlizzardClient, RetryEvent};
pub use logging::{Event, EventSink, JsonlEventSink, VecEventSink};
pub use normalize::normalize_locale;
pub use oauth::TokenProvider;
pub use package::{
    validate_package_directory, PackageBuilder, PackageLocaleInput, PackageRequest, PackageResult,
};
pub use sqlite::{SqliteBuildMetadata, SqliteWriter};

pub struct BuildRequest {
    pub data_version: DataVersion,
    pub output_root: PathBuf,
    pub credentials: Credentials,
}

pub struct BuildResult {
    pub version_directory: PathBuf,
    pub manifest: Manifest,
}

/// 안정된 pipeline error 분류를 CLI process exit code로 변환한다.
pub fn exit_code(error: &PipelineError) -> i32 {
    match error {
        PipelineError::Cli(_) | PipelineError::Config(_) => 2,
        PipelineError::Auth(_) => 3,
        PipelineError::Network(_) => 4,
        PipelineError::ApiStructure(_) => 5,
        PipelineError::Normalize(_) | PipelineError::Sqlite(_) => 6,
        PipelineError::Package(_) | PipelineError::Io(_) => 7,
    }
}

/// process 경계에서 final JSONL failure event와 안정된 exit code를 함께 결정한다.
///
/// event sink 자체가 실패해도 원래 pipeline 오류의 exit 분류를 유지한다.
pub fn finish_process(result: Result<(), PipelineError>, events: &mut dyn EventSink) -> u8 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            let code = exit_code(&error);
            if let Ok(event) = Event::failure(code) {
                let _ = events.emit(event);
            }
            code as u8
        }
    }
}

/// 공식 API 수집부터 package 발행까지 실제 production adapter를 실행한다.
pub async fn run_build(
    request: BuildRequest,
    events: &mut dyn EventSink,
) -> Result<BuildResult, PipelineError> {
    let client = BlizzardClient::new(request.credentials, HttpPolicy::default())?;
    run_build_inner(request.data_version, request.output_root, client, events).await
}

/// offline 전체 pipeline 검증이 production orchestration을 그대로 호출하도록 하는
/// 비기본 test-support adapter이다.
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub async fn run_build_with_test_client(
    request: BuildRequest,
    client: BlizzardClient,
    events: &mut dyn EventSink,
) -> Result<BuildResult, PipelineError> {
    run_build_inner(request.data_version, request.output_root, client, events).await
}

async fn run_build_inner(
    data_version: DataVersion,
    output_root: PathBuf,
    client: BlizzardClient,
    events: &mut dyn EventSink,
) -> Result<BuildResult, PipelineError> {
    if output_root.join(data_version.to_string()).exists() {
        return Err(PipelineError::Config(
            "package directory already exists".into(),
        ));
    }

    let clock = client.clock();
    emit(events, Event::started("collect"))?;
    let mut collector = Collector::new(client);
    let collected = collector.collect_all().await;
    for retry in collector.take_retry_events() {
        emit(events, Event::retry(retry.attempt, retry.status_code))?;
    }
    let locales = collected?;
    emit(events, Event::completed("collect"))?;

    let generated_at = clock::utc_timestamp(clock.as_ref())
        .map_err(|error| PipelineError::Io(error.to_string()))?;
    let temporary = tempfile::tempdir().map_err(io_error)?;
    let package_inputs =
        prepare_package_inputs(&locales, &data_version, &generated_at, &temporary, events)?;

    emit(events, Event::started("package"))?;
    let result = PackageBuilder::build(PackageRequest {
        data_version,
        output_root,
        generated_at,
        locales: package_inputs,
    })?;

    // Atomic rename inside PackageBuilder is the publish commit point. Once it
    // succeeds, logging failures must not turn a durable package into a failed,
    // non-retryable build result.
    emit_best_effort(events, Event::completed("package"));
    emit_best_effort(events, Event::success());

    Ok(BuildResult {
        version_directory: result.version_directory,
        manifest: result.manifest,
    })
}

fn prepare_package_inputs(
    locales: &CollectedLocales,
    data_version: &DataVersion,
    generated_at: &str,
    temporary: &TempDir,
    events: &mut dyn EventSink,
) -> Result<BTreeMap<String, PackageLocaleInput>, PipelineError> {
    let mut inputs = BTreeMap::new();
    for locale in [&locales.ko_kr, &locales.en_us] {
        emit(events, Event::started("normalize"))?;
        let raw_bytes = locale
            .raw
            .canonical_bytes()
            .map_err(|error| PipelineError::ApiStructure(error.to_string()))?;
        let raw_sha256 = hex::encode(Sha256::digest(&raw_bytes));
        let catalog = normalize_locale(locale, &raw_sha256, generated_at)?;
        let sqlite_path = temporary.path().join(format!("{}.sqlite", locale.locale));
        let metadata = SqliteBuildMetadata::new(data_version.to_string(), generated_at, &catalog);
        SqliteWriter::write(&sqlite_path, &catalog, &metadata)?;
        emit(events, Event::completed("normalize"))?;
        emit(
            events,
            Event::locale_summary(locale.locale.clone(), catalog.card_counts),
        )?;
        inputs.insert(
            locale.locale.clone(),
            PackageLocaleInput {
                raw_bytes,
                sqlite_path,
                card_counts: catalog.card_counts,
            },
        );
    }
    Ok(inputs)
}

fn emit(events: &mut dyn EventSink, event: io::Result<Event>) -> Result<(), PipelineError> {
    events.emit(event.map_err(io_error)?).map_err(io_error)
}

fn emit_best_effort(events: &mut dyn EventSink, event: io::Result<Event>) {
    if let Ok(event) = event {
        let _ = events.emit(event);
    }
}

fn io_error(error: io::Error) -> PipelineError {
    PipelineError::Io(error.to_string())
}
