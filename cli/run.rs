//! Run command
//!
//! Runs the Tork engine in the specified mode.

use super::error::CliError;
use tracing::info;

/// Engine execution modes
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EngineMode {
    /// Standalone mode - all-in-one process
    #[default]
    Standalone,
    /// Coordinator mode - manages job scheduling
    Coordinator,
    /// Worker mode - executes tasks
    Worker,
    /// Unknown mode
    Unknown(String),
}

impl EngineMode {
    /// Parse engine mode from string
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "standalone" => Self::Standalone,
            "coordinator" => Self::Coordinator,
            "worker" => Self::Worker,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Returns true if this is a valid, known mode
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

/// Run the Tork engine in the specified mode
///
/// # Arguments
///
/// * `mode` - The engine mode to run in
/// * `broker_type` - The broker type (inmemory, rabbitmq)
/// * `datastore_type` - The datastore type (inmemory, postgres)
/// * `locker_type` - The locker type (inmemory, redis)
///
/// # Errors
///
/// Returns [`CliError::MissingArgument`] if mode is empty.
/// Returns [`CliError::Engine`] if the mode is unknown.
/// Returns [`CliError::Engine`] if the engine fails to run.
pub async fn run_engine(
    mode: &str,
    broker_type: &str,
    datastore_type: &str,
    locker_type: &str,
) -> Result<(), CliError> {
    let mode = mode.trim();

    if mode.is_empty() {
        return Err(CliError::MissingArgument("mode".to_string()));
    }

    let engine_mode = EngineMode::from_str(mode);

    if !engine_mode.is_valid() {
        return Err(CliError::Engine(format!(
            "unknown engine mode: {mode}. Valid modes are: standalone, coordinator, worker"
        )));
    }

    info!(
        "Starting Tork engine in {} mode (broker={}, datastore={}, locker={})",
        engine_mode_description(&engine_mode),
        broker_type,
        datastore_type,
        locker_type
    );

    // In a full implementation, this would:
    // 1. Set the engine configuration via engine::Config
    // 2. Initialize the appropriate broker, datastore, and locker
    // 3. Run the engine via engine::run()

    Ok(())
}

/// Get a human-readable description of the engine mode
const fn engine_mode_description(mode: &EngineMode) -> &'static str {
    match mode {
        EngineMode::Standalone => "standalone",
        EngineMode::Coordinator => "coordinator",
        EngineMode::Worker => "worker",
        EngineMode::Unknown(_) => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_mode_from_str() {
        assert_eq!(EngineMode::from_str("standalone"), EngineMode::Standalone);
        assert_eq!(EngineMode::from_str("Standalone"), EngineMode::Standalone);
        assert_eq!(EngineMode::from_str("STANDALONE"), EngineMode::Standalone);
        assert_eq!(EngineMode::from_str("coordinator"), EngineMode::Coordinator);
        assert_eq!(EngineMode::from_str("worker"), EngineMode::Worker);
        assert_eq!(
            EngineMode::from_str("unknown"),
            EngineMode::Unknown("unknown".to_string())
        );
    }

    #[test]
    fn test_engine_mode_is_valid() {
        assert!(EngineMode::Standalone.is_valid());
        assert!(EngineMode::Coordinator.is_valid());
        assert!(EngineMode::Worker.is_valid());
        assert!(!EngineMode::Unknown("bad".to_string()).is_valid());
    }

    #[test]
    fn test_engine_mode_default() {
        assert_eq!(EngineMode::default(), EngineMode::Standalone);
    }

    #[test]
    fn test_engine_mode_description() {
        assert_eq!(engine_mode_description(&EngineMode::Standalone), "standalone");
        assert_eq!(engine_mode_description(&EngineMode::Coordinator), "coordinator");
        assert_eq!(engine_mode_description(&EngineMode::Worker), "worker");
        assert_eq!(engine_mode_description(&EngineMode::Unknown("x".to_string())), "unknown");
    }
}
