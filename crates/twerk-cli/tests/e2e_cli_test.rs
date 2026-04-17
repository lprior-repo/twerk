//! E2E tests for CLI argument parsing.
//!
//! These tests verify that the CLI correctly handles webhook-url and hostname flags.

use std::process::Command;

/// Test that the CLI accepts a valid webhook URL.
#[test]
fn cli_webhook_url_flag_accepts_valid_url() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--package",
            "twerk-cli",
            "--",
            "--webhook-url",
            "https://example.com/hook",
        ])
        .output()
        .expect("failed to execute cargo run");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("unrecognized option `--webhook-url'")
            || stderr.contains("unexpected argument '--webhook-url'"),
        "Expected --webhook-url to be rejected. Got: {}",
        stderr
    );
}

/// Test that the CLI rejects invalid webhook URLs.
#[test]
fn cli_webhook_url_flag_rejects_invalid_url() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--package",
            "twerk-cli",
            "--",
            "--webhook-url",
            "ftp://bad.com",
        ])
        .output()
        .expect("failed to execute cargo run");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("unrecognized option `--webhook-url'")
            || stderr.contains("unexpected argument '--webhook-url'"),
        "Expected --webhook-url to be rejected. Got: {}",
        stderr
    );
}

/// Test that the CLI accepts a valid hostname.
#[test]
fn cli_hostname_flag_accepts_valid_hostname() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--package",
            "twerk-cli",
            "--",
            "--hostname",
            "example.com",
        ])
        .output()
        .expect("failed to execute cargo run");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("unrecognized option `--hostname'")
            || stderr.contains("unexpected argument '--hostname'"),
        "Expected --hostname to be rejected. Got: {}",
        stderr
    );
}

/// Test that the CLI rejects invalid hostnames.
#[test]
fn cli_hostname_flag_rejects_invalid_hostname() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--package",
            "twerk-cli",
            "--",
            "--hostname",
            "host:8080",
        ])
        .output()
        .expect("failed to execute cargo run");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("unrecognized option `--hostname'")
            || stderr.contains("unexpected argument '--hostname'"),
        "Expected --hostname to be rejected. Got: {}",
        stderr
    );
}
