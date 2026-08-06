use std::{io, path::PathBuf, process::ExitCode};

use card_data_contract::DataVersion;
use card_data_pipeline::{
    finish_process, verify_image_candidate, BuildRequest, Credentials, EventSink, JsonlEventSink,
    PipelineError,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(disable_help_flag = true, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Build {
        #[arg(long)]
        data_version: String,
        #[arg(long)]
        output_root: PathBuf,
    },
    ImageBaselineBuild {
        #[arg(long)]
        package_root: PathBuf,
        #[arg(long)]
        output_root: PathBuf,
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        run_attempt: u32,
    },
    ImageBaselineVerify {
        #[arg(long)]
        candidate_root: PathBuf,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let stderr = io::stderr();
    let mut events = JsonlEventSink::new(stderr.lock());
    let result = run_cli(&mut events).await;
    ExitCode::from(finish_process(result, &mut events))
}

async fn run_cli(events: &mut dyn EventSink) -> Result<(), PipelineError> {
    let cli = Cli::try_parse().map_err(|_| PipelineError::Cli("invalid command line".into()))?;
    match cli.command {
        Command::Build {
            data_version,
            output_root,
        } => {
            let data_version = DataVersion::parse(&data_version).map_err(PipelineError::Cli)?;
            let credentials = Credentials::from_env()?;
            card_data_pipeline::run_build(
                BuildRequest {
                    data_version,
                    output_root,
                    credentials,
                },
                events,
            )
            .await?;
        }
        Command::ImageBaselineBuild {
            package_root,
            output_root,
            run_id,
            run_attempt,
        } => {
            card_data_pipeline::run_image_baseline_build(
                package_root,
                output_root,
                &run_id,
                run_attempt,
                events,
            )
            .await?;
        }
        Command::ImageBaselineVerify { candidate_root } => {
            verify_image_candidate(candidate_root)?;
        }
    }
    Ok(())
}
