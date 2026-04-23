//! Error types for trigger data construction.

use thiserror::Error;

pub use crate::domain::{CronExpressionError, GoDurationError};
pub use crate::id::IdError;

// =============================================================================
// TriggerDataError
// =============================================================================

/// Errors that can occur during trigger data construction or validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TriggerDataError {
    #[error("invalid trigger ID: {0}")]
    InvalidTriggerId(#[from] IdError),

    #[error("invalid cron expression: {0}")]
    InvalidCronExpression(#[from] CronExpressionError),

    #[error("invalid interval: {0}")]
    InvalidInterval(#[from] GoDurationError),

    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("invalid HTTP method: {0}")]
    InvalidHttpMethod(String),

    #[error("empty required field: {0}")]
    EmptyRequiredField(String),

    #[error("invalid JMESPath expression: {0}")]
    InvalidJmespath(String),

    #[error("header limit exceeded: {0}")]
    HeaderLimitExceeded(String),
}
