//! Aggregated domain parse error type.

use thiserror::Error;

use super::cron_expression::CronExpressionError;
use super::go_duration::GoDurationError;
use super::priority::PriorityError;
use super::queue_name::QueueNameError;

/// Unified error type for domain parsing failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainParseError {
    #[error("invalid cron expression: {0}")]
    Cron(#[from] CronExpressionError),
    #[error("invalid duration: {0}")]
    Duration(#[from] GoDurationError),
    #[error("invalid queue name: {0}")]
    QueueName(#[from] QueueNameError),
    #[error("invalid priority: {0}")]
    Priority(#[from] PriorityError),
    #[error("invalid retry limit: {0}")]
    RetryLimit(#[from] ParseRetryError),
}

/// Error for retry limit range validation (1-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ParseRetryError {
    #[error("retry limit {0} is out of range (must be 1-10)")]
    OutOfRange(i64),
}
