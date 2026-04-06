//! Main CLI entry point
//!
//! Orchestrates the CLI: parses arguments, displays banner, and dispatches commands.

use clap::Parser;
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

/// Git commit hash (placeholder - would be set by build script in production)
pub const GIT_COMMIT: &str = "unknown";

/// Parsed top-level CLI action.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CliAction {
    Execute(Commands),
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
    Cli::try_parse_from(args.iter().cloned()).map(|cli| CliAction::Execute(cli.command))
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

    let CliAction::Execute(cmd) = action;

    // Parse command line arguments before any output side effects.

    // Setup logging
    setup_logging()?;

    // Display banner
    let banner_mode = get_banner_mode();
    display_banner(banner_mode, VERSION, GIT_COMMIT);

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
            health_check(&ep).await?;
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
    fn test_default_endpoint() {
        assert_eq!(DEFAULT_ENDPOINT, "http://localhost:8000");
        assert!(DEFAULT_ENDPOINT.starts_with("http://"));
        assert!(DEFAULT_ENDPOINT.contains("localhost"));
    }

    #[test]
    fn test_default_datastore_type() {
        assert_eq!(DEFAULT_DATASTORE_TYPE, "postgres");
    }

    #[test]
    fn test_default_postgres_dsn() {
        assert!(DEFAULT_POSTGRES_DSN.contains("localhost"));
    }

    #[test]
    fn test_version_not_empty() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_git_commit_not_empty() {
        let commit = GIT_COMMIT;
        assert!(!commit.is_empty());
        assert_eq!(commit, "unknown"); // Default placeholder
    }

    #[test]
    fn test_get_git_commit_returns_string() {
        let commit = get_git_commit();
        assert!(!commit.is_empty());
    }

    #[test]
    fn test_constants_are_static() {
        // Verify constants are accessible without mutation
        let _ep = DEFAULT_ENDPOINT;
        let _dst = DEFAULT_DATASTORE_TYPE;
        let _pg_dsn = DEFAULT_POSTGRES_DSN;
        let _ver = VERSION;
        let _git = GIT_COMMIT;
    }

    #[test]
    fn test_parse_cli_args_without_subcommand_shows_help() {
        let args = vec![OsString::from("twerk")];

        match parse_cli_args(&args) {
            Ok(_) => {
                unreachable!("expected clap to short-circuit with error when missing subcommand")
            }
            Err(error) => assert_eq!(
                error.kind(),
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ),
        }
    }

    #[test]
    fn test_parse_cli_args_for_version_short_circuits() {
        let args = vec![OsString::from("twerk"), OsString::from("--version")];

        match parse_cli_args(&args) {
            Ok(_) => unreachable!("expected version flag to short-circuit clap parsing"),
            Err(error) => assert_eq!(error.kind(), ErrorKind::DisplayVersion),
        }
    }

    #[test]
    fn test_parse_cli_args_for_run_command_executes() {
        let args = vec![
            OsString::from("twerk"),
            OsString::from("run"),
            OsString::from("coordinator"),
        ];

        assert!(matches!(
            parse_cli_args(&args),
            Ok(CliAction::Execute(Commands::Run {
                mode: crate::commands::RunMode::Coordinator
            }))
        ));
    }

    #[test]
    fn test_get_banner_mode_prefers_environment_override() {
        std::env::set_var("TWERK_CLI_BANNER_MODE", "off");

        let result = get_banner_mode();

        std::env::remove_var("TWERK_CLI_BANNER_MODE");
        assert_eq!(result, BannerMode::Off);
    }

    #[test]
    fn test_get_endpoint_reads_client_endpoint_env_override() {
        std::env::set_var("TWERK_CLIENT_ENDPOINT", "http://127.0.0.1:9999");

        let endpoint = get_endpoint();

        std::env::remove_var("TWERK_CLIENT_ENDPOINT");
        assert_eq!(endpoint, "http://127.0.0.1:9999");
    }
}
