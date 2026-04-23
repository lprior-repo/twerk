//! Logging utilities
//!
//! Provides logging setup and a writer that adapts to tracing.

use std::io;
use std::str::FromStr;

use thiserror::Error;

// ============================================================================
// Error types
// ============================================================================

/// Domain errors for logging operations
#[derive(Debug, Error)]
pub enum LoggingError {
    /// Invalid log level specified.
    #[error("invalid logging level: {level}")]
    InvalidLevel { level: String },

    /// Invalid log format specified.
    #[error("invalid logging format: {format}")]
    InvalidFormat { format: String },
}

// ============================================================================
// Log level enum and conversions
// ============================================================================

/// Log level for tracing writer.
#[derive(Debug, Clone, Copy)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<Level> for tracing::Level {
    fn from(level: Level) -> Self {
        match level {
            Level::Trace => tracing::Level::TRACE,
            Level::Debug => tracing::Level::DEBUG,
            Level::Info => tracing::Level::INFO,
            Level::Warn => tracing::Level::WARN,
            Level::Error => tracing::Level::ERROR,
        }
    }
}

// ============================================================================
// TracingWriter
// ============================================================================

/// A writer that logs each write at a specified level.
///
/// This is useful for capturing output from processes and logging
/// it with the task ID context.
#[derive(Debug)]
pub struct TracingWriter {
    task_id: String,
    level: Level,
}

impl TracingWriter {
    /// Create a new `TracingWriter`.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to include in log entries
    /// * `level` - The log level to use
    #[must_use]
    pub fn new(task_id: String, level: Level) -> Self {
        Self { task_id, level }
    }

    /// Write a log entry.
    ///
    /// The entire contents are logged as a single log line.
    #[allow(clippy::cognitive_complexity)]
    pub fn write(&self, contents: &str) {
        let line = contents.trim_end();
        if line.is_empty() {
            return;
        }

        let span = tracing::info_span!("task_log", task_id = %self.task_id);
        let _guard = span.enter();

        match self.level {
            Level::Trace => tracing::trace!(line),
            Level::Debug => tracing::debug!(line),
            Level::Info => tracing::info!(line),
            Level::Warn => tracing::warn!(line),
            Level::Error => tracing::error!(line),
        }
    }
}

impl io::Write for TracingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Convert bytes to string, ignoring invalid UTF-8
        let contents = String::from_utf8_lossy(buf);
        TracingWriter::write(self, &contents);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Alias for `TracingWriter` to match Go's `ZerologWriter` API.
///
/// Go's `NewZerologWriter` creates a writer that logs each line with task-id context.
/// Rust uses tracing instead of zerolog, but the interface is the same.
pub type ZerologWriter = TracingWriter;

/// Create a new `ZerologWriter` aliased to `TracingWriter::new` for Go parity.
#[must_use]
pub fn new_zerolog_writer(task_id: String, level: Level) -> ZerologWriter {
    TracingWriter::new(task_id, level)
}

// ============================================================================
// Normalize level
// ============================================================================

/// Normalize a log level string, handling aliases.
///
/// Go's zerolog supports both `"warn"` and `"warning"` for the warn level.
fn normalize_level(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "warning" => "warn".to_string(),
        other => other.to_string(),
    }
}

// ============================================================================
// SetupLogging
// ============================================================================

/// Setup logging based on configuration using tracing.
///
/// This is the primary logging setup function.
///
/// Reads configuration from environment variables (via config system):
/// - `TWERK_LOGGING_LEVEL`: Log level (debug, info, warn, warning, error). Defaults to "debug".
/// - `TWERK_LOGGING_FORMAT`: Log format (pretty, json). Defaults to "pretty".
///
/// # Errors
///
/// Returns [`LoggingError`] if invalid log level or format is specified.
pub fn setup_logging() -> Result<(), LoggingError> {
    use tracing::Level;
    use tracing_subscriber::{
        fmt::{self, format::FmtSpan},
        layer::SubscriberExt,
        util::SubscriberInitExt,
        EnvFilter,
    };

    let raw_level = crate::conf::string_default("logging.level", "debug");
    let log_format = crate::conf::string_default("logging.format", "pretty");

    let normalized = normalize_level(&raw_level);
    let level = Level::from_str(&normalized)
        .map_err(|_| LoggingError::InvalidLevel { level: raw_level })?;

    // The hardcoded "twerk_runtime=debug" directive is always valid.
    // If parsing somehow fails, fall back to the user's configured level.
    let twerk_runtime_directive: tracing_subscriber::filter::Directive =
        match "twerk_runtime=debug".parse::<tracing_subscriber::filter::Directive>() {
            Ok(d) => d,
            Err(_) => level.into(),
        };

    let env_filter = EnvFilter::from_default_env()
        .add_directive(level.into())
        .add_directive(twerk_runtime_directive);

    match log_format.to_ascii_lowercase().as_str() {
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::redundant_pattern_matching)]
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
    }

    #[test]
    fn test_tracing_writer_write() {
        let writer = TracingWriter::new("task-123".to_string(), Level::Info);

        // Should not panic when writing
        writer.write("test log line\n");
        writer.write("");
    }

    #[test]
    fn test_tracing_writer_as_trait() {
        use std::io::Write;

        let mut writer = TracingWriter::new("task-456".to_string(), Level::Debug);

        // Should implement Write trait
        let result = writer.write_all(b"test output\n");
        assert!(matches!(result, Ok(_)));

        let result = writer.flush();
        assert!(matches!(result, Ok(_)));
    }

    #[test]
    fn test_level_to_tracing_level() {
        assert_eq!(tracing::Level::from(Level::Trace), tracing::Level::TRACE);
        assert_eq!(tracing::Level::from(Level::Debug), tracing::Level::DEBUG);
        assert_eq!(tracing::Level::from(Level::Info), tracing::Level::INFO);
        assert_eq!(tracing::Level::from(Level::Warn), tracing::Level::WARN);
        assert_eq!(tracing::Level::from(Level::Error), tracing::Level::ERROR);
    }
}
