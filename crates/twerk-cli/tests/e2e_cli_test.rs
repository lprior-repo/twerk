//! E2E tests for twerk-cli contract and behavioral inventory.
//!
//! These tests verify the CLI contract and behavioral inventory from the
//! Red Queen test plan for twerk-cli.

use serde::Deserialize;
use std::env;
use std::process::Command;

fn cli_binary() -> String {
    if let Ok(path) = env::var("TWERK_CLI_BINARY") {
        return path;
    }
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| std::path::Path::new("."));
    workspace_root
        .join("target")
        .join("debug")
        .join("twerk")
        .to_string_lossy()
        .to_string()
}

#[derive(Debug, Deserialize)]
struct JsonVersionOutput {
    #[serde(rename = "type")]
    output_type: String,
    version: String,
    commit: String,
}

mod contract {
    use super::*;

    #[test]
    fn c1_help_command_exits_zero() {
        let output = Command::new(&cli_binary())
            .arg("--help")
            .output()
            .expect("failed to execute twerk --help");
        assert_eq!(output.status.code(), Some(0), "C1: --help should exit 0");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage:"), "C1: help should contain Usage");
    }

    #[test]
    fn c2_version_command_exits_zero() {
        let output = Command::new(&cli_binary())
            .arg("--version")
            .output()
            .expect("failed to execute twerk --version");
        assert_eq!(output.status.code(), Some(0), "C2: --version should exit 0");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("twerk"),
            "C2: version output should contain 'twerk'"
        );
    }

    #[test]
    fn c3_json_flag_exits_zero() {
        let output = Command::new(&cli_binary())
            .arg("--json")
            .output()
            .expect("failed to execute twerk --json");
        assert_eq!(output.status.code(), Some(0), "C3: --json should exit 0");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("\"type\":\"help\""),
            "C3: JSON output should have type:help"
        );
        assert!(
            stdout.contains("\"version\""),
            "C3: JSON output should have version"
        );
    }

    #[test]
    fn c4_run_subcommand_help_exits_zero() {
        let output = Command::new(&cli_binary())
            .args(["run", "--help"])
            .output()
            .expect("failed to execute twerk run --help");
        assert_eq!(
            output.status.code(),
            Some(0),
            "C4: run --help should exit 0"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Usage:"),
            "C4: run help should contain Usage"
        );
    }

    #[test]
    fn c5_migration_subcommand_help_exits_zero() {
        let output = Command::new(&cli_binary())
            .args(["migration", "--help"])
            .output()
            .expect("failed to execute twerk migration --help");
        assert_eq!(
            output.status.code(),
            Some(0),
            "C5: migration --help should exit 0"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Usage:"),
            "C5: migration help should contain Usage"
        );
    }

    #[test]
    fn c6_health_subcommand_help_exits_zero() {
        let output = Command::new(&cli_binary())
            .args(["health", "--help"])
            .output()
            .expect("failed to execute twerk health --help");
        assert_eq!(
            output.status.code(),
            Some(0),
            "C6: health --help should exit 0"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Usage:"),
            "C6: health help should contain Usage"
        );
    }
}

mod behavioral {
    use super::*;

    #[test]
    fn b1_no_command_shows_help_and_exits_zero() {
        let output = Command::new(&cli_binary())
            .output()
            .expect("failed to execute bare twerk");
        assert_eq!(
            output.status.code(),
            Some(0),
            "B1: no command should exit 0"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage:"), "B1: output should show help");
    }

    #[test]
    fn b2_help_flag_shows_help_and_exits_zero() {
        let output = Command::new(&cli_binary())
            .arg("--help")
            .output()
            .expect("failed to execute twerk --help");
        assert_eq!(output.status.code(), Some(0), "B2: --help should exit 0");
    }

    #[test]
    fn b3_version_flag_shows_version_and_exits_zero() {
        let output = Command::new(&cli_binary())
            .arg("--version")
            .output()
            .expect("failed to execute twerk --version");
        assert_eq!(output.status.code(), Some(0), "B3: --version should exit 0");
    }

    #[test]
    fn b4_json_flag_shows_json_and_exits_zero() {
        let output = Command::new(&cli_binary())
            .arg("--json")
            .output()
            .expect("failed to execute twerk --json");
        assert_eq!(output.status.code(), Some(0), "B4: --json should exit 0");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: JsonVersionOutput =
            serde_json::from_str(&stdout).expect("B4: JSON output should be valid JSON");
        assert_eq!(parsed.output_type, "help", "B4: type should be 'help'");
        assert!(
            !parsed.version.is_empty(),
            "B4: version should not be empty"
        );
    }

    #[test]
    fn b8_invalid_run_mode_exits_2() {
        let output = Command::new(&cli_binary())
            .args(["run", "invalid-mode"])
            .output()
            .expect("failed to execute twerk run invalid-mode");
        assert_eq!(
            output.status.code(),
            Some(2),
            "B8: invalid mode should exit 2"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("invalid value"),
            "B8: error should mention 'invalid value'"
        );
    }

    #[test]
    fn b9_run_without_mode_exits_2() {
        let output = Command::new(&cli_binary())
            .args(["run"])
            .output()
            .expect("failed to execute twerk run");
        assert_eq!(
            output.status.code(),
            Some(2),
            "B9: run without mode should exit 2"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("required"),
            "B9: error should mention 'required'"
        );
    }

    #[test]
    fn b11_migration_without_yes_exits_1() {
        let output = Command::new(&cli_binary())
            .args(["migration"])
            .output()
            .expect("failed to execute twerk migration");
        assert_eq!(
            output.status.code(),
            Some(1),
            "B11: migration without --yes should exit 1"
        );
    }

    #[test]
    fn b13_health_empty_endpoint_exits_1() {
        let output = Command::new(&cli_binary())
            .args(["health", "--endpoint", ""])
            .output()
            .expect("failed to execute twerk health --endpoint ''");
        assert_eq!(
            output.status.code(),
            Some(1),
            "B13: empty endpoint should exit 1"
        );
    }

    #[test]
    fn b14_health_invalid_url_exits_1() {
        let output = Command::new(&cli_binary())
            .args(["health", "--endpoint", "not-a-url"])
            .output()
            .expect("failed to execute twerk health --endpoint 'not-a-url'");
        assert_eq!(
            output.status.code(),
            Some(1),
            "B14: invalid URL should exit 1"
        );
    }
}

mod error_handling {
    use super::*;

    #[test]
    fn e1_invalid_mode_arg_exits_2() {
        let output = Command::new(&cli_binary())
            .args(["run", "bogus"])
            .output()
            .expect("failed to execute twerk run bogus");
        assert_eq!(
            output.status.code(),
            Some(2),
            "E1: clap error should exit 2"
        );
    }

    #[test]
    fn e2_missing_required_arg_exits_2() {
        let output = Command::new(&cli_binary())
            .args(["run"])
            .output()
            .expect("failed to execute twerk run");
        assert_eq!(
            output.status.code(),
            Some(2),
            "E2: missing arg should exit 2"
        );
    }

    #[test]
    fn e4_json_mode_suppresses_banner() {
        let output = Command::new(&cli_binary())
            .args(["--json", "run", "standalone"])
            .output()
            .expect("failed to execute twerk --json run standalone");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stdout.contains("twerk"),
            "E4: JSON output should not contain banner 'twerk'"
        );
        assert!(
            !stderr.contains("twerk"),
            "E4: stderr should not contain banner"
        );
        assert!(
            stdout.contains("error") || stdout.contains("Error") || output.status.code() != Some(0),
            "E4: Should produce error JSON (no RabbitMQ)"
        );
    }

    #[test]
    fn e5_json_mode_suppresses_logging() {
        let output = Command::new(&cli_binary())
            .args(["--json", "run", "standalone"])
            .output()
            .expect("failed to execute twerk --json run standalone");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains(" INFO"),
            "E5: JSON mode should not contain INFO logs"
        );
        assert!(
            !stderr.contains(" WARN"),
            "E5: JSON mode should not contain WARN logs"
        );
    }
}

mod json_output {
    use super::*;

    #[test]
    fn json_help_contains_version_info() {
        let output = Command::new(&cli_binary())
            .arg("--json")
            .output()
            .expect("failed to execute twerk --json");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: JsonVersionOutput =
            serde_json::from_str(&stdout).expect("JSON output should parse as JsonVersionOutput");
        assert!(!parsed.version.is_empty());
        assert!(!parsed.commit.is_empty());
    }

    #[test]
    fn json_health_endpoint_not_found_exits_1() {
        let output = Command::new(&cli_binary())
            .args(["--json", "health", "--endpoint", "http://localhost:9999"])
            .output()
            .expect("failed to execute twerk --json health");
        assert_eq!(
            output.status.code(),
            Some(1),
            "health check to non-existent endpoint should exit 1"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("{"), "JSON output expected");
    }
}
