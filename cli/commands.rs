//! CLI command definitions
//!
//! Defines the command-line interface using clap.

use clap::Parser;

/// CLI commands
#[derive(Debug, Parser)]
#[command(name = "tork")]
#[command(about = "A distributed workflow engine", long_about = None)]
pub enum Commands {
    /// Run the Tork engine
    Run {
        /// The mode to run in (standalone, coordinator, worker)
        #[arg(value_name = "mode")]
        mode: Option<String>,
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
        Self::Run { mode: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_commands_derive() {
        // Verify Commands struct can be created
        let cmd = Commands::Run {
            mode: Some("standalone".to_string()),
        };
        assert!(matches!(cmd, Commands::Run { .. }));
    }

    #[test]
    fn test_commands_default() {
        let cmd = Commands::default();
        assert!(matches!(cmd, Commands::Run { mode: None }));
    }

    #[test]
    fn test_commands_help() {
        let mut cmd = Commands::command();
        // Just verify it doesn't panic
        let _ = cmd.render_help().to_string();
    }
}
