//! Main CLI entry point
//!
//! Orchestrates the CLI: parses arguments, displays banner, and dispatches commands.

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use serde_json::{json, Value};
use std::ffi::OsString;
use tracing::Level;
use tracing_subscriber::{fmt, fmt::format::FmtSpan, prelude::*, EnvFilter};
use twerk_common::load_config;
use twerk_core::domain::{Dsn, Endpoint};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitStatus {
    Success = 0,
    Failure = 1,
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
fn get_endpoint() -> Result<Endpoint, twerk_core::domain::EndpointError> {
    get_config_string("client.endpoint")
        .or_else(|| get_config_string("endpoint"))
        .map(Endpoint::new)
        .unwrap_or_else(|| Endpoint::new(DEFAULT_ENDPOINT))
}

/// Get datastore type from configuration or default
fn get_datastore_type() -> String {
    get_config_string("datastore.type").unwrap_or_else(|| DEFAULT_DATASTORE_TYPE.to_string())
}

/// Get `PostgreSQL` DSN from configuration or default
fn get_postgres_dsn() -> Result<Dsn, twerk_core::domain::DsnError> {
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

fn json_requested(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "--json")
}

fn os_string_eq(value: &OsString, expected: &str) -> bool {
    value == expected
}

fn parse_cli_args(args: &[OsString]) -> Result<CliAction, clap::Error> {
    Cli::try_parse_from(args.iter().cloned()).map(|cli| CliAction::Execute(cli.command, cli.json))
}

fn render_help_for_path(path: &[String]) -> Result<String, CliError> {
    let mut command = Cli::command();
    let mut buffer = Vec::new();
    let target = path.iter().try_fold(&mut command, |current, segment| {
        current
            .find_subcommand_mut(segment)
            .ok_or_else(|| CliError::MissingArgument(format!("unknown help target: {segment}")))
    })?;

    target.write_long_help(&mut buffer).map_err(CliError::Io)?;
    String::from_utf8(buffer).map_err(|error| CliError::Config(error.to_string()))
}

fn render_top_level_help() -> Result<String, CliError> {
    render_help_for_path(&[])
}

fn print_json(value: &Value) {
    println!("{value}");
}

fn json_help_payload(content: String) -> Value {
    json!({
        "type": "help",
        "version": VERSION,
        "commit": GIT_COMMIT,
        "content": content,
    })
}

fn json_version_payload(content: String) -> Value {
    json!({
        "type": "version",
        "version": VERSION,
        "commit": GIT_COMMIT,
        "content": content,
    })
}

fn json_error_payload(kind: &str, message: String, content: Option<String>) -> Value {
    json!({
        "type": "error",
        "kind": kind,
        "version": VERSION,
        "commit": GIT_COMMIT,
        "message": message,
        "content": content,
    })
}

const fn clap_error_kind_name(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::DisplayHelp => "help",
        ErrorKind::DisplayVersion => "version",
        ErrorKind::InvalidValue => "invalid_value",
        ErrorKind::InvalidSubcommand => "invalid_subcommand",
        ErrorKind::MissingRequiredArgument => "missing_required_argument",
        ErrorKind::UnknownArgument => "unknown_argument",
        _ => "parse_error",
    }
}

fn handle_parse_error(error: clap::Error, emit_json: bool) -> i32 {
    let exit_code = error.exit_code();
    if emit_json {
        let content = error.to_string();
        let payload = match error.kind() {
            ErrorKind::DisplayHelp => json_help_payload(content),
            ErrorKind::DisplayVersion => json_version_payload(content),
            kind => json_error_payload(clap_error_kind_name(kind), content.clone(), Some(content)),
        };
        print_json(&payload);
        exit_code
    } else {
        error.exit()
    }
}

fn handle_runtime_error(error: CliError, emit_json: bool) -> i32 {
    if emit_json {
        let kind = match error {
            CliError::InvalidEndpoint(_)
            | CliError::MissingArgument(_)
            | CliError::InvalidHostname(_) => "validation",
            _ => "runtime",
        };
        print_json(&json_error_payload(kind, error.to_string(), None));
    } else {
        eprintln!("Error: {error}");
    }
    ExitStatus::Failure as i32
}

fn handle_json_help_subcommand(args: &[OsString]) -> Option<i32> {
    let is_help_subcommand = args.get(1).is_some_and(|arg| os_string_eq(arg, "help"));
    if !json_requested(args) || !is_help_subcommand {
        return None;
    }

    let help_path = args
        .iter()
        .skip(2)
        .filter(|arg| !os_string_eq(arg, "--json"))
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    match render_help_for_path(&help_path) {
        Ok(content) => {
            print_json(&json_help_payload(content));
            Some(ExitStatus::Success as i32)
        }
        Err(error) => Some(handle_runtime_error(error, true)),
    }
}

const fn should_emit_startup_ui(command: &Commands, json_mode: bool) -> bool {
    !json_mode && !matches!(command, Commands::Version)
}

async fn execute_command(command: Commands, json_mode: bool) -> Result<(), CliError> {
    match command {
        Commands::Run { mode, hostname } => run_engine(mode, hostname).await,
        Commands::Migration { yes: _ } => {
            let dstype = get_datastore_type();
            let dsn = get_postgres_dsn()?;
            run_migration(&dstype, dsn.as_str()).await
        }
        Commands::Health { endpoint } => {
            let ep = if let Some(ep_str) = endpoint {
                Endpoint::new(ep_str)
                    .map_err(|error| CliError::InvalidEndpoint(error.to_string()))?
            } else {
                get_endpoint()?
            };
            health_check(ep.as_str(), json_mode).await.map(|_| ())
        }
        Commands::Version => {
            let content = format!("twerk {VERSION}\n");
            if json_mode {
                print_json(&json_version_payload(content));
            } else {
                println!("twerk {VERSION}");
            }
            Ok(())
        }
    }
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

    let (cmd, json_mode) = match action {
        CliAction::Execute(cmd, json) => (cmd, json),
    };

    // If no subcommand was provided, display help and exit 0
    let cmd = match cmd {
        Some(cmd) => cmd,
        None => match render_top_level_help() {
            Ok(content) => {
                if json_mode {
                    print_json(&json_help_payload(content));
                } else {
                    print!("{content}");
                }
                return ExitStatus::Success as i32;
            }
            Err(error) => {
                return handle_runtime_error(error, json_mode);
            }
        },
    };

    // Setup logging and banner for interactive commands only.
    if should_emit_startup_ui(&cmd, json_mode) {
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
    fn parse_cli_args_returns_version_subcommand() {
        let args = vec![OsString::from("twerk"), OsString::from("version")];

        assert!(matches!(
            parse_cli_args(&args),
            Ok(CliAction::Execute(Some(Commands::Version), false))
        ));
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
                    mode: crate::commands::RunMode::Coordinator,
                    hostname: None
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

    #[allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn get_endpoint_reads_client_endpoint_from_environment_override() {
        std::env::set_var("TWERK_CLIENT_ENDPOINT", "http://127.0.0.1:9999");

        let endpoint = get_endpoint().unwrap();

        std::env::remove_var("TWERK_CLIENT_ENDPOINT");
        assert_eq!(endpoint.as_str(), "http://127.0.0.1:9999");
    }

    #[allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn render_top_level_help_contains_usage() {
        let help = render_top_level_help().expect("help should render");

        assert!(help.contains("Usage:"));
    }
}
