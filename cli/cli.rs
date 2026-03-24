//! Main CLI entry point
//!
//! Orchestrates the CLI: parses arguments, displays banner, and dispatches commands.

use std::env;

use clap::Parser;
use config::{Config, Environment};
use tracing::Level;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use super::banner::{display_banner, BannerMode};
use super::commands::Commands;
use super::error::CliError;
use super::health::health_check;
use super::migrate::{run_migration, DEFAULT_POSTGRES_DSN};
use super::run::run_engine;

/// Default endpoint for health checks
pub const DEFAULT_ENDPOINT: &str = "http://localhost:8000";

/// Default datastore type
pub const DEFAULT_DATASTORE_TYPE: &str = "postgres";

/// Tork version string
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash (placeholder - would be set by build script in production)
pub const GIT_COMMIT: &str = "unknown";

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
    let log_level_str = get_config_string("logging.level").unwrap_or_else(|| "info".to_string());

    let level: Level = log_level_str
        .parse()
        .map_err(|_| CliError::Logging(format!("invalid log level: {log_level_str}")))?;

    let filter = EnvFilter::try_from_default_env()
        .map_err(|e| CliError::Logging(e.to_string()))?
        .add_directive(level.into());

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    Ok(())
}

/// Get banner mode from configuration
fn get_banner_mode() -> BannerMode {
    get_config_string("cli.banner.mode")
        .map(|s| BannerMode::from_str(&s))
        .unwrap_or_default()
}

/// Get endpoint from configuration or default
fn get_endpoint() -> String {
    get_config_string("cli.endpoint").unwrap_or_else(|| DEFAULT_ENDPOINT.to_string())
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
/// Environment variables are prefixed with `TORK_` and use double underscore for nesting.
/// e.g., `TORK_DATABASE__POSTGRES__DSN` for `datastore.postgres.dsn`.
fn get_config_string(key: &str) -> Option<String> {
    let config = Config::builder()
        .add_source(config::File::with_name("config"))
        .add_source(config::File::with_name("config.local"))
        .add_source(
            Environment::with_prefix("TORK")
                .separator("__")
                .try_parsing(true),
        )
        .build()
        .ok()?;

    config.get_string(key).ok()
}

/// Execute the CLI with the given command
///
/// # Errors
///
/// Returns an error if command execution fails.
pub async fn run() -> Result<(), CliError> {
    // Display banner
    let banner_mode = get_banner_mode();
    display_banner(banner_mode, VERSION, GIT_COMMIT);

    // Setup logging
    setup_logging()?;

    // Parse command line arguments
    let cmd = Commands::parse();

    match cmd {
        Commands::Run { mode } => {
            let mode = mode.unwrap_or_default();
            run_engine(&mode).await?;
        }
        Commands::Migration { .. } => {
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
}
