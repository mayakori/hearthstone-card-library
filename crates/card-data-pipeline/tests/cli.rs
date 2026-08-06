use std::{fs, path::Path};

use assert_cmd::Command;
use card_data_pipeline::{exit_code, Event, EventSink, JsonlEventSink, PipelineError};
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const VERSION: &str = "36.0.3-build247416-r1";
const CLIENT_ID: &str = "cli-test-client-id";
const CLIENT_SECRET: &str = "cli-test-client-secret";
const TOKEN: &str = "fixture-access-token";

#[test]
fn missing_credentials_exits_two_with_secret_safe_jsonl() {
    let output = tempfile::tempdir().unwrap();
    let assertion = binary()
        .env_remove("BLIZZARD_CLIENT_ID")
        .env_remove("BLIZZARD_CLIENT_SECRET")
        .args(build_args(VERSION, output.path()))
        .assert()
        .code(2);

    assert_jsonl_failure(&assertion.get_output().stderr, 2);
    assert!(assertion.get_output().stdout.is_empty());
}

#[test]
fn invalid_data_version_exits_two_with_jsonl() {
    let output = tempfile::tempdir().unwrap();
    let assertion = binary()
        .env("BLIZZARD_CLIENT_ID", CLIENT_ID)
        .env("BLIZZARD_CLIENT_SECRET", CLIENT_SECRET)
        .args(build_args("latest", output.path()))
        .assert()
        .code(2);

    assert_jsonl_failure(&assertion.get_output().stderr, 2);
    assert!(assertion.get_output().stdout.is_empty());
}

#[test]
fn existing_output_exits_two_before_network_access() {
    let output = tempfile::tempdir().unwrap();
    fs::create_dir_all(output.path().join(VERSION)).unwrap();
    let assertion = binary()
        .env("BLIZZARD_CLIENT_ID", CLIENT_ID)
        .env("BLIZZARD_CLIENT_SECRET", CLIENT_SECRET)
        .args(build_args(VERSION, output.path()))
        .assert()
        .code(2);

    assert_jsonl_failure(&assertion.get_output().stderr, 2);
    assert!(assertion.get_output().stdout.is_empty());
}

#[test]
fn clap_parse_error_is_jsonl_and_never_writes_stdout() {
    let assertion = binary().arg("unexpected").assert().code(2);

    assert_jsonl_failure(&assertion.get_output().stderr, 2);
    assert!(assertion.get_output().stdout.is_empty());
}

#[test]
fn image_baseline_build_does_not_require_blizzard_credentials_and_validates_package() {
    let output = tempfile::tempdir().unwrap();
    let missing_package = output.path().join("missing-package");
    let assertion = binary()
        .env_remove("BLIZZARD_CLIENT_ID")
        .env_remove("BLIZZARD_CLIENT_SECRET")
        .args([
            "image-baseline-build",
            "--package-root",
            &missing_package.display().to_string(),
            "--output-root",
            &output.path().display().to_string(),
            "--run-id",
            "12345",
            "--run-attempt",
            "1",
        ])
        .assert()
        .code(7);

    assert_jsonl_failure(&assertion.get_output().stderr, 7);
    assert!(assertion.get_output().stdout.is_empty());
}

#[test]
fn typed_pipeline_errors_have_the_stable_exit_codes() {
    for (error, expected) in [
        (PipelineError::Cli("x".into()), 2),
        (PipelineError::Config("x".into()), 2),
        (PipelineError::Auth("x".into()), 3),
        (PipelineError::Network("x".into()), 4),
        (PipelineError::ApiStructure("x".into()), 5),
        (PipelineError::Normalize("x".into()), 6),
        (PipelineError::Sqlite("x".into()), 6),
        (PipelineError::Package("x".into()), 7),
        (PipelineError::Io("x".into()), 7),
    ] {
        assert_eq!(exit_code(&error), expected);
    }
}

#[cfg(feature = "test-support")]
#[test]
fn test_support_runner_maps_every_typed_failure_to_a_safe_process_contract() {
    for (kind, expected_exit) in [
        ("auth", 3),
        ("network", 4),
        ("api", 5),
        ("normalize", 6),
        ("package", 7),
    ] {
        let assertion = test_support_runner().arg(kind).assert().code(expected_exit);
        assert_jsonl_failure(&assertion.get_output().stderr, expected_exit);
        assert!(assertion.get_output().stdout.is_empty());
    }
}

#[test]
fn jsonl_retry_event_contains_only_safe_scalars() {
    let mut sink = JsonlEventSink::new(Vec::new());
    sink.emit(Event::retry(2, Some(503)).unwrap()).unwrap();

    let bytes = sink.into_inner();
    let event: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(event["level"], "warn");
    assert_eq!(event["stage"], "collect");
    assert_eq!(event["event"], "retry");
    assert_eq!(event["attempt"], 2);
    assert_eq!(event["status_code"], 503);
    for field in event.as_object().unwrap().keys() {
        assert!(
            [
                "schema_version",
                "timestamp",
                "level",
                "stage",
                "event",
                "locale",
                "attempt",
                "status_code",
                "counts",
                "error_code",
                "message",
            ]
            .contains(&field.as_str()),
            "unexpected log field: {field}"
        );
    }
}

fn binary() -> Command {
    Command::cargo_bin("card-data-pipeline").unwrap()
}

#[cfg(feature = "test-support")]
fn test_support_runner() -> Command {
    Command::cargo_bin("card-data-pipeline-test-runner").unwrap()
}

fn build_args(version: &str, output: &Path) -> [String; 5] {
    [
        "build".into(),
        "--data-version".into(),
        version.into(),
        "--output-root".into(),
        output.display().to_string(),
    ]
}

fn assert_jsonl_failure(stderr: &[u8], exit_code: i32) {
    let text = String::from_utf8(stderr.to_vec()).unwrap();
    assert!(
        !text.trim().is_empty(),
        "stderr must contain a JSONL failure event"
    );
    assert!(!text.contains(CLIENT_ID));
    assert!(!text.contains(CLIENT_SECRET));
    assert!(!text.contains(TOKEN));
    assert!(!text.contains("Authorization"));
    let events = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    for event in &events {
        let object = event.as_object().unwrap();
        assert_eq!(object["schema_version"], 1);
        OffsetDateTime::parse(object["timestamp"].as_str().unwrap(), &Rfc3339).unwrap();
        for field in object.keys() {
            assert!(
                [
                    "schema_version",
                    "timestamp",
                    "level",
                    "stage",
                    "event",
                    "locale",
                    "attempt",
                    "status_code",
                    "counts",
                    "error_code",
                    "message",
                ]
                .contains(&field.as_str()),
                "unexpected log field: {field}"
            );
        }
    }
    let final_event = events.last().unwrap().as_object().unwrap();
    assert_eq!(final_event["level"], "error");
    assert_eq!(final_event["stage"], "final");
    assert_eq!(final_event["event"], "failure");
    assert_eq!(final_event["error_code"], exit_code);
    assert_eq!(final_event["message"], "build failed");
}

#[test]
fn jsonl_image_retry_event_uses_the_image_stage_and_safe_scalars() {
    let mut sink = JsonlEventSink::new(Vec::new());
    sink.emit(Event::image_retry(1, Some(503)).unwrap())
        .unwrap();

    let event: Value = serde_json::from_slice(&sink.into_inner()).unwrap();
    assert_eq!(event["level"], "warn");
    assert_eq!(event["stage"], "image_download");
    assert_eq!(event["event"], "retry");
    assert_eq!(event["attempt"], 1);
    assert_eq!(event["status_code"], 503);
}
