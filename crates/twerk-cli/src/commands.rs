//! CLI command definitions.

use clap::{Parser, Subcommand, ValueEnum};

/// Concrete command examples shown in top-level help output.
const HELP_EXAMPLES: &str = "Examples:\n  twerk run standalone\n  twerk migration --yes\n  twerk health --endpoint http://localhost:8000";

/// Top-level CLI parser.
#[derive(Debug, Clone, Parser)]
#[command(name = "twerk")]
#[command(bin_name = "twerk")]
#[command(about = "A distributed workflow engine", long_about = None)]
#[command(version = crate::cli::VERSION, propagate_version = true)]
#[command(arg_required_else_help = true, after_help = HELP_EXAMPLES)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Supported engine run modes.
#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum RunMode {
    /// Run a single-node instance.
    Standalone,
    /// Run only the coordinator role.
    Coordinator,
    /// Run only the worker role.
    Worker,
}

/// CLI subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Commands {
    /// Run the Twerk engine
    Run {
        /// The mode to run in (standalone, coordinator, worker)
        #[arg(value_name = "mode", required = true)]
        mode: RunMode,
    },
    /// Run database migration
    Migration {
        /// Skip confirmation prompt (for automation)
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Perform a health check
    Health {
        /// The endpoint to check (defaults to <http://localhost:8000>)
        #[arg(long, short = 'e')]
        endpoint: Option<String>,
    },
}

impl Default for Commands {
    fn default() -> Self {
        Self::Run {
            mode: RunMode::Standalone,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn test_commands_derive() {
        // Verify Commands struct can be created
        let cmd = Commands::Run {
            mode: RunMode::Standalone,
        };
        assert!(matches!(cmd, Commands::Run { .. }));
    }

    #[test]
    fn test_commands_default() {
        let cmd = Commands::default();
        assert!(matches!(
            cmd,
            Commands::Run {
                ref mode
            } if mode == &RunMode::Standalone
        ));
    }

    #[test]
    fn test_commands_help() {
        use clap::CommandFactory;

        let mut cmd = Cli::command();
        // Just verify it doesn't panic
        let _ = cmd.render_help().to_string();
    }

    #[test]
    fn test_help_contains_examples() {
        use clap::CommandFactory;

        let help = Cli::command().render_long_help().to_string();

        assert!(help.contains("Examples:"));
        assert!(help.contains("twerk run standalone"));
        assert!(help.contains("twerk migration --yes"));
        assert!(help.contains("twerk health --endpoint http://localhost:8000"));
    }

    #[test]
    fn test_version_flag_is_supported() {
        match Cli::try_parse_from(["twerk", "--version"]) {
            Ok(_) => unreachable!("expected clap to short-circuit with version output"),
            Err(error) => assert_eq!(error.kind(), ErrorKind::DisplayVersion),
        }
    }

    #[test]
    fn test_run_mode_value_parser_accepts_known_modes() {
        let cli = Cli::try_parse_from(["twerk", "run", "worker"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                command: Commands::Run {
                    mode: RunMode::Worker
                }
            })
        ));
    }
}
