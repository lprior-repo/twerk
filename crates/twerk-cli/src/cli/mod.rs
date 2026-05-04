//! Main CLI entry point
//!
//! Orchestrates the CLI: parses arguments, displays banner, and dispatches commands.

mod dispatch;
mod help;

use serde_json::json;
use std::ffi::OsString;
use tracing::Level;
use tracing_subscriber::{fmt, fmt::format::FmtSpan, prelude::*, EnvFilter};
use twerk_common::load_config;
use twerk_core::domain::{Dsn, Endpoint};
use twerk_infrastructure::config as app_config;
use twerk_infrastructure::reexec;

use crate::banner::{display_banner, BannerMode};
use crate::commands::Commands;
use crate::error::CliError;
use crate::migrate::DEFAULT_POSTGRES_DSN;

use dispatch::{
    execute_command, handle_json_help_subcommand, handle_parse_error, handle_runtime_error,
    parse_cli_args,
};
use help::{json_success_payload, print_json, render_top_level_help, write_help_to_stdout};

/// Default endpoint for health checks
pub const DEFAULT_ENDPOINT: &str = "http://localhost:8000";

/// Default datastore type
pub const DEFAULT_DATASTORE_TYPE: &str = "postgres";

/// Twerk version string
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash set at build time
pub const GIT_COMMIT: &str = env!("GIT_COMMIT_HASH");

/// Parsed top-level CLI action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CliAction {
    Execute(Option<Commands>, bool, bool), // (command, json_mode, quiet)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum ExitStatus {
    Success = 0,
    Failure = 1,
}

/// Get the current git commit hash at runtime
#[must_use]
pub fn get_git_commit() -> String {
    get_config_string("cli.git_sha").unwrap_or_else(|| "unknown".to_string())
}

/// Setup logging based on configuration
///
/// # Errors
///
/// Returns [`CliError::Logging`] if the log level is invalid.
pub fn setup_logging() -> Result<(), CliError> {
    let log_level_str = get_config_string("logging.level").unwrap_or_else(|| String::from("info"));

    let level: Level = log_level_str
        .parse()
        .map_err(|_| CliError::Logging(format!("invalid log level: {log_level_str}")))?;

    let level_directive: tracing_subscriber::filter::Directive = level.into();
    let filter = EnvFilter::try_from_default_env().map_or_else(
        |_| EnvFilter::new(&log_level_str),
        |env| env.add_directive(level_directive),
    );

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_span_events(FmtSpan::CLOSE)
                .with_target(true)
                .with_file(true)
                .with_line_number(true),
        )
        .with(filter)
        .try_init()
        .or_else(|error| {
            if error
                .to_string()
                .contains("global default trace dispatcher has already been set")
            {
                Ok(())
            } else {
                Err(CliError::Logging(format!("logging setup error: {error}")))
            }
        })?;

    Ok(())
}

/// Get banner mode from configuration
fn get_banner_mode() -> BannerMode {
    std::env::var("TWERK_CLI_BANNER_MODE").ok().map_or_else(
        || {
            get_config_string("cli.banner.mode")
                .map_or_else(BannerMode::default, |value| BannerMode::from_str(&value))
        },
        |value| BannerMode::from_str(&value),
    )
}

/// Get endpoint from configuration or default
pub(super) fn get_endpoint() -> Result<Endpoint, twerk_core::domain::EndpointError> {
    get_config_string("client.endpoint")
        .or_else(|| get_config_string("endpoint"))
        .map(Endpoint::new)
        .unwrap_or_else(|| Endpoint::new(DEFAULT_ENDPOINT))
}

/// Get datastore type from configuration or default
pub(super) fn get_datastore_type() -> String {
    get_config_string("datastore.type").unwrap_or_else(|| DEFAULT_DATASTORE_TYPE.to_string())
}

/// Get `PostgreSQL` DSN from configuration or default
pub(super) fn get_postgres_dsn() -> Result<Dsn, twerk_core::domain::DsnError> {
    get_config_string("datastore.postgres.dsn")
        .map(Dsn::new)
        .unwrap_or_else(|| Dsn::new(DEFAULT_POSTGRES_DSN))
}

/// Get a string config value, checking config file first, then environment variables.
/// Environment variables are prefixed with `TWERK_` and use single underscore for nesting.
/// e.g., `TWERK_DATASTORE_POSTGRES_DSN` for `datastore.postgres.dsn`.
fn get_config_string(key: &str) -> Option<String> {
    let _ = load_config();
    match app_config::string(key) {
        value if value.is_empty() => None,
        value => Some(value),
    }
}

fn collect_args() -> Vec<OsString> {
    std::env::args_os().collect()
}

pub(super) fn json_requested(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "--json")
}

pub(super) fn os_string_eq(value: &OsString, expected: &str) -> bool {
    value == expected
}

pub(super) const fn should_emit_startup_ui(command: &Commands, json_mode: bool) -> bool {
    !json_mode && !matches!(command, Commands::Version)
}

/// Execute the CLI with the given command
pub async fn run() -> i32 {
    if reexec::init() {
        return ExitStatus::Success as i32;
    }

    let args = collect_args();
    if let Some(exit_code) = handle_json_help_subcommand(&args) {
        return exit_code;
    }
    let emit_json = json_requested(&args);
    let action = match parse_cli_args(&args) {
        Ok(action) => action,
        Err(error) => return handle_parse_error(error, emit_json),
    };

    let (cmd, json_mode, quiet) = match action {
        CliAction::Execute(cmd, json, quiet) => (cmd, json, quiet),
    };

    // If no subcommand was provided, display help and exit 0
    let cmd = match cmd {
        Some(cmd) => cmd,
        None => match render_top_level_help() {
            Ok(content) => {
                if json_mode {
                    print_json(&json_success_payload("help", json!(content)));
                } else {
                    write_help_to_stdout(&content);
                }
                return ExitStatus::Success as i32;
            }
            Err(error) => {
                return handle_runtime_error(error, json_mode);
            }
        },
    };

    // Setup logging and banner for interactive commands only.
    if should_emit_startup_ui(&cmd, json_mode) && !quiet {
        if let Err(error) = setup_logging() {
            return handle_runtime_error(error, false);
        }
        let banner_mode = get_banner_mode();
        display_banner(banner_mode, VERSION, GIT_COMMIT);
    }

    let execution = execute_command(cmd, json_mode).await;

    match execution {
        Ok(()) => ExitStatus::Success as i32,
        Err(error) => handle_runtime_error(error, json_mode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::DEFAULT_POSTGRES_DSN;

    #[test]
    fn default_endpoint_is_localhost_http() {
        assert_eq!(DEFAULT_ENDPOINT, "http://localhost:8000");
        assert!(DEFAULT_ENDPOINT.starts_with("http://"));
        assert!(DEFAULT_ENDPOINT.contains("localhost"));
    }

    #[test]
    fn default_datastore_type_is_postgres() {
        assert_eq!(DEFAULT_DATASTORE_TYPE, "postgres");
    }

    #[test]
    fn default_postgres_dsn_contains_localhost() {
        assert!(DEFAULT_POSTGRES_DSN.contains("localhost"));
    }

    #[test]
    fn version_constant_is_not_empty() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn git_commit_constant_is_not_empty() {
        let commit = GIT_COMMIT;
        assert!(!commit.is_empty());
    }

    #[test]
    fn get_git_commit_returns_non_empty_string() {
        let commit = get_git_commit();
        assert!(!commit.is_empty());
    }

    #[test]
    fn constants_are_accessible_without_mutation() {
        // Verify constants are accessible without mutation
        let _ep = DEFAULT_ENDPOINT;
        let _dst = DEFAULT_DATASTORE_TYPE;
        let _pg_dsn = DEFAULT_POSTGRES_DSN;
        let _ver = VERSION;
        let _git = GIT_COMMIT;
    }

    #[test]
    fn get_endpoint_reads_client_endpoint_from_environment_override() {
        std::env::set_var("TWERK_CLIENT_ENDPOINT", "http://127.0.0.1:9999");

        let endpoint = match get_endpoint() {
            Ok(endpoint) => endpoint,
            Err(error) => panic!("expected endpoint override to parse: {error}"),
        };

        std::env::remove_var("TWERK_CLIENT_ENDPOINT");
        assert_eq!(endpoint.as_str(), "http://127.0.0.1:9999");
    }

    #[test]
    fn version_subcommand_skips_startup_ui_in_text_mode() {
        assert!(!should_emit_startup_ui(&Commands::Version, false));
    }

    #[test]
    fn health_command_emits_startup_ui_in_text_mode() {
        assert!(should_emit_startup_ui(
            &Commands::Health { endpoint: None },
            false
        ));
    }

    #[test]
    fn json_mode_skips_startup_ui_for_all_commands() {
        assert!(!should_emit_startup_ui(
            &Commands::Health { endpoint: None },
            true
        ));
    }
}
