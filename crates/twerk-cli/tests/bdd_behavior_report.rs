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
//! | 14 | migrate | `DEFAULT_POSTGRES_DSN` uses placeholders, not real credentials | migrate.rs |
//!
//! ## Execution Evidence

use clap::Parser;
use std::ffi::OsString;
use twerk_cli::cli::{DEFAULT_DATASTORE_TYPE, DEFAULT_ENDPOINT, VERSION};
use twerk_cli::commands::{Cli, Commands, RunMode};
use twerk_cli::error::CliError;
use twerk_cli::health::{health_check, HealthResponse};
use twerk_cli::migrate::{run_migration, DEFAULT_POSTGRES_DSN};

// B2: LOGGING_ENV_LOCK removed — env-var tests use serial_test::serial instead.
// Holzmann Rule 7: no shared mutable state (LazyLock<Mutex>) in tests.

struct LoggingEnvGuard {
    previous: Option<OsString>,
}

impl LoggingEnvGuard {
    fn set(value: &str) -> Self {
        let previous = std::env::var_os("TWERK_LOGGING_LEVEL");
        std::env::set_var("TWERK_LOGGING_LEVEL", value);
        Self { previous }
    }
}

impl Drop for LoggingEnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var("TWERK_LOGGING_LEVEL", previous);
        } else {
            std::env::remove_var("TWERK_LOGGING_LEVEL");
        }
    }
}

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

// B2: Uses #[serial_test::serial] instead of LOGGING_ENV_LOCK for env-var isolation
#[test]
#[serial_test::serial]
fn claim_3_setup_logging_accepts_valid_level() {
    let _guard = LoggingEnvGuard::set("debug");
    let result = twerk_cli::cli::setup_logging();
    match result {
        Ok(()) => {} // expected
        Err(e) => panic!("setup_logging should accept 'debug', got error: {e}"),
    }
}

#[test]
#[serial_test::serial]
fn claim_4_setup_logging_rejects_invalid_level() {
    let _guard = LoggingEnvGuard::set("invalid_level_xyz");
    let result = twerk_cli::cli::setup_logging();
    assert!(matches!(result, Err(CliError::Logging(_))));
}

#[test]
fn claim_5_cli_struct_can_be_parsed_with_no_subcommand() {
    let args = ["twerk"];
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
    let cli = result.expect("standalone mode should be accepted");
    assert!(
        matches!(cli.command, Some(Commands::Run { .. })),
        "expected Run command, got {:?}",
        cli.command
    );
}

#[test]
fn claim_9_run_command_accepts_coordinator_mode() {
    let args = vec!["twerk", "run", "coordinator"];
    let cli = Cli::try_parse_from(args).expect("coordinator mode should be accepted");
    match cli.command {
        Some(Commands::Run { mode, .. }) => {
            assert_eq!(mode, RunMode::Coordinator, "expected Coordinator mode");
        }
        other => panic!("expected Run command, got {:?}", other),
    }
}

#[test]
fn claim_10_run_command_accepts_worker_mode() {
    let args = vec!["twerk", "run", "worker"];
    let cli = Cli::try_parse_from(args).expect("worker mode should be accepted");
    match cli.command {
        Some(Commands::Run { mode, .. }) => {
            assert_eq!(mode, RunMode::Worker, "expected Worker mode");
        }
        other => panic!("expected Run command, got {:?}", other),
    }
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

// B1: claim_14 loop expanded into 8 individual test functions.
// Holzmann Rule 2: no loops in test bodies.

#[test]
fn claim_14_config_error_display_contains_expected_substring() {
    let err = CliError::Config("test".into());
    let msg = err.to_string();
    assert!(
        msg.contains("configuration error"),
        "error '{msg}' should contain 'configuration error'"
    );
}

#[test]
fn claim_14_health_failed_display_contains_expected_substring() {
    let err = CliError::HealthFailed { status: 500 };
    let msg = err.to_string();
    assert!(
        msg.contains("health check failed"),
        "error '{msg}' should contain 'health check failed'"
    );
}

#[test]
fn claim_14_invalid_body_display_contains_expected_substring() {
    let err = CliError::InvalidBody("bad".into());
    let msg = err.to_string();
    assert!(
        msg.contains("invalid response body"),
        "error '{msg}' should contain 'invalid response body'"
    );
}

#[test]
fn claim_14_missing_argument_display_contains_expected_substring() {
    let err = CliError::MissingArgument("arg".into());
    let msg = err.to_string();
    assert!(
        msg.contains("missing required argument"),
        "error '{msg}' should contain 'missing required argument'"
    );
}

#[test]
fn claim_14_migration_error_display_contains_expected_substring() {
    let err = CliError::Migration("fail".into());
    let msg = err.to_string();
    assert!(
        msg.contains("migration error"),
        "error '{msg}' should contain 'migration error'"
    );
}

#[test]
fn claim_14_unknown_datastore_display_contains_expected_substring() {
    let err = CliError::UnknownDatastore("x".into());
    let msg = err.to_string();
    assert!(
        msg.contains("unsupported datastore type"),
        "error '{msg}' should contain 'unsupported datastore type'"
    );
}

#[test]
fn claim_14_logging_error_display_contains_expected_substring() {
    let err = CliError::Logging("bad".into());
    let msg = err.to_string();
    assert!(
        msg.contains("logging setup error"),
        "error '{msg}' should contain 'logging setup error'"
    );
}

#[test]
fn claim_14_engine_error_display_contains_expected_substring() {
    let err = CliError::Engine("fail".into());
    let msg = err.to_string();
    assert!(
        msg.contains("engine error"),
        "error '{msg}' should contain 'engine error'"
    );
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

// B3: explicit variant match — no bare assert!(result.is_err())
#[test]
fn claim_17_health_check_returns_http_error_on_connection_failure() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(health_check("http://localhost:99999", false));
    match result {
        Err(CliError::Http(_)) => {}
        Err(other) => panic!("expected Http error, got {:?}", other),
        Ok(val) => panic!("expected error, got Ok({val})"),
    }
}

// B4: split into two tests with explicit variant match — no bare is_err()
#[test]
fn claim_18_health_check_trailing_slash_returns_http_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(health_check("http://localhost:99999/", false));
    match result {
        Err(CliError::Http(_)) => {}
        Err(other) => panic!("expected Http error, got {:?}", other),
        Ok(val) => panic!("expected error, got Ok({val})"),
    }
}

#[test]
fn claim_18_health_check_bare_endpoint_returns_http_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(health_check("http://localhost:99999", false));
    match result {
        Err(CliError::Http(_)) => {}
        Err(other) => panic!("expected Http error, got {:?}", other),
        Ok(val) => panic!("expected error, got Ok({val})"),
    }
}

// B5: explicit variant match — no bare is_err()
#[test]
fn claim_19_migration_returns_unknown_datastore_error_for_mysql() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(run_migration("mysql", "dsn"));
    match result {
        Err(CliError::UnknownDatastore(msg)) => {
            assert!(msg.contains("mysql"));
        }
        Err(other) => panic!("expected UnknownDatastore, got {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn claim_20_default_postgres_dsn_format() {
    let dsn = DEFAULT_POSTGRES_DSN;
    assert!(dsn.contains("host=localhost"));
    assert!(dsn.contains("dbname=twerk"));
    assert!(dsn.contains("port=5432"));
    assert!(
        dsn.contains("PLACEHOLDER_MUST_OVERRIDE"),
        "DSN must use placeholder credentials, not real values"
    );
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
    fn breakage_check_health_with_whitespace_returns_http_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(health_check("  http://localhost:99999  ", false));
        match result {
            Err(CliError::Http(_)) => {}
            Err(other) => panic!("expected Http error, got {:?}", other),
            Ok(val) => panic!("expected error, got Ok({val})"),
        }
    }

    #[test]
    fn breakage_check_empty_json_body_parsing_returns_error() {
        let empty_json = r#""#;
        let result: Result<HealthResponse, _> = serde_json::from_str(empty_json);
        let err = result.unwrap_err();
        assert!(
            !err.to_string().is_empty(),
            "serde_json error should have a non-empty message"
        );
    }

    // B6: 15 individual completeness_check tests — one per CliError variant.
    // No let _ = suppression — every constructed variant is asserted upon.
    // Holzmann Rule: every constructed value must be asserted upon.

    #[test]
    fn completeness_check_config_variant_is_constructible_and_displayable() {
        let variant = CliError::Config("test".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_http_variant_is_constructible_and_displayable() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let reqwest_err = rt
            .block_on(async { reqwest::get("http://unreachable-host.invalid/").await })
            .unwrap_err();
        let variant = CliError::from(reqwest_err);
        let debug_str = format!("{variant:?}");
        assert!(!debug_str.is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_http_status_variant_is_constructible_and_displayable() {
        let variant = CliError::HttpStatus {
            status: 500,
            reason: "Internal Server Error".into(),
        };
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_health_failed_variant_is_constructible_and_displayable() {
        let variant = CliError::HealthFailed { status: 503 };
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_invalid_body_variant_is_constructible_and_displayable() {
        let variant = CliError::InvalidBody("bad payload".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_missing_argument_variant_is_constructible_and_displayable() {
        let variant = CliError::MissingArgument("arg".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_migration_variant_is_constructible_and_displayable() {
        let variant = CliError::Migration("fail".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_unknown_datastore_variant_is_constructible_and_displayable() {
        let variant = CliError::UnknownDatastore("redis".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_logging_variant_is_constructible_and_displayable() {
        let variant = CliError::Logging("bad level".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_engine_variant_is_constructible_and_displayable() {
        let variant = CliError::Engine("start failed".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_invalid_hostname_variant_is_constructible_and_displayable() {
        let variant = CliError::InvalidHostname("!!!bad".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_invalid_endpoint_variant_is_constructible_and_displayable() {
        let variant = CliError::InvalidEndpoint("not a url".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_not_found_variant_is_constructible_and_displayable() {
        let variant = CliError::NotFound("resource xyz".into());
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_api_error_variant_is_constructible_and_displayable() {
        let variant = CliError::ApiError {
            code: 400,
            message: "bad input".into(),
        };
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    #[test]
    fn completeness_check_io_variant_is_constructible_and_displayable() {
        use std::io;
        let variant = CliError::Io(io::Error::new(io::ErrorKind::NotFound, "file missing"));
        assert!(!format!("{variant:?}").is_empty());
        assert!(!variant.to_string().is_empty());
    }

    // B7: boundary_check with explicit error assertion — no let _ = suppression
    #[test]
    fn boundary_check_very_long_endpoint_string_returns_http_error() {
        let long_endpoint = format!("http://localhost:{}", "9".repeat(1000));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(health_check(&long_endpoint, false));
        match result {
            Err(CliError::Http(_)) => {}
            Err(other) => panic!("expected Http error, got {:?}", other),
            Ok(val) => panic!("expected error, got Ok({val})"),
        }
    }
}
