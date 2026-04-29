//! CLI argument parsing, error handling, and command execution dispatch.

use clap::error::ErrorKind;
use clap::Parser;
use std::ffi::OsString;
use twerk_core::domain::Endpoint;

use crate::commands::{
    Cli, Commands, JobCommand, MetricsCommand, NodeCommand, QueueCommand,
    ScheduledJobCommand, TaskCommand, TriggerCommand, UserCommand,
};
use crate::error::CliError;
use crate::handlers;
use crate::health::health_check;
use crate::run::run_engine;

use super::help::{
    clap_error_kind_name, detect_help_variant, json_error_payload, json_help_payload,
    json_version_payload, print_json, render_help_for_path, HelpVariant,
};
use super::{get_endpoint, json_requested, os_string_eq};

#[allow(dead_code)]
const fn command_name(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::ServerStart { .. } => "server start",
        Commands::Health { .. } => "health",
        Commands::Version => "version",
        Commands::Job { .. } => "job",
        Commands::ScheduledJob { .. } => "scheduled-job",
        Commands::Task { .. } => "task",
        Commands::Queue { .. } => "queue",
        Commands::Trigger { .. } => "trigger",
        Commands::Node { .. } => "node",
        Commands::Metrics { .. } => "metrics",
        Commands::User { .. } => "user",
    }
}

pub(super) fn parse_cli_args(args: &[OsString]) -> Result<super::CliAction, clap::Error> {
    Cli::try_parse_from(args.iter().cloned())
        .map(|cli| super::CliAction::Execute(cli.command, cli.json))
}

pub(super) fn handle_parse_error(error: clap::Error, emit_json: bool) -> i32 {
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

pub(super) fn handle_runtime_error(error: CliError, emit_json: bool) -> i32 {
    if emit_json {
        let kind = error.kind().to_string();
        print_json(&json_error_payload(&kind, error.to_string(), None));
    } else {
        eprintln!("Error: {error}");
    }
    error.exit_code()
}

pub(super) fn handle_json_help_subcommand(args: &[OsString]) -> Option<i32> {
    let is_help_subcommand = args.get(1).is_some_and(|arg| os_string_eq(arg, "help"));
    if !json_requested(args) || !is_help_subcommand {
        return None;
    }

    let help_variant = detect_help_variant(args);

    let help_path = args
        .iter()
        .skip(2)
        .filter(|arg| !os_string_eq(arg, "--json") && !os_string_eq(arg, "--long"))
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let variant = match help_variant {
        HelpVariant::None => HelpVariant::Short, // Default to Short for `twerk help --json`
        other => other,
    };

    match render_help_for_path(&help_path, variant) {
        Ok(content) => {
            print_json(&json_help_payload(content));
            Some(super::ExitStatus::Success as i32)
        }
        Err(error) => Some(handle_runtime_error(error, true)),
    }
}

pub(super) async fn execute_command(command: Commands, json_mode: bool) -> Result<(), CliError> {
    match command {
        Commands::ServerStart { mode, hostname } => run_engine(mode, hostname).await,
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
            let content = format!("twerk {}\n", super::VERSION);
            if json_mode {
                print_json(&json_version_payload(content));
            } else {
                println!("twerk {}", super::VERSION);
            }
            Ok(())
        }
        Commands::Job { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                JobCommand::List => {
                    handlers::job::job_list(ep_str, json_mode).await?;
                }
                JobCommand::Create { body } => {
                    handlers::job::job_create(ep_str, &body, json_mode).await?;
                }
                JobCommand::Get { id } => {
                    handlers::job::job_get(ep_str, &id, json_mode).await?;
                }
                JobCommand::Log { id } => {
                    handlers::job::job_log(ep_str, &id, json_mode).await?;
                }
                JobCommand::Cancel { id } => {
                    handlers::job::job_cancel(ep_str, &id, json_mode).await?;
                }
                JobCommand::Restart { id } => {
                    handlers::job::job_restart(ep_str, &id, json_mode).await?;
                }
            }
            Ok(())
        }
        Commands::ScheduledJob { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                ScheduledJobCommand::List => {
                    handlers::scheduled_job::scheduled_job_list(ep_str, json_mode).await?;
                }
                ScheduledJobCommand::Create { body } => {
                    handlers::scheduled_job::scheduled_job_create(ep_str, &body, json_mode).await?;
                }
                ScheduledJobCommand::Get { id } => {
                    handlers::scheduled_job::scheduled_job_get(ep_str, &id, json_mode).await?;
                }
                ScheduledJobCommand::Delete { id } => {
                    handlers::scheduled_job::scheduled_job_delete(ep_str, &id, json_mode).await?;
                }
                ScheduledJobCommand::Pause { id } => {
                    handlers::scheduled_job::scheduled_job_pause(ep_str, &id, json_mode).await?;
                }
                ScheduledJobCommand::Resume { id } => {
                    handlers::scheduled_job::scheduled_job_resume(ep_str, &id, json_mode).await?;
                }
            }
            Ok(())
        }
        Commands::Task { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                TaskCommand::Get { id } => {
                    handlers::task::task_get(ep_str, &id, json_mode).await?;
                }
                TaskCommand::Log { id, page, size, q } => {
                    handlers::task::task_log(ep_str, &id, page, size, q, json_mode).await?;
                }
            }
            Ok(())
        }
        Commands::Queue { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                QueueCommand::List => {
                    handlers::queue::queue_list(ep_str, json_mode).await?;
                }
                QueueCommand::Get { name } => {
                    handlers::queue::queue_get(ep_str, &name, json_mode).await?;
                }
                QueueCommand::Delete { name } => {
                    handlers::queue::queue_delete(ep_str, &name, json_mode).await?;
                }
            }
            Ok(())
        }
        Commands::Trigger { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                TriggerCommand::List => {
                    handlers::trigger::trigger_list(ep_str, json_mode).await?;
                }
                TriggerCommand::Get { id } => {
                    handlers::trigger::trigger_get(ep_str, &id, json_mode).await?;
                }
                TriggerCommand::Create { body } => {
                    handlers::trigger::trigger_create(ep_str, &body, json_mode).await?;
                }
                TriggerCommand::Update { id, body } => {
                    handlers::trigger::trigger_update(ep_str, &id, &body, json_mode).await?;
                }
                TriggerCommand::Delete { id } => {
                    handlers::trigger::trigger_delete(ep_str, &id, json_mode).await?;
                }
            }
            Ok(())
        }
        Commands::Node { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                NodeCommand::List => {
                    handlers::node::node_list(ep_str, json_mode).await?;
                }
            }
            Ok(())
        }
        Commands::Metrics { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                MetricsCommand::Get => {
                    handlers::metrics::metrics_get(ep_str, json_mode).await?;
                }
            }
            Ok(())
        }
        Commands::User { command } => {
            let ep = get_endpoint()?;
            let ep_str = ep.as_str();
            match command {
                UserCommand::Create { username, password } => {
                    handlers::user::user_create(ep_str, &username, &password, json_mode).await?;
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliAction;
    use std::ffi::OsString;

    use clap::error::ErrorKind;

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
    fn parse_cli_args_returns_server_start_command_for_coordinator_mode() {
        let args = vec![
            OsString::from("twerk"),
            OsString::from("server-start"),
            OsString::from("coordinator"),
        ];

        assert!(matches!(
            parse_cli_args(&args),
            Ok(CliAction::Execute(
                Some(Commands::ServerStart {
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
}
