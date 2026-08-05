use std::{env, io, process::ExitCode};

use card_data_pipeline::{finish_process, JsonlEventSink, PipelineError};

const FIXTURE_SECRET: &str = "fixture-client-secret";
const FIXTURE_TOKEN: &str = "fixture-access-token";

fn main() -> ExitCode {
    let stderr = io::stderr();
    let mut events = JsonlEventSink::new(stderr.lock());
    ExitCode::from(finish_process(injected_result(), &mut events))
}

fn injected_result() -> Result<(), PipelineError> {
    let message = format!("Authorization: Bearer {FIXTURE_TOKEN}; secret={FIXTURE_SECRET}");
    match env::args().nth(1).as_deref() {
        Some("auth") => Err(PipelineError::Auth(message)),
        Some("network") => Err(PipelineError::Network(message)),
        Some("api") => Err(PipelineError::ApiStructure(message)),
        Some("normalize") => Err(PipelineError::Normalize(message)),
        Some("package") => Err(PipelineError::Package(message)),
        _ => Err(PipelineError::Cli(
            "test-support runner requires an error kind".into(),
        )),
    }
}
