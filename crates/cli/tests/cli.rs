use std::process::Command;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use clap as _;
use kit as _;
use serde as _;
use serde_json::Value;
use tokio as _;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_avdkit"))
}

static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

fn plan_path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "avdkit-cli-{}-{}-{suffix}.json",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn parses_nested_commands_and_emits_json() {
    let output = command().args(["--json", "environment"]).output().unwrap();

    assert!(output.status.success(), "{output:?}");
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["schema_version"], "0.1.0");
    assert!(response["data"]["snapshot"]["host"]["platform"].is_string());
}

#[test]
fn clap_usage_errors_exit_with_two() {
    let output = command().arg("unknown-command").output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}

#[test]
fn operation_errors_exit_with_one_and_keep_the_json_contract() {
    let output = command()
        .args(["--json", "devices", "get", "../invalid"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let response: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(response["schema_version"], "0.1.0");
    assert_eq!(response["data"]["code"], "invalid_input");
}

#[test]
fn creates_shows_and_requires_approval_for_plans() {
    let path = plan_path("approval");
    let output = command()
        .args([
            "plan",
            "create",
            "--id",
            "avdkit_test_cli",
            "--profile",
            "avdkit_test_profile",
            "--image",
            "system-images;android-999;google_apis;arm64-v8a",
            "--output",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(path.is_file());

    let output = command()
        .args(["--json", "plan", "show", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["data"]["kind"], "create_device");
    assert_eq!(response["data"]["steps"].as_array().unwrap().len(), 4);

    let output = command()
        .args(["plan", "execute", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--approve"));
    fs::remove_file(path).unwrap();
}

#[test]
fn destructive_device_commands_require_approval() {
    for arguments in [
        vec!["devices", "delete", "avdkit_test_cli"],
        vec!["devices", "cleanup-tests"],
    ] {
        let output = command().args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--approve"),
            "{output:?}"
        );
    }
}

#[test]
fn json_long_operation_ends_with_an_ndjson_error_result() {
    let path = plan_path("ndjson");
    let create = command()
        .args([
            "plan",
            "create",
            "--id",
            "avdkit_test_cli_ndjson",
            "--profile",
            "avdkit_test_profile_ndjson",
            "--image",
            "system-images;android-999;google_apis;arm64-v8a",
            "--output",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(create.status.success(), "{create:?}");

    let output = command()
        .args([
            "--json",
            "plan",
            "execute",
            path.to_str().unwrap(),
            "--approve",
        ])
        .output()
        .unwrap();
    assert!(
        matches!(output.status.code(), Some(3) | Some(4)),
        "{output:?}"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let lines = String::from_utf8(output.stdout).unwrap();
    let responses = lines
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let final_response = responses.last().unwrap();
    assert_eq!(final_response["schema_version"], "0.1.0");
    assert!(matches!(
        final_response["data"]["code"].as_str(),
        Some("capability_unavailable" | "tool_not_found" | "package_not_found")
    ));
    fs::remove_file(path).unwrap();
}
