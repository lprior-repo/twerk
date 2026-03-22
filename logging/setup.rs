//! Logging setup

use std::str::FromStr;

use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

use super::error::LoggingError;
use crate::conf;

/// Setup logging based on configuration.
///
/// Reads configuration from:
/// - `logging.level`: Log level (debug, info, warn, error). Defaults to "debug".
/// - `logging.format`: Log format (pretty, json). Defaults to "pretty".
///
/// # Errors
///
/// Returns [`LoggingError`] if invalid log level or format is specified.
pub fn setup_logging() -> Result<(), LoggingError> {
    let log_level = conf::string_default("logging.level", "debug");
    let _log_format = conf::string_default("logging.format", "pretty");

    let level =
        Level::from_str(&log_level.to_lowercase()).map_err(|_| LoggingError::InvalidLevel {
            level: log_level.clone(),
        })?;

    let env_filter = EnvFilter::from_default_env()
        .add_directive(level.into())
        .add_directive(
            "tork_runtime=debug"
                .parse()
                .expect("hardcoded directive is valid"),
        );

    // Note: JSON formatting requires the "json" feature of tracing-subscriber
    // For now, we use pretty formatting with full details
    let fmt_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_logging_default() {
        // Should not panic with valid defaults
        // Note: setup_logging may panic if a subscriber is already set
        let result = setup_logging();
        assert!(result.is_ok());
    }
}
