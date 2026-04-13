//! Main CLI entry point
//!
//! Orchestrates the CLI: parses arguments, displays banner, and dispatches commands.

use clap::{CommandFactory, Parser};
use std::ffi::OsString;
use tracing::Level;
use tracing_subscriber::{fmt, fmt::format::FmtSpan, prelude::*, EnvFilter};
use twerk_common::load_config;
use twerk_infrastructure::config as app_config;
use twerk_infrastructure::reexec;

use super::banner::{display_banner, BannerMode};
use super::commands::{Cli, Commands};
use super::error::CliError;
use super::health::health_check;
use super::migrate::{run_migration, DEFAULT_POSTGRES_DSN};
use super::run::run_engine;

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
enum CliAction {
    Execute(Option<Commands>, bool), // (command, json_mode)
}

/// Get the current git commit hash at runtime
///
/// # Errors
///
/// Returns [`CliError::Logging`] if the log level is invalid.
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
        .init();

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
fn get_endpoint() -> String {
    get_config_string("client.endpoint")
        .or_else(|| get_config_string("endpoint"))
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string())
}

/// Get datastore type from configuration or default
fn get_datastore_type() -> String {
    get_config_string("datastore.type").unwrap_or_else(|| DEFAULT_DATASTORE_TYPE.to_string())
}

/// Get `PostgreSQL` DSN from configuration or default
fn get_postgres_dsn() -> String {
    get_config_string("datastore.postgres.dsn").unwrap_or_else(|| DEFAULT_POSTGRES_DSN.to_string())
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

fn parse_cli_args(args: &[OsString]) -> Result<CliAction, clap::Error> {
    Cli::try_parse_from(args.iter().cloned()).map(|cli| CliAction::Execute(cli.command, cli.json))
}

fn exit_for_parse_error(error: clap::Error) -> ! {
    error.exit()
}

/// Execute the CLI with the given command
///
/// # Errors
///
/// Returns an error if command execution fails.
pub async fn run() -> Result<(), CliError> {
    if reexec::init() {
        return Ok(());
    }

    let _ = load_config();

    let args = collect_args();
    let action = match parse_cli_args(&args) {
        Ok(action) => action,
        Err(error) => exit_for_parse_error(error),
    };

    let (cmd, json_mode) = match action {
        CliAction::Execute(cmd, json) => (cmd, json),
    };

    // If no subcommand was provided, display help and exit 0
    let cmd = match cmd {
        Some(cmd) => cmd,
        None => {
            if json_mode {
                println!(
                    r#"{{"type":"help","version":"{}","commit":"{}"}}"#,
                    VERSION, GIT_COMMIT
                );
            } else {
                Cli::command().print_help().map_err(CliError::Io)?;
            }
            std::process::exit(0);
        }
    };

    // Setup logging (suppress in json mode for cleaner output)
    if !json_mode {
        setup_logging()?;
    }

    // Display banner only in non-json mode
    if !json_mode {
        let banner_mode = get_banner_mode();
        display_banner(banner_mode, VERSION, GIT_COMMIT);
    }

    match cmd {
        Commands::Run { mode } => {
            run_engine(mode).await?;
        }
        Commands::Migration { yes: _ } => {
            let dstype = get_datastore_type();
            let dsn = get_postgres_dsn();
            run_migration(&dstype, &dsn).await?;
        }
        Commands::Health { endpoint } => {
            let ep = endpoint.unwrap_or_else(get_endpoint);
            health_check(&ep, json_mode).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    use clap::error::ErrorKind;

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
    fn parse_cli_args_returns_execute_none_when_subcommand_missing() {
        let args = vec![OsString::from("twerk")];

        match parse_cli_args(&args) {
            Ok(CliAction::Execute(None, false)) => {
                // No subcommand provided - help will be shown and exit 0 in run()
            }
            other => unreachable!(
                "expected Ok(CliAction::Execute(None, false)), got {:?}",
                other
            ),
        }
    }

    #[test]
    fn parse_cli_args_returns_display_version_error_when_version_flag_present() {
        let args = vec![OsString::from("twerk"), OsString::from("--version")];

        match parse_cli_args(&args) {
            Ok(_) => unreachable!("expected version flag to short-circuit clap parsing"),
            Err(error) => assert_eq!(error.kind(), ErrorKind::DisplayVersion),
        }
    }

    #[test]
    fn parse_cli_args_returns_run_command_for_coordinator_mode() {
        let args = vec![
            OsString::from("twerk"),
            OsString::from("run"),
            OsString::from("coordinator"),
        ];

        assert!(matches!(
            parse_cli_args(&args),
            Ok(CliAction::Execute(
                Some(Commands::Run {
                    mode: crate::commands::RunMode::Coordinator
                }),
                false
            ))
        ));
    }

    #[test]
    fn parse_cli_args_enables_json_mode_for_health_command() {
        let args = vec![
            OsString::from("twerk"),
            OsString::from("--json"),
            OsString::from("health"),
            OsString::from("--endpoint"),
            OsString::from("http://localhost:8080"),
        ];

        match parse_cli_args(&args) {
            Ok(CliAction::Execute(Some(Commands::Health { endpoint }), true)) => {
                assert_eq!(endpoint, Some("http://localhost:8080".to_string()));
            }
            other => unreachable!("expected json mode health command, got {:?}", other),
        }
    }

    #[test]
    fn get_banner_mode_prefers_environment_override_when_set() {
        std::env::set_var("TWERK_CLI_BANNER_MODE", "off");

        let result = get_banner_mode();

        std::env::remove_var("TWERK_CLI_BANNER_MODE");
        assert_eq!(result, BannerMode::Off);
    }

    #[test]
    fn get_endpoint_reads_client_endpoint_from_environment_override() {
        std::env::set_var("TWERK_CLIENT_ENDPOINT", "http://127.0.0.1:9999");

        let endpoint = get_endpoint();

        std::env::remove_var("TWERK_CLIENT_ENDPOINT");
        assert_eq!(endpoint, "http://127.0.0.1:9999");
    }
}
