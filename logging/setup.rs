//! Logging setup
//!
//! Provides feature parity with Go's zerolog-based logging:
//! - Supports `"warn"` and `"warning"` as aliases for the warn level
//! - Supports `"pretty"` (human-readable) and `"json"` (structured) formats

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

/// Normalize a log level string, handling aliases.
///
/// Go's zerolog supports both `"warn"` and `"warning"` for the warn level.
/// `tracing::Level::from_str` only accepts `"warn"`, so we normalize.
fn normalize_level(raw: &str) -> String {
    match raw.to_lowercase().as_str() {
        "warning" => "warn".to_string(),
        other => other.to_string(),
    }
}

/// Setup logging based on configuration.
///
/// Reads configuration from:
/// - `logging.level`: Log level (debug, info, warn, warning, error). Defaults to "debug".
/// - `logging.format`: Log format (pretty, json). Defaults to "pretty".
///
/// # Errors
///
/// Returns [`LoggingError`] if invalid log level or format is specified.
pub fn setup_logging() -> Result<(), LoggingError> {
    let raw_level = conf::string_default("logging.level", "debug");
    let log_format = conf::string_default("logging.format", "pretty");

    let normalized = normalize_level(&raw_level);
    let level = Level::from_str(&normalized)
        .map_err(|_| LoggingError::InvalidLevel { level: raw_level })?;

    // The hardcoded "tork_runtime=debug" directive is always valid.
    // If parsing somehow fails, fall back to the user's configured level.
    let tork_runtime_directive: tracing_subscriber::filter::Directive = "tork_runtime=debug"
        .parse()
        .unwrap_or_else(|_| level.into());

    let env_filter = EnvFilter::from_default_env()
        .add_directive(level.into())
        .add_directive(tork_runtime_directive);

    match log_format.to_lowercase().as_str() {
        "pretty" => {
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
        }
        "json" => {
            let fmt_layer = fmt::layer()
                .json()
                .with_target(true)
                .with_span_events(FmtSpan::CLOSE);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();
        }
        other => {
            return Err(LoggingError::InvalidFormat {
                format: other.to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_level_warning() {
        assert_eq!(normalize_level("warning"), "warn");
        assert_eq!(normalize_level("WARNING"), "warn");
        assert_eq!(normalize_level("warn"), "warn");
        assert_eq!(normalize_level("info"), "info");
        assert_eq!(normalize_level("debug"), "debug");
    }

    #[test]
    fn test_normalize_level_preserves_valid() {
        assert_eq!(normalize_level("error"), "error");
        assert_eq!(normalize_level("trace"), "trace");
    }
}
