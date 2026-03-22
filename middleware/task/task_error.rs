//! Task middleware errors.

use thiserror::Error;

/// Errors that can occur during task middleware operations.
#[derive(Debug, Error)]
pub enum TaskMiddlewareError {
    /// The task was not found.
    #[error("task not found: {0}")]
    TaskNotFound(String),

    /// The job was not found.
    #[error("job not found: {0}")]
    JobNotFound(String),

    /// A datastore error occurred.
    #[error("datastore error: {0}")]
    Datastore(String),

    /// An evaluation error occurred.
    #[error("evaluation error: {0}")]
    Evaluation(String),

    /// A webhook error occurred.
    #[error("webhook error: {0}")]
    Webhook(String),

    /// An HTTP error occurred.
    #[error("HTTP error: {0}")]
    Http(String),

    /// A cache error occurred.
    #[error("cache error: {0}")]
    Cache(String),

    /// A redaction error occurred.
    #[error("redaction error: {0}")]
    Redaction(String),

    /// Host environment error.
    #[error("host environment error: {0}")]
    HostEnv(String),

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// A generic middleware error.
    #[error("middleware error: {0}")]
    Middleware(String),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for TaskMiddlewareError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        TaskMiddlewareError::Datastore(err.to_string())
    }
}
