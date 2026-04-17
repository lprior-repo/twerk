//! BDD Behavior Report for twerk-cli
//!
//! ## Claim Sheet
//!
//! | # | Module | Claim | Source |
//! |---|--------|-------|--------|
//! | 1 | cli | `DEFAULT_ENDPOINT = "http://localhost:8000"` | cli.rs:21 |
//! | 2 | cli | `DEFAULT_DATASTORE_TYPE = "postgres"` | cli.rs:24 |
//! | 3 | cli | `setup_logging()` parses valid log levels without error | cli.rs:53 |
//! | 4 | cli | `setup_logging()` returns `CliError::Logging` for invalid level | cli.rs:58 |
//! | 5 | commands | `Commands::Run { mode }` accepts standalone/coordinator/worker | commands.rs:27-35 |
//! | 6 | commands | `Commands::Migration { yes: bool }` skips confirmation | commands.rs:47-50 |
//! | 7 | commands | `Commands::Health { endpoint }` accepts optional endpoint | commands.rs:53-57 |
//! | 8 | commands | `Cli --json` global flag enables JSON output mode | commands.rs:17-19 |
//! | 9 | commands | `--version` short-circuits with DisplayVersion error | commands.rs |
//! | 10 | error | `CliError` variants format with expected messages | error.rs |
//! | 11 | health | `HealthResponse` deserializes from JSON | health.rs:10-14 |
//! | 12 | migrate | `run_migration("postgres", dsn)` executes schema | migrate.rs:35-61 |
//! | 13 | migrate | `run_migration("mysql", dsn)` returns `CliError::UnknownDatastore` | migrate.rs:59 |
//! | 14 | migrate | `DEFAULT_POSTGRES_DSN` contains required connection fields | migrate.rs |
//!
//! ## Execution Evidence

use clap::Parser;
use twerk_cli::cli::{DEFAULT_DATASTORE_TYPE, DEFAULT_ENDPOINT, VERSION};
use twerk_cli::commands::{Cli, Commands};
use twerk_cli::error::CliError;
use twerk_cli::health::{health_check, HealthResponse};
use twerk_cli::migrate::{run_migration, DEFAULT_POSTGRES_DSN};

#[test]
fn claim_1_default_endpoint_constant() {
    assert_eq!(DEFAULT_ENDPOINT, "http://localhost:8000");
    assert!(DEFAULT_ENDPOINT.starts_with("http://"));
    assert!(DEFAULT_ENDPOINT.contains("localhost"));
}

#[test]
fn claim_2_default_datastore_type() {
    assert_eq!(DEFAULT_DATASTORE_TYPE, "postgres");
}

#[test]
fn claim_3_setup_logging_accepts_valid_level() {
    std::env::set_var("TWERK_LOGGING_LEVEL", "debug");
    let result = twerk_cli::cli::setup_logging();
    std::env::remove_var("TWERK_LOGGING_LEVEL");
    assert!(result.is_ok(), "setup_logging should accept 'debug'");
}

#[test]
fn claim_4_setup_logging_rejects_invalid_level() {
    std::env::set_var("TWERK_LOGGING_LEVEL", "invalid_level_xyz");
    let result = twerk_cli::cli::setup_logging();
    std::env::remove_var("TWERK_LOGGING_LEVEL");
    assert!(matches!(result, Err(CliError::Logging(_))));
}

#[test]
fn claim_5_cli_struct_can_be_parsed_with_no_subcommand() {
    let args = vec!["twerk"];
    let cli = Cli::parse_from(args.iter());
    assert!(cli.command.is_none());
}

#[test]
fn claim_6_help_flag_shows_help() {
    use clap::error::ErrorKind;
    let args = vec!["twerk", "--help"];
    match Cli::try_parse_from(args) {
        Ok(_) => panic!("--help should short-circuit"),
        Err(error) => assert_eq!(error.kind(), ErrorKind::DisplayHelp),
    }
}

#[test]
fn claim_7_version_flag_returns_display_version_error() {
    use clap::error::ErrorKind;
    let args = vec!["twerk", "--version"];
    match Cli::try_parse_from(args) {
        Ok(_) => panic!("expected version flag to short-circuit clap parsing"),
        Err(error) => assert_eq!(error.kind(), ErrorKind::DisplayVersion),
    }
}

#[test]
fn claim_8_run_command_accepts_standalone_mode() {
    let args = vec!["twerk", "run", "standalone"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "standalone mode should be accepted");
    if let Ok(cli) = result {
        assert!(matches!(cli.command, Some(Commands::Run { .. })));
    }
}

#[test]
fn claim_9_run_command_accepts_coordinator_mode() {
    let args = vec!["twerk", "run", "coordinator"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "coordinator mode should be accepted");
}

#[test]
fn claim_10_run_command_accepts_worker_mode() {
    let args = vec!["twerk", "run", "worker"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_ok(), "worker mode should be accepted");
}

#[test]
fn claim_11_migration_command_accepts_yes_flag() {
    let args = vec!["twerk", "migration", "--yes"];
    let cli = Cli::try_parse_from(args).unwrap();
    match cli.command {
        Some(Commands::Migration { yes }) => assert!(yes, "migration --yes should set yes=true"),
        other => panic!("expected Migration, got {:?}", other),
    }
}

#[test]
fn claim_12_health_command_accepts_endpoint_option() {
    let args = vec!["twerk", "health", "--endpoint", "http://localhost:9000"];
    let cli = Cli::try_parse_from(args).unwrap();
    match cli.command {
        Some(Commands::Health { endpoint }) => {
            assert_eq!(endpoint, Some("http://localhost:9000".to_string()));
        }
        other => panic!("expected Health with endpoint, got {:?}", other),
    }
}

#[test]
fn claim_13_json_global_flag() {
    let args = vec!["twerk", "--json", "health"];
    let cli = Cli::try_parse_from(args).unwrap();
    assert!(cli.json, "expected json flag to be set");
}

#[test]
fn claim_14_cli_error_display_messages() {
    let errors: Vec<(CliError, &str)> = vec![
        (CliError::Config("test".into()), "configuration error"),
        (
            CliError::HealthFailed { status: 500 },
            "health check failed",
        ),
        (CliError::InvalidBody("bad".into()), "invalid response body"),
        (
            CliError::MissingArgument("arg".into()),
            "missing required argument",
        ),
        (CliError::Migration("fail".into()), "migration error"),
        (
            CliError::UnknownDatastore("x".into()),
            "unsupported datastore type",
        ),
        (CliError::Logging("bad".into()), "logging setup error"),
        (CliError::Engine("fail".into()), "engine error"),
    ];

    for (err, expected_substring) in errors {
        let msg = err.to_string();
        assert!(
            msg.contains(expected_substring),
            "error '{}' should contain '{}'",
            msg,
            expected_substring
        );
    }
}

#[test]
fn claim_15_health_response_deserialize() {
    let json = r#"{"status": "ok"}"#;
    let response: HealthResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.status, "ok");
}

#[test]
fn claim_16_health_response_deserialize_with_extra_fields() {
    let json = r#"{"status": "ok", "extra": "ignored"}"#;
    let response: HealthResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.status, "ok");
}

#[test]
fn claim_17_health_check_error_on_connection_failure() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(health_check("http://localhost:99999", false));
    assert!(result.is_err());
    match result.unwrap_err() {
        CliError::Http(_) => {}
        other => panic!("expected Http error, got {:?}", other),
    }
}

#[test]
fn claim_18_health_check_endpoint_with_trailing_slash() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result1 = rt.block_on(health_check("http://localhost:99999/", false));
    let result2 = rt.block_on(health_check("http://localhost:99999", false));
    assert!(result1.is_err());
    assert!(result2.is_err());
}

#[test]
fn claim_19_migration_rejects_unknown_datastore() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(run_migration("mysql", "dsn"));
    assert!(result.is_err());
    match result.unwrap_err() {
        CliError::UnknownDatastore(msg) => {
            assert!(msg.contains("mysql"));
        }
        other => panic!("expected UnknownDatastore, got {:?}", other),
    }
}

#[test]
fn claim_20_default_postgres_dsn_format() {
    let dsn = DEFAULT_POSTGRES_DSN;
    assert!(dsn.contains("host=localhost"));
    assert!(dsn.contains("user=twerk"));
    assert!(dsn.contains("dbname=twerk"));
    assert!(dsn.contains("port=5432"));
}

// =============================================================================
// ADVERSARIAL TESTS - Liar Check, Breakage Check, Completeness Check
// =============================================================================

mod adversarial {
    use super::*;

    #[test]
    fn liar_check_version_not_empty() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn breakage_check_health_with_whitespace() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(health_check("  http://localhost:99999  ", false));
        assert!(result.is_err());
    }

    #[test]
    fn breakage_check_empty_json_body_parsing() {
        let empty_json = r#""#;
        let result: Result<HealthResponse, _> = serde_json::from_str(empty_json);
        assert!(result.is_err());
    }

    #[test]
    fn completeness_check_all_error_variants_in_public_api() {
        let _ = CliError::Config("x".into());
        let _ = CliError::HealthFailed { status: 500 };
        let _ = CliError::InvalidBody("x".into());
        let _ = CliError::MissingArgument("x".into());
        let _ = CliError::Migration("x".into());
        let _ = CliError::UnknownDatastore("x".into());
        let _ = CliError::Logging("x".into());
        let _ = CliError::Engine("x".into());
    }

    #[test]
    fn boundary_check_very_long_endpoint_string() {
        let long_endpoint = format!("http://localhost:{}", "9".repeat(1000));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(health_check(&long_endpoint, false));
    }
}
