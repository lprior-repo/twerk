//! Domain errors for CLI operations

use thiserror::Error;
use twerk_core::domain::{DsnError, EndpointError};

/// Category of error for exit code determination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Validation error (exit code 2) - invalid input, configuration, or arguments
    Validation,
    /// Runtime error (exit code 1) - operations that failed during execution
    Runtime,
}

impl ErrorKind {
    /// Returns the exit code associated with this error kind
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Validation => 2,
            Self::Runtime => 1,
        }
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation => write!(f, "validation"),
            Self::Runtime => write!(f, "runtime"),
        }
    }
}

/// Errors that can occur during CLI operations
#[derive(Debug, Error)]
pub enum CliError {
    /// Configuration error
    #[error("configuration error: {0}")]
    Config(String),

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// HTTP response with non-OK status
    #[error("HTTP error {status}: {reason}")]
    HttpStatus { status: u16, reason: String },

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

    /// Resource not found
    #[error("not found: {0}")]
    NotFound(String),

    /// API error response
    #[error("API error {code}: {message}")]
    ApiError { code: u16, message: String },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<DsnError> for CliError {
    fn from(e: DsnError) -> Self {
        Self::Migration(e.to_string())
    }
}

impl From<EndpointError> for CliError {
    fn from(e: EndpointError) -> Self {
        Self::InvalidEndpoint(e.to_string())
    }
}

impl CliError {
    /// Returns the category of this error
    pub const fn kind(&self) -> ErrorKind {
        match self {
            Self::InvalidEndpoint(_) => ErrorKind::Validation,
            Self::MissingArgument(_) => ErrorKind::Validation,
            Self::InvalidHostname(_) => ErrorKind::Validation,
            Self::UnknownDatastore(_) => ErrorKind::Validation,
            Self::Config(_) => ErrorKind::Validation,
            Self::Http(_) => ErrorKind::Runtime,
            Self::HttpStatus { .. } => ErrorKind::Runtime,
            Self::HealthFailed { .. } => ErrorKind::Runtime,
            Self::InvalidBody(_) => ErrorKind::Runtime,
            Self::Migration(_) => ErrorKind::Runtime,
            Self::Logging(_) => ErrorKind::Runtime,
            Self::Engine(_) => ErrorKind::Runtime,
            Self::NotFound(_) => ErrorKind::Runtime,
            Self::ApiError { .. } => ErrorKind::Runtime,
            Self::Io(_) => ErrorKind::Runtime,
        }
    }

    /// Returns the exit code for this error
    pub const fn exit_code(&self) -> i32 {
        self.kind().exit_code()
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
    fn test_error_kind_exit_codes() {
        assert_eq!(ErrorKind::Validation.exit_code(), 2);
        assert_eq!(ErrorKind::Runtime.exit_code(), 1);
    }

    #[test]
    fn test_error_kind_display() {
        assert_eq!(format!("{}", ErrorKind::Validation), "validation");
        assert_eq!(format!("{}", ErrorKind::Runtime), "runtime");
    }

    #[test]
    fn test_validation_errors_return_exit_code_2() {
        assert_eq!(CliError::Config("x".into()).exit_code(), 2);
        assert_eq!(CliError::InvalidEndpoint("x".into()).exit_code(), 2);
        assert_eq!(CliError::MissingArgument("x".into()).exit_code(), 2);
        assert_eq!(CliError::InvalidHostname("x".into()).exit_code(), 2);
        assert_eq!(CliError::UnknownDatastore("x".into()).exit_code(), 2);
    }

    #[test]
    fn test_runtime_errors_return_exit_code_1() {
        use std::io;
        assert_eq!(CliError::Logging("x".into()).exit_code(), 1);
        assert_eq!(CliError::Engine("x".into()).exit_code(), 1);
        assert_eq!(CliError::Migration("x".into()).exit_code(), 1);
        assert_eq!(CliError::NotFound("x".into()).exit_code(), 1);
        assert_eq!(
            CliError::ApiError {
                code: 500,
                message: "x".into()
            }
            .exit_code(),
            1
        );
        assert_eq!(
            CliError::Io(io::Error::new(io::ErrorKind::NotFound, "x")).exit_code(),
            1
        );
    }

    #[test]
    fn test_exit_codes_are_never_negative() {
        assert!(ErrorKind::Validation.exit_code() >= 0);
        assert!(ErrorKind::Runtime.exit_code() >= 0);
        assert!(CliError::Config("x".into()).exit_code() >= 0);
        assert!(CliError::Engine("x".into()).exit_code() >= 0);
    }

    #[test]
    fn test_cli_error_debug() {
        let err = CliError::Config("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Config"));
    }
}
