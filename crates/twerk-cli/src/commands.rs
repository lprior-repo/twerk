//! CLI command definitions.

use clap::{Parser, Subcommand, ValueEnum};

const HELP_EXAMPLES: &str = "Examples:\n  twerk server-start standalone\n  twerk health --endpoint http://localhost:8000\n  twerk job list";

#[derive(Debug, Clone, Parser)]
#[command(name = "twerk")]
#[command(bin_name = "twerk")]
#[command(about = "A distributed workflow engine", long_about = None)]
#[command(version = crate::cli::VERSION, propagate_version = true)]
#[command(after_help = HELP_EXAMPLES)]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum RunMode {
    Standalone,
    Coordinator,
    Worker,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum JobCommand {
    List,
    Create {
        #[arg(required = true)]
        body: String,
    },
    Get {
        #[arg(required = true)]
        id: String,
    },
    Log {
        #[arg(required = true)]
        id: String,
    },
    Cancel {
        #[arg(required = true)]
        id: String,
    },
    Restart {
        #[arg(required = true)]
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ScheduledJobCommand {
    List,
    Create {
        #[arg(required = true)]
        body: String,
    },
    Get {
        #[arg(required = true)]
        id: String,
    },
    Delete {
        #[arg(required = true)]
        id: String,
    },
    Pause {
        #[arg(required = true)]
        id: String,
    },
    Resume {
        #[arg(required = true)]
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum TaskCommand {
    Get {
        #[arg(required = true)]
        id: String,
    },
    Log {
        #[arg(required = true)]
        id: String,
        #[arg(long)]
        page: Option<i64>,
        #[arg(long)]
        size: Option<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum QueueCommand {
    List,
    Get {
        #[arg(required = true)]
        name: String,
    },
    Delete {
        #[arg(required = true)]
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum TriggerCommand {
    List,
    Get {
        #[arg(required = true)]
        id: String,
    },
    Create {
        #[arg(required = true)]
        body: String,
    },
    Update {
        #[arg(required = true)]
        id: String,
        #[arg(required = true)]
        body: String,
    },
    Delete {
        #[arg(required = true)]
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum NodeCommand {
    List,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum MetricsCommand {
    Get,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum UserCommand {
    Create {
        #[arg(required = true)]
        username: String,
        /// The password for the new user
        #[arg(required = true)]
        password: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Commands {
    ServerStart {
        #[arg(value_name = "mode", required = true)]
        mode: RunMode,
        #[arg(long)]
        hostname: Option<String>,
    },
    Health {
        #[arg(long, short = 'e')]
        endpoint: Option<String>,
    },
    Version,
    Job {
        #[command(subcommand)]
        command: JobCommand,
    },
    ScheduledJob {
        #[command(subcommand)]
        command: ScheduledJobCommand,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    Queue {
        #[command(subcommand)]
        command: QueueCommand,
    },
    Trigger {
        #[command(subcommand)]
        command: TriggerCommand,
    },
    Node {
        #[command(subcommand)]
        command: NodeCommand,
    },
    Metrics {
        #[command(subcommand)]
        command: MetricsCommand,
    },
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
}

impl Default for Commands {
    fn default() -> Self {
        Self::ServerStart {
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
    fn commands_server_start_variant_matches_shape_when_constructed_directly() {
        let cmd = Commands::ServerStart {
            mode: RunMode::Standalone,
            hostname: None,
        };
        assert!(matches!(cmd, Commands::ServerStart { .. }));
    }

    #[test]
    fn commands_default_returns_standalone_server_start_when_no_override_exists() {
        let cmd = Commands::default();
        assert!(matches!(
            cmd,
            Commands::ServerStart {
                ref mode,
                hostname: None
            } if mode == &RunMode::Standalone
        ));
    }

    #[test]
    fn cli_command_renders_help_without_panicking() {
        use clap::CommandFactory;

        let mut cmd = Cli::command();
        let help = cmd.render_help().to_string();
        assert!(help.contains("Usage:"));
    }

    #[test]
    fn help_contains_examples_when_rendered() {
        use clap::CommandFactory;

        let help = Cli::command().render_long_help().to_string();

        assert!(help.contains("Examples:"));
        assert!(help.contains("twerk server-start standalone"));
        assert!(help.contains("twerk health --endpoint"));
    }

    #[test]
    fn version_flag_is_supported_when_requested() {
        match Cli::try_parse_from(["twerk", "--version"]) {
            Ok(_) => unreachable!("expected clap to short-circuit with version output"),
            Err(error) => assert_eq!(error.kind(), ErrorKind::DisplayVersion),
        }
    }

    #[test]
    fn server_start_accepts_known_modes_when_parsing() {
        let cli = Cli::try_parse_from(["twerk", "server-start", "worker"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::ServerStart {
                    mode: RunMode::Worker,
                    hostname: None
                })
            })
        ));
    }

    #[test]
    fn version_subcommand_is_supported_when_parsing_cli() {
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
    fn job_list_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "job", "list"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Job { command: JobCommand::List })
            })
        ));
    }

    #[test]
    fn job_create_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "job", "create", r#"{"name":"test"}"#]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Job { command: JobCommand::Create { ref body } })
            }) if body == r#"{"name":"test"}"#
        ));
    }

    #[test]
    fn job_get_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "job", "get", "job-123"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Job { command: JobCommand::Get { ref id } })
            }) if id == "job-123"
        ));
    }

    #[test]
    fn job_cancel_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "job", "cancel", "job-123"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Job { command: JobCommand::Cancel { ref id } })
            }) if id == "job-123"
        ));
    }

    #[test]
    fn job_restart_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "job", "restart", "job-123"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::Job { command: JobCommand::Restart { ref id } })
            }) if id == "job-123"
        ));
    }

    #[test]
    fn scheduled_job_list_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "scheduled-job", "list"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::ScheduledJob { command: ScheduledJobCommand::List })
            })
        ));
    }

    #[test]
    fn scheduled_job_create_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "scheduled-job", "create", r#"{"name":"test"}"#]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::ScheduledJob { command: ScheduledJobCommand::Create { ref body } })
            }) if body == r#"{"name":"test"}"#
        ));
    }

    #[test]
    fn scheduled_job_delete_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "scheduled-job", "delete", "sj-123"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::ScheduledJob { command: ScheduledJobCommand::Delete { ref id } })
            }) if id == "sj-123"
        ));
    }

    #[test]
    fn scheduled_job_pause_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "scheduled-job", "pause", "sj-123"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::ScheduledJob { command: ScheduledJobCommand::Pause { ref id } })
            }) if id == "sj-123"
        ));
    }

    #[test]
    fn scheduled_job_resume_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "scheduled-job", "resume", "sj-123"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                json: false,
                command: Some(Commands::ScheduledJob { command: ScheduledJobCommand::Resume { ref id } })
            }) if id == "sj-123"
        ));
    }

    #[test]
    fn task_get_subcommand_parses_when_id_is_present() {
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
    fn task_log_subcommand_parses_when_id_is_present() {
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
    fn queue_subcommands_parse_when_list_and_get_are_requested() {
        let list = Cli::try_parse_from(["twerk", "queue", "list"]);
        assert!(matches!(
            list,
            Ok(Cli {
                command: Some(Commands::Queue {
                    command: QueueCommand::List
                }),
                ..
            })
        ));

        let get = Cli::try_parse_from(["twerk", "queue", "get", "my-queue"]);
        assert!(matches!(
            get,
            Ok(Cli { command: Some(Commands::Queue { command: QueueCommand::Get { ref name } }), .. })
            if name == "my-queue"
        ));
    }

    #[test]
    fn trigger_subcommands_parse_when_list_and_get_are_requested() {
        let list = Cli::try_parse_from(["twerk", "trigger", "list"]);
        assert!(matches!(
            list,
            Ok(Cli {
                command: Some(Commands::Trigger {
                    command: TriggerCommand::List
                }),
                ..
            })
        ));

        let get = Cli::try_parse_from(["twerk", "trigger", "get", "trig-1"]);
        assert!(matches!(
            get,
            Ok(Cli { command: Some(Commands::Trigger { command: TriggerCommand::Get { ref id } }), .. })
            if id == "trig-1"
        ));
    }

    #[test]
    fn node_list_subcommand_parses() {
        let cli = Cli::try_parse_from(["twerk", "node", "list"]);

        assert!(matches!(
            cli,
            Ok(Cli {
                command: Some(Commands::Node { command: NodeCommand::List }),
                ..
            })
        ));
    }
}
