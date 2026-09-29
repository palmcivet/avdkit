use std::process::Command;

use clap as _;
use kit as _;
use serde as _;
use serde_json::Value;
use tokio as _;

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_avdkit"))
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

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let response: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(response["schema_version"], "0.1.0");
    assert_eq!(response["data"]["code"], "invalid_input");
}
