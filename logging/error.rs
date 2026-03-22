//! Domain errors for logging operations

use thiserror::Error;

/// Errors that can occur during logging setup.
#[derive(Debug, Error)]
pub enum LoggingError {
    /// Invalid log level specified.
    #[error("invalid logging level: {level}")]
    InvalidLevel { level: String },

    /// Invalid log format specified.
    #[error("invalid logging format: {format}")]
    InvalidFormat { format: String },
}
