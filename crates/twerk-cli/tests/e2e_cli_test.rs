//! E2E tests for twerk-cli help, version, and JSON error behavior.

use serde::Deserialize;
use std::env;
use std::process::{Command, Output};

fn cli_binary() -> String {
    if let Ok(path) = env::var("TWERK_CLI_BINARY") {
        return path;
    }
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|path| path.parent())
        .unwrap_or_else(|| std::path::Path::new("."));
    workspace_root
        .join("target")
        .join("debug")
        .join("twerk")
        .to_string_lossy()
        .to_string()
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(cli_binary())
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to execute {:?}: {error}", args))
}

fn stdout_string(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_string(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn expected_version_line(command_name: &str) -> String {
    format!("{command_name} {}\n", env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Deserialize)]
struct JsonCliOutput {
    #[serde(rename = "type")]
    output_type: String,
    version: String,
    commit: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    exit_code: Option<i32>,
}

fn parse_json_output(output: &Output) -> JsonCliOutput {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout was not valid JSON: {error}; stdout={:?}",
            output.stdout
        )
    })
}

#[test]
fn help_flag_exits_zero() {
    let output = run_cli(&["--help"]);

    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_string(&output).contains("Usage:"));
}

#[test]
fn version_flag_exits_zero() {
    let output = run_cli(&["--version"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_string(&output), expected_version_line("twerk"));
    assert_eq!(stderr_string(&output), "");
}

#[test]
fn version_subcommand_matches_top_level_version_output_without_noise() {
    let output = run_cli(&["version"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_string(&output), expected_version_line("twerk"));
    assert_eq!(stderr_string(&output), "");
}

#[test]
fn json_top_level_help_writes_help_json_to_stdout() {
    let output = run_cli(&["--json"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "success");
    assert!(!parsed.version.is_empty());
    assert!(!parsed.commit.is_empty());
    assert!(parsed.data.is_some());
    let data_binding = parsed.data.unwrap();
    let data_str = data_binding.as_str().unwrap_or_default();
    assert!(data_str.contains("Usage:"));
}

#[test]
fn json_help_flag_returns_rendered_help_content() {
    let output = run_cli(&["--json", "--help"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "help");
    assert!(parsed.content.unwrap_or_default().contains("Usage:"));
}

#[test]
fn help_subcommand_in_json_mode_returns_rendered_help_content() {
    let output = run_cli(&["help", "--json"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "help");
    assert!(parsed.content.unwrap_or_default().contains("Usage:"));
}

#[test]
fn run_help_in_json_mode_returns_rendered_help_content() {
    let output = run_cli(&["run", "--json", "--help"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "help");
    assert!(parsed.content.unwrap_or_default().contains("Usage:"));
}

#[test]
fn json_version_flag_writes_version_json_to_stdout() {
    let output = run_cli(&["--json", "--version"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "version");
    assert!(parsed.content.unwrap_or_default().contains("twerk"));
}

#[test]
fn version_subcommand_supports_json_mode() {
    let output = run_cli(&["version", "--json"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "version");
    assert!(parsed.content.unwrap_or_default().contains("twerk"));
}

#[test]
fn propagated_subcommand_version_forms_remain_clean() {
    [
        (["run", "--version"], "twerk-run"),
        (["migration", "--version"], "twerk-migration"),
        (["health", "--version"], "twerk-health"),
    ]
    .into_iter()
    .for_each(|(args, command_name)| {
        let output = run_cli(&args);

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(stdout_string(&output), expected_version_line(command_name));
        assert_eq!(stderr_string(&output), "");
    });
}

#[test]
fn json_invalid_run_mode_preserves_clap_exit_code() {
    let output = run_cli(&["--json", "run", "invalid-mode"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "error");
    assert_eq!(parsed.kind.as_deref(), Some("invalid_value"));
    assert!(parsed.message.unwrap_or_default().contains("invalid value"));
}

#[test]
fn json_missing_run_mode_preserves_clap_exit_code() {
    let output = run_cli(&["--json", "run"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "error");
    assert_eq!(parsed.kind.as_deref(), Some("missing_required_argument"));
    assert!(parsed.message.unwrap_or_default().contains("required"));
}

#[test]
fn json_invalid_health_endpoint_writes_structured_validation_error() {
    let output = run_cli(&["--json", "health", "--endpoint", "not-a-url"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "error");
    assert_eq!(parsed.kind.as_deref(), Some("validation"));
    assert!(parsed
        .message
        .unwrap_or_default()
        .contains("invalid endpoint"));
}

#[test]
fn json_health_connection_failure_writes_structured_runtime_error() {
    let output = run_cli(&["--json", "health", "--endpoint", "http://127.0.0.1:9"]);
    let parsed = parse_json_output(&output);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr_string(&output), "");
    assert_eq!(parsed.output_type, "error");
    assert_eq!(parsed.kind.as_deref(), Some("runtime"));
    assert!(parsed
        .message
        .unwrap_or_default()
        .contains("HTTP request failed"));
}
