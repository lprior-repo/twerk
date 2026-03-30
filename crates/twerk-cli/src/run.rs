//! Run command
//!
//! Runs the Twerk engine in the specified mode.
//!
//! Go parity: `cli/run.go` → parses mode arg, calls `engine.SetMode(mode)` then `engine.Run()`.

use twerk_app::engine::{Config, Engine, Mode};

use crate::commands::RunMode;
use crate::CliError;
use tracing::info;

impl RunMode {
    /// Convert CLI run mode into engine mode.
    #[must_use]
    pub const fn into_engine_mode(self) -> Mode {
        match self {
            Self::Standalone => Mode::Standalone,
            Self::Coordinator => Mode::Coordinator,
            Self::Worker => Mode::Worker,
        }
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
/// Returns [`CliError::Engine`] if the engine fails to start or run.
pub async fn run_engine(mode: RunMode) -> Result<(), CliError> {
    let engine_mode = mode.into_engine_mode();

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
    fn test_run_mode_maps_standalone() {
        assert_eq!(RunMode::Standalone.into_engine_mode(), Mode::Standalone);
    }

    #[test]
    fn test_run_mode_maps_coordinator() {
        assert_eq!(RunMode::Coordinator.into_engine_mode(), Mode::Coordinator);
    }

    #[test]
    fn test_run_mode_maps_worker() {
        assert_eq!(RunMode::Worker.into_engine_mode(), Mode::Worker);
    }
}
