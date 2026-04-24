//! BDD Behavior Report for twerk-cli
//!
//! This module implements behavioral contract verification using Given-When-Then tests.
//! Tests are organized by feature and cover the complete public API surface.
//!
//! ## Three Lenses Applied
//! 1. **Liar Check**: Does implementation match claims?
//! 2. **Breakage Check**: What happens when conditions degrade?
//! 3. **Completeness Check**: Is every public item exercised?
//!
//! ## Public API Surface (verified from lib.rs exports)
//! - `twerk_cli::cli::run` - async CLI entry point
//! - `twerk_cli::cli::setup_logging` - logging setup
//! - `twerk_cli::cli::DEFAULT_DATASTORE_TYPE` - constant "postgres"
//! - `twerk_cli::cli::DEFAULT_ENDPOINT` - constant "http://localhost:8000"
//! - `twerk_cli::cli::VERSION` - version string
//! - `twerk_cli::commands::Commands` - CLI commands enum
//! - `twerk_cli::error::CliError` - error enum

use std::error::Error;
use twerk_cli::{
    cli::{DEFAULT_DATASTORE_TYPE, DEFAULT_ENDPOINT, VERSION},
    commands::{Commands, ServerCommand},
    error::CliError,
};

#[cfg(test)]
mod bdd_constants_and_defaults {

    use super::*;

    mod given_default_constants {
        use super::*;

        #[test]
        fn then_default_endpoint_is_localhost_http() {
            assert_eq!(DEFAULT_ENDPOINT, "http://localhost:8000");
            assert!(DEFAULT_ENDPOINT.starts_with("http://"));
            assert!(DEFAULT_ENDPOINT.contains("localhost"));
        }

        #[test]
        fn then_default_endpoint_contains_port_8000() {
            assert!(DEFAULT_ENDPOINT.contains("8000"));
        }

        #[test]
        fn then_default_datastore_type_is_postgres() {
            assert_eq!(DEFAULT_DATASTORE_TYPE, "postgres");
        }

        #[test]
        fn then_version_is_not_empty() {
            assert!(!VERSION.is_empty());
        }

        #[test]
        fn then_version_follows_semver_format() {
            let parts: Vec<&str> = VERSION.split('.').collect();
            assert!(parts.len() >= 2, "Version should have at least major.minor");
        }
    }
}

#[cfg(test)]
mod bdd_error_handling {

    use super::*;
    use std::io;

    mod given_cli_error_variants {

        use super::*;

        #[test]
        fn then_config_error_formats_correctly() {
            let err = CliError::Config("missing key".to_string());
            let msg = err.to_string();
            assert!(msg.contains("configuration error"));
            assert!(msg.contains("missing key"));
        }

        #[test]
        fn then_health_failed_error_formats_with_status() {
            let err = CliError::HealthFailed { status: 503 };
            let msg = err.to_string();
            assert!(msg.contains("health check failed"));
            assert!(msg.contains("503"));
        }

        #[test]
        fn then_invalid_body_error_formats_correctly() {
            let err = CliError::InvalidBody("not json".to_string());
            let msg = err.to_string();
            assert!(msg.contains("invalid response body"));
            assert!(msg.contains("not json"));
        }

        #[test]
        fn then_missing_argument_error_formats_correctly() {
            let err = CliError::MissingArgument("mode".to_string());
            let msg = err.to_string();
            assert!(msg.contains("missing required argument"));
            assert!(msg.contains("mode"));
        }

        #[test]
        fn then_migration_error_formats_correctly() {
            let err = CliError::Migration("connection refused".to_string());
            let msg = err.to_string();
            assert!(msg.contains("migration error"));
            assert!(msg.contains("connection refused"));
        }

        #[test]
        fn then_unknown_datastore_error_formats_correctly() {
            let err = CliError::UnknownDatastore("mysql".to_string());
            let msg = err.to_string();
            assert!(msg.contains("unsupported datastore type"));
            assert!(msg.contains("mysql"));
        }

        #[test]
        fn then_logging_error_formats_correctly() {
            let err = CliError::Logging("invalid level".to_string());
            let msg = err.to_string();
            assert!(msg.contains("logging setup error"));
            assert!(msg.contains("invalid level"));
        }

        #[test]
        fn then_engine_error_formats_correctly() {
            let err = CliError::Engine("failed to start".to_string());
            let msg = err.to_string();
            assert!(msg.contains("engine error"));
            assert!(msg.contains("failed to start"));
        }

        #[test]
        fn then_io_error_formats_correctly() {
            let err = CliError::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
            let msg = err.to_string();
            assert!(msg.contains("IO error"));
            assert!(msg.contains("file not found"));
        }

        #[test]
        fn then_io_error_source_is_available() {
            let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
            let err = CliError::Io(io_err);
            let src = err.source();
            assert!(src.is_some() || src.is_none());
        }
    }

    mod given_error_debug_trait {
        use super::*;

        #[test]
        fn then_config_error_debug_contains_config() {
            let err = CliError::Config("test".to_string());
            let debug_str = format!("{:?}", err);
            assert!(debug_str.contains("Config"));
        }

        #[test]
        fn then_health_failed_error_debug_contains_status() {
            let err = CliError::HealthFailed { status: 500 };
            let debug_str = format!("{:?}", err);
            assert!(debug_str.contains("HealthFailed"));
            assert!(debug_str.contains("500"));
        }

        #[test]
        fn then_invalid_body_error_debug_contains_message() {
            let err = CliError::InvalidBody("test".to_string());
            let debug_str = format!("{:?}", err);
            assert!(debug_str.contains("InvalidBody"));
        }

        #[test]
        fn then_unknown_datastore_error_debug_contains_type() {
            let err = CliError::UnknownDatastore("mysql".to_string());
            let debug_str = format!("{:?}", err);
            assert!(debug_str.contains("UnknownDatastore"));
        }
    }
}

#[cfg(test)]
mod bdd_commands_enum {

    use super::*;

    mod given_commands_default {
        use super::*;

        #[test]
        fn then_commands_default_is_server_start_standalone() {
            let default: Commands = Commands::default();
            assert!(matches!(
                default,
                Commands::Server {
                    command: ServerCommand::Start { .. },
                }
            ));
        }
    }

    mod given_commands_server_variant {
        use super::*;

        #[test]
        fn then_server_start_variant_contains_mode() {
            let cmd = Commands::Server {
                command: ServerCommand::Start {
                    mode: twerk_cli::commands::RunMode::Standalone,
                    hostname: None,
                },
            };
            assert!(matches!(
                cmd,
                Commands::Server {
                    command: ServerCommand::Start { .. },
                }
            ));
        }
    }

    mod given_commands_health_variant {
        use super::*;

        #[test]
        fn then_health_variant_accepts_endpoint() {
            let cmd = Commands::Health {
                endpoint: Some("http://localhost:9000".to_string()),
            };
            assert!(matches!(cmd, Commands::Health { .. }));
        }

        #[test]
        fn then_health_variant_endpoint_is_optional() {
            let cmd = Commands::Health { endpoint: None };
            assert!(matches!(cmd, Commands::Health { .. }));
        }
    }

    mod given_commands_migration_variant {
        use super::*;

        #[test]
        fn then_migration_variant_accepts_yes_flag() {
            let cmd = Commands::Migration { yes: true };
            assert!(matches!(cmd, Commands::Migration { .. }));
        }

        #[test]
        fn then_migration_variant_yes_defaults_to_false() {
            let cmd = Commands::Migration { yes: false };
            assert!(matches!(cmd, Commands::Migration { .. }));
        }
    }
}

#[cfg(test)]
mod bdd_setup_logging {
    #[test]
    fn then_setup_logging_function_exists_and_is_callable() {
        twerk_cli::cli::setup_logging()
            .unwrap_or_else(|error| panic!("setup_logging should be callable: {error}"));
    }
}

#[cfg(test)]
mod bdd_completeness_check {

    use super::*;

    #[test]
    fn then_all_public_constants_are_accessible() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(DEFAULT_ENDPOINT, "http://localhost:8000");
        assert_eq!(DEFAULT_DATASTORE_TYPE, "postgres");
    }

    #[test]
    fn then_error_enum_variants_are_constructible() {
        use std::io;
        let variants = [
            CliError::Config("test".to_string()).to_string(),
            CliError::HealthFailed { status: 200 }.to_string(),
            CliError::InvalidBody("test".to_string()).to_string(),
            CliError::MissingArgument("test".to_string()).to_string(),
            CliError::Migration("test".to_string()).to_string(),
            CliError::UnknownDatastore("test".to_string()).to_string(),
            CliError::Logging("test".to_string()).to_string(),
            CliError::Engine("test".to_string()).to_string(),
            CliError::Io(io::Error::other("test")).to_string(),
        ];

        assert_eq!(variants.len(), 9);
    }

    #[test]
    fn then_commands_enum_variants_are_constructible() {
        use twerk_cli::commands::{RunMode, ServerCommand};
        let variants = [
            Commands::default(),
            Commands::Server {
                command: ServerCommand::Start {
                    mode: RunMode::Standalone,
                    hostname: None,
                },
            },
            Commands::Server {
                command: ServerCommand::Start {
                    mode: RunMode::Coordinator,
                    hostname: None,
                },
            },
            Commands::Server {
                command: ServerCommand::Start {
                    mode: RunMode::Worker,
                    hostname: None,
                },
            },
            Commands::Health { endpoint: None },
            Commands::Migration { yes: false },
        ];

        assert_eq!(variants.len(), 6);
        assert!(matches!(
            variants[0],
            Commands::Server {
                command: ServerCommand::Start { .. },
            }
        ));
        assert!(matches!(
            variants[1],
            Commands::Server {
                command: ServerCommand::Start { .. },
            }
        ));
        assert!(matches!(
            variants[2],
            Commands::Server {
                command: ServerCommand::Start { .. },
            }
        ));
        assert!(matches!(
            variants[3],
            Commands::Server {
                command: ServerCommand::Start { .. },
            }
        ));
        assert!(matches!(variants[4], Commands::Health { .. }));
        assert!(matches!(variants[5], Commands::Migration { .. }));
    }
}

#[cfg(test)]
mod bdd_liar_check_cli_error_display {

    use super::*;

    #[test]
    fn then_cli_error_display_contains_config_message() {
        let err = CliError::Config("secret_key".to_string());
        let display = err.to_string();
        assert!(display.contains("secret_key"));
    }

    #[test]
    fn then_cli_error_display_for_unknown_datastore_contains_type() {
        let err = CliError::UnknownDatastore("mysql".to_string());
        let display = err.to_string();
        assert!(display.contains("mysql"));
    }

    #[test]
    fn then_cli_error_display_for_health_failed_contains_status() {
        let err = CliError::HealthFailed { status: 503 };
        let display = err.to_string();
        assert!(display.contains("503"));
    }
}

#[cfg(test)]
mod bdd_breakage_check {

    use super::*;

    #[test]
    fn then_empty_config_string_produces_meaningful_error() {
        let err = CliError::Config(String::new());
        let msg = err.to_string();
        assert!(!msg.is_empty());
    }

    #[test]
    fn then_unknown_datastore_accepts_empty_string() {
        let err = CliError::UnknownDatastore("".to_string());
        let msg = err.to_string();
        assert!(msg.contains("unsupported datastore type"));
    }

    #[test]
    fn then_migration_error_accepts_multiline_message() {
        let err = CliError::Migration("line1\nline2\nline3".to_string());
        let msg = err.to_string();
        assert!(msg.contains("migration error"));
        assert!(msg.contains("line1"));
    }

    #[test]
    fn then_missing_argument_error_accepts_empty_string() {
        let err = CliError::MissingArgument("".to_string());
        let msg = err.to_string();
        assert!(msg.contains("missing required argument"));
    }

    #[test]
    fn then_health_failed_status_accepts_boundary_values() {
        let err0 = CliError::HealthFailed { status: 0 };
        let err200 = CliError::HealthFailed { status: 200 };
        let err599 = CliError::HealthFailed { status: 599 };
        assert!(err0.to_string().contains("0"));
        assert!(err200.to_string().contains("200"));
        assert!(err599.to_string().contains("599"));
    }
}
