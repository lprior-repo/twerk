//! Run command
//!
//! Runs the Twerk engine in the specified mode.
//!
//! Go parity: `cli/run.go` → parses mode arg, calls `engine.SetMode(mode)` then `engine.Run()`.

use twerk_app::engine::{Config, Engine, Mode};

use crate::CliError;
use tracing::info;

/// Parse engine mode from a CLI argument string.
///
/// Maps to Go's `engine.Mode` type: `MODE_STANDALONE`, `MODE_COORDINATOR`, `MODE_WORKER`.
/// Returns `None` for unrecognized modes.
#[must_use]
pub fn parse_mode(s: &str) -> Option<Mode> {
    match s.trim().to_lowercase().as_str() {
        "standalone" => Some(Mode::Standalone),
        "coordinator" => Some(Mode::Coordinator),
        "worker" => Some(Mode::Worker),
        _ => None,
    }
}

/// Run the Twerk engine in the specified mode.
///
/// Go parity:
/// ```go
/// mode := ctx.Args().First()
/// engine.SetMode(engine.Mode(mode))
/// return engine.Run()
/// ```
///
/// # Arguments
///
/// * `mode` - The engine mode to run in (standalone, coordinator, worker)
///
/// # Errors
///
/// Returns [`CliError::MissingArgument`] if mode is empty.
/// Returns [`CliError::Engine`] if the mode is unknown.
/// Returns [`CliError::Engine`] if the engine fails to start or run.
pub async fn run_engine(mode: &str) -> Result<(), CliError> {
    if mode.trim().is_empty() {
        return Err(CliError::MissingArgument("mode".to_string()));
    }

    let engine_mode = parse_mode(mode).ok_or_else(|| {
        CliError::Engine(format!(
            "unknown engine mode: {mode}. Valid modes are: standalone, coordinator, worker"
        ))
    })?;

    info!("Starting Twerk engine in {engine_mode:?} mode");

    let config = Config {
        mode: engine_mode,
        ..Config::default()
    };

    let mut engine = Engine::new(config);
    engine
        .run()
        .await
        .map_err(|e| CliError::Engine(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mode_standalone() {
        assert_eq!(parse_mode("standalone"), Some(Mode::Standalone));
        assert_eq!(parse_mode("Standalone"), Some(Mode::Standalone));
        assert_eq!(parse_mode("STANDALONE"), Some(Mode::Standalone));
        assert_eq!(parse_mode("Standalone "), Some(Mode::Standalone));
        assert_eq!(parse_mode(" standalone"), Some(Mode::Standalone));
    }

    #[test]
    fn test_parse_mode_coordinator() {
        assert_eq!(parse_mode("coordinator"), Some(Mode::Coordinator));
        assert_eq!(parse_mode("Coordinator"), Some(Mode::Coordinator));
        assert_eq!(parse_mode("COORDINATOR"), Some(Mode::Coordinator));
        assert_eq!(parse_mode("Coordinator "), Some(Mode::Coordinator));
    }

    #[test]
    fn test_parse_mode_worker() {
        assert_eq!(parse_mode("worker"), Some(Mode::Worker));
        assert_eq!(parse_mode("Worker"), Some(Mode::Worker));
        assert_eq!(parse_mode("WORKER"), Some(Mode::Worker));
        assert_eq!(parse_mode(" worker "), Some(Mode::Worker));
    }

    #[test]
    fn test_parse_mode_invalid() {
        assert_eq!(parse_mode("bogus"), None);
        assert_eq!(parse_mode("unknown"), None);
        assert_eq!(parse_mode("mode123"), None);
        assert_eq!(parse_mode("stand-alone"), None);
        assert_eq!(parse_mode("coordinators"), None);
    }

    #[test]
    fn test_parse_mode_empty() {
        assert_eq!(parse_mode(""), None);
        assert_eq!(parse_mode("   "), None);
        assert_eq!(parse_mode("\t"), None);
        assert_eq!(parse_mode("\n"), None);
    }

    #[test]
    fn test_parse_mode_with_whitespace() {
        assert_eq!(parse_mode("  standalone  "), Some(Mode::Standalone));
        assert_eq!(parse_mode("  coordinator  "), Some(Mode::Coordinator));
        assert_eq!(parse_mode("  worker  "), Some(Mode::Worker));
    }
}
