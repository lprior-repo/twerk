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
#[command(after_help = HELP_EXAMPLES)]
pub struct Cli {
    /// Enable JSON output for automation and scripting.
    /// When enabled, commands output structured JSON instead of human-readable text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Commands>,
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

/// Task subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum TaskCommand {
    /// Get a task by ID
    Get {
        /// The task ID
        #[arg(required = true)]
        id: String,
    },
    /// Get task log entries
    Log {
        /// The task ID
        #[arg(required = true)]
        id: String,
        /// Page number
        #[arg(long)]
        page: Option<i64>,
        /// Page size
        #[arg(long)]
        size: Option<i64>,
    },
}

/// Queue subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum QueueCommand {
    /// List all queues
    List,
    /// Get a queue by name
    Get {
        /// The queue name
        #[arg(required = true)]
        name: String,
    },
    /// Delete a queue by name
    Delete {
        /// The queue name
        #[arg(required = true)]
        name: String,
    },
}

/// Trigger subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum TriggerCommand {
    /// List all triggers
    List,
    /// Get a trigger by ID
    Get {
        /// The trigger ID
        #[arg(required = true)]
        id: String,
    },
    /// Create a trigger
    Create {
        /// JSON body for the trigger
        #[arg(required = true)]
        body: String,
    },
    /// Update a trigger
    Update {
        /// The trigger ID
        #[arg(required = true)]
        id: String,
        /// JSON body for the update
        #[arg(required = true)]
        body: String,
    },
    /// Delete a trigger by ID
    Delete {
        /// The trigger ID
        #[arg(required = true)]
        id: String,
    },
}

/// CLI subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Commands {
    /// Run the Twerk engine
    Run {
        /// The mode to run in (standalone, coordinator, worker)
        #[arg(value_name = "mode", required = true)]
        mode: RunMode,

        /// The coordinator hostname for workers to connect to
        #[arg(long)]
        hostname: Option<String>,
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
    /// Show version information
    Version,
    /// Task operations
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Queue operations
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
    /// Trigger operations
    Trigger {
        #[command(subcommand)]
        command: TriggerCommand,
    },
}

impl Default for Commands {
    fn default() -> Self {
        Self::Run {
            mode: RunMode::Standalone,
            hostname: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn test_commands_derive() {
        let cmd = Commands::Run {
            mode: RunMode::Standalone,
            hostname: None,
        };
        assert!(matches!(cmd, Commands::Run { .. }));
    }

    #[test]
    fn test_commands_default() {
        let cmd = Commands::default();
        assert!(matches!(
            cmd,
            Commands::Run {
                ref mode,
                hostname: None
            } if mode == &RunMode::Standalone
        ));
    }

    #[test]
    fn test_commands_help() {
        use clap::CommandFactory;

        let mut cmd = Cli::command();
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
                json: false,
                command: Some(Commands::Run {
                    mode: RunMode::Worker,
                    hostname: None
                })
            })
        ));
    }

    #[test]
    fn test_version_subcommand_is_supported() {
        let cli = Cli::try_parse_from(["twerk", "version"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Version)
            })
        ));
    }

    #[test]
    fn test_task_get_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "task", "get", "task-123"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Task { command: TaskCommand::Get { ref id } })
            }) if id == "task-123"
        ));
    }

    #[test]
    fn test_task_log_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "task", "log", "task-456"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Task { command: TaskCommand::Log { ref id, .. } })
            }) if id == "task-456"
        ));
    }

    #[test]
    fn test_queue_subcommands_parse() {
        let list = Cli::try_parse_from(["twerk", "queue", "list"]);
        assert!(matches!(
            list,
            Ok(Cli { command: Some(Commands::Queue { command: QueueCommand::List }), .. })
        ));

        let get = Cli::try_parse_from(["twerk", "queue", "get", "my-queue"]);
        assert!(matches!(
            get,
            Ok(Cli { command: Some(Commands::Queue { command: QueueCommand::Get { ref name } }), .. })
            if name == "my-queue"
        ));
    }

    #[test]
    fn test_trigger_subcommands_parse() {
        let list = Cli::try_parse_from(["twerk", "trigger", "list"]);
        assert!(matches!(
            list,
            Ok(Cli { command: Some(Commands::Trigger { command: TriggerCommand::List }), .. })
        ));

        let get = Cli::try_parse_from(["twerk", "trigger", "get", "trig-1"]);
        assert!(matches!(
            get,
            Ok(Cli { command: Some(Commands::Trigger { command: TriggerCommand::Get { ref id } }), .. })
            if id == "trig-1"
        ));
    }
}
