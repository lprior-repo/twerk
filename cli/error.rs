//! Domain errors for CLI operations

use thiserror::Error;

/// Errors that can occur during CLI operations
#[derive(Debug, Error)]
pub enum CliError {
    /// Configuration error
    #[error("configuration error: {0}")]
    Config(String),

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Health check failed with non-OK status
    #[error("health check failed with status: {status}")]
    HealthFailed { status: u16 },

    /// Invalid response body
    #[error("invalid response body: {0}")]
    InvalidBody(String),

    /// Missing required argument
    #[error("missing required argument: {0}")]
    MissingArgument(String),

    /// Datastore migration error
    #[error("migration error: {0}")]
    Migration(String),

    /// Unknown datastore type
    #[error("unsupported datastore type: {0}")]
    UnknownDatastore(String),

    /// Logging setup error
    #[error("logging setup error: {0}")]
    Logging(String),

    /// Engine error
    #[error("engine error: {0}")]
    Engine(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
