//! Tests for --format flag (json|table|quiet) across CLI commands.
//!
//! Bead: tw-0q5j - Test output formatting respects --format flag
//! Spec: 1) --format json -> valid JSON, 2) --format table -> aligned columns with headers,
//!       3) --format quiet -> only IDs one per line, 4) invalid format -> error message

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

#[derive(Debug, Deserialize)]
struct JsonCliOutput {
    #[serde(rename = "type")]
    output_type: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    commit: Option<String>,
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

fn is_valid_json(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

mod queue_format_tests {
    use super::*;

    #[test]
    fn queue_list_format_json_outputs_valid_json() {
        let output = run_cli(&["--format", "json", "queue", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        assert!(
            stderr_string(&output).is_empty(),
            "expected empty stderr, got: {}",
            stderr_string(&output)
        );
        assert!(
            is_valid_json(&stdout_string(&output)),
            "expected valid JSON output, got: {}",
            stdout_string(&output)
        );
    }

    #[test]
    fn queue_list_format_table_outputs_aligned_columns_with_headers() {
        let output = run_cli(&["--format", "table", "queue", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        let stdout = stdout_string(&output);
        assert!(
            stdout.contains("NAME") && stdout.contains("SIZE"),
            "expected table headers (NAME, SIZE), got: {}",
            stdout
        );
        assert!(
            stdout.contains('-') || stdout.contains("---"),
            "expected column separators in table output, got: {}",
            stdout
        );
    }

    #[test]
    fn queue_list_format_quiet_outputs_only_ids_one_per_line() {
        let output = run_cli(&["--format", "quiet", "queue", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        let stdout = stdout_string(&output);
        let lines: Vec<&str> = stdout.lines().collect();
        assert!(
            !lines.is_empty(),
            "expected non-empty output in quiet mode"
        );
        for line in &lines {
            assert!(
                !line.contains(' '),
                "expected no spaces in quiet mode output (IDs only), got: {}",
                line
            );
        }
    }

    #[test]
    fn queue_list_invalid_format_returns_error() {
        let output = run_cli(&["--format", "invalid", "queue", "list"]);
        assert_ne!(
            output.status.code(),
            Some(0),
            "expected non-zero exit code for invalid format"
        );
        let stderr = stderr_string(&output);
        let stdout = stdout_string(&output);
        assert!(
            stderr.contains("invalid") || stderr.contains("error")
                || stdout.contains("invalid") || stdout.contains("error"),
            "expected error message for invalid format, got stderr: {}, stdout: {}",
            stderr,
            stdout
        );
    }
}

mod trigger_format_tests {
    use super::*;

    #[test]
    fn trigger_list_format_json_outputs_valid_json() {
        let output = run_cli(&["--format", "json", "trigger", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        assert!(
            stderr_string(&output).is_empty(),
            "expected empty stderr, got: {}",
            stderr_string(&output)
        );
        assert!(
            is_valid_json(&stdout_string(&output)),
            "expected valid JSON output, got: {}",
            stdout_string(&output)
        );
    }

    #[test]
    fn trigger_list_format_table_outputs_aligned_columns_with_headers() {
        let output = run_cli(&["--format", "table", "trigger", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        let stdout = stdout_string(&output);
        assert!(
            stdout.contains("ID") && stdout.contains("NAME"),
            "expected table headers (ID, NAME), got: {}",
            stdout
        );
    }

    #[test]
    fn trigger_list_format_quiet_outputs_only_ids_one_per_line() {
        let output = run_cli(&["--format", "quiet", "trigger", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        let stdout = stdout_string(&output);
        let lines: Vec<&str> = stdout.lines().collect();
        assert!(
            !lines.is_empty(),
            "expected non-empty output in quiet mode"
        );
        for line in &lines {
            assert!(
                !line.contains(' '),
                "expected no spaces in quiet mode output (IDs only), got: {}",
                line
            );
        }
    }

    #[test]
    fn trigger_list_invalid_format_returns_error() {
        let output = run_cli(&["--format", "invalid", "trigger", "list"]);
        assert_ne!(
            output.status.code(),
            Some(0),
            "expected non-zero exit code for invalid format"
        );
    }
}

mod node_format_tests {
    use super::*;

    #[test]
    fn node_list_format_json_outputs_valid_json() {
        let output = run_cli(&["--format", "json", "node", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        assert!(
            stderr_string(&output).is_empty(),
            "expected empty stderr, got: {}",
            stderr_string(&output)
        );
        assert!(
            is_valid_json(&stdout_string(&output)),
            "expected valid JSON output, got: {}",
            stdout_string(&output)
        );
    }

    #[test]
    fn node_list_format_table_outputs_aligned_columns_with_headers() {
        let output = run_cli(&["--format", "table", "node", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        let stdout = stdout_string(&output);
        assert!(
            stdout.contains("ID") && stdout.contains("NAME"),
            "expected table headers (ID, NAME), got: {}",
            stdout
        );
    }

    #[test]
    fn node_list_format_quiet_outputs_only_ids_one_per_line() {
        let output = run_cli(&["--format", "quiet", "node", "list"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        let stdout = stdout_string(&output);
        let lines: Vec<&str> = stdout.lines().collect();
        assert!(
            !lines.is_empty(),
            "expected non-empty output in quiet mode"
        );
        for line in &lines {
            assert!(
                !line.contains(' '),
                "expected no spaces in quiet mode output (IDs only), got: {}",
                line
            );
        }
    }

    #[test]
    fn node_list_invalid_format_returns_error() {
        let output = run_cli(&["--format", "invalid", "node", "list"]);
        assert_ne!(
            output.status.code(),
            Some(0),
            "expected non-zero exit code for invalid format"
        );
    }
}

mod task_format_tests {
    use super::*;

    #[test]
    fn task_get_format_json_outputs_valid_json() {
        let output = run_cli(&["--format", "json", "task", "get", "test-task-id"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        assert!(
            stderr_string(&output).is_empty(),
            "expected empty stderr, got: {}",
            stderr_string(&output)
        );
        assert!(
            is_valid_json(&stdout_string(&output)),
            "expected valid JSON output, got: {}",
            stdout_string(&output)
        );
    }

    #[test]
    fn task_get_format_table_outputs_human_readable_format() {
        let output = run_cli(&["--format", "table", "task", "get", "test-task-id"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "expected exit 0, got {:?}",
            output.status.code()
        );
        let stdout = stdout_string(&output);
        assert!(
            !is_valid_json(&stdout),
            "expected non-JSON (human-readable) output in table mode"
        );
    }

    #[test]
    fn task_get_invalid_format_returns_error() {
        let output = run_cli(&["--format", "invalid", "task", "get", "test-task-id"]);
        assert_ne!(
            output.status.code(),
            Some(0),
            "expected non-zero exit code for invalid format"
        );
    }
}