//! Domain errors for CLI operations

use thiserror::Error;
use twerk_core::domain::{DsnError, EndpointError};

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

    /// Invalid hostname
    #[error("invalid hostname: {0}")]
    InvalidHostname(String),

    /// Invalid endpoint URL
    #[error("invalid endpoint: {0}")]
    InvalidEndpoint(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<DsnError> for CliError {
    fn from(e: DsnError) -> Self {
        CliError::Migration(e.to_string())
    }
}

impl From<EndpointError> for CliError {
    fn from(e: EndpointError) -> Self {
        CliError::InvalidEndpoint(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_cli_error_config() {
        let err = CliError::Config("missing key".to_string());
        assert!(err.to_string().contains("configuration error"));
        assert!(err.to_string().contains("missing key"));
    }

    #[test]
    fn test_cli_error_health_failed() {
        let err = CliError::HealthFailed { status: 503 };
        assert!(err.to_string().contains("health check failed"));
        assert!(err.to_string().contains("503"));
    }

    #[test]
    fn test_cli_error_invalid_body() {
        let err = CliError::InvalidBody("not json".to_string());
        assert!(err.to_string().contains("invalid response body"));
        assert!(err.to_string().contains("not json"));
    }

    #[test]
    fn test_cli_error_missing_argument() {
        let err = CliError::MissingArgument("mode".to_string());
        assert!(err.to_string().contains("missing required argument"));
        assert!(err.to_string().contains("mode"));
    }

    #[test]
    fn test_cli_error_migration() {
        let err = CliError::Migration("connection refused".to_string());
        assert!(err.to_string().contains("migration error"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn test_cli_error_unknown_datastore() {
        let err = CliError::UnknownDatastore("mysql".to_string());
        assert!(err.to_string().contains("unsupported datastore type"));
        assert!(err.to_string().contains("mysql"));
    }

    #[test]
    fn test_cli_error_logging() {
        let err = CliError::Logging("invalid level".to_string());
        assert!(err.to_string().contains("logging setup error"));
        assert!(err.to_string().contains("invalid level"));
    }

    #[test]
    fn test_cli_error_engine() {
        let err = CliError::Engine("failed to start".to_string());
        assert!(err.to_string().contains("engine error"));
        assert!(err.to_string().contains("failed to start"));
    }

    #[test]
    fn test_cli_error_io() {
        let err = CliError::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        assert!(err.to_string().contains("IO error"));
        assert!(err.to_string().contains("file not found"));
    }

    #[test]
    fn test_cli_error_debug() {
        let err = CliError::Config("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Config"));
    }
}
