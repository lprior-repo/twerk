//! Primitive parsers and legacy compatibility validators.
//!
//! Each `parse_*` function returns a validated newtype from [`crate::domain`].
//! The `validate_*` functions are backwards-compatible wrappers that discard
//! the typed return value.

use crate::domain::{
    CronExpression, DomainParseError, GoDuration, ParseRetryError, Priority, QueueName,
    QueueNameError,
};
use crate::types::RetryLimit;

// ===================================================================
// Parse-Don't-Validate: new parser functions returning validated types
// ===================================================================

/// Parse a cron expression into a validated [`CronExpression`].
///
/// # Errors
/// Returns [`DomainParseError::Cron`] on invalid syntax.
pub fn parse_cron(cron: &str) -> Result<CronExpression, DomainParseError> {
    CronExpression::new(cron).map_err(DomainParseError::Cron)
}

/// Parse a Go-style duration into a validated [`GoDuration`].
///
/// # Errors
/// Returns [`DomainParseError::Duration`] on invalid syntax.
pub fn parse_duration(duration: &str) -> Result<GoDuration, DomainParseError> {
    GoDuration::new(duration).map_err(DomainParseError::Duration)
}

/// Parse a queue name into a validated [`QueueName`].
///
/// # Errors
/// Returns [`DomainParseError::QueueName`] if the name is reserved or malformed.
pub fn parse_queue_name(name: &str) -> Result<QueueName, DomainParseError> {
    // Check for reserved names
    if name == "x-jobs" || name.starts_with("x-exclusive.") {
        return Err(DomainParseError::QueueName(QueueNameError::Reserved(
            name.to_string(),
        )));
    }
    QueueName::new(name).map_err(DomainParseError::QueueName)
}

/// Parse a retry limit into a validated [`RetryLimit`].
///
/// Validates that the value is in 1..=10 before constructing a [`RetryLimit`].
///
/// # Errors
/// Returns [`DomainParseError::RetryLimit`] if not in 1..=10.
pub fn parse_retry(limit: i64) -> Result<RetryLimit, DomainParseError> {
    if !(1..=10).contains(&limit) {
        return Err(DomainParseError::RetryLimit(ParseRetryError::OutOfRange(
            limit,
        )));
    }
    match u32::try_from(limit) {
        Ok(v) => RetryLimit::new(v)
            .map_err(|_| DomainParseError::RetryLimit(ParseRetryError::OutOfRange(limit))),
        Err(_) => Err(DomainParseError::RetryLimit(ParseRetryError::OutOfRange(
            limit,
        ))),
    }
}

/// Parse a priority value into a validated [`Priority`].
///
/// # Errors
/// Returns [`DomainParseError::Priority`] if not in 0..=9.
pub fn parse_priority(priority: i64) -> Result<Priority, DomainParseError> {
    Priority::new(priority).map_err(DomainParseError::Priority)
}

// ===================================================================
// Backwards-compatible public API (validate_* returning Result<(), _>)
// ===================================================================

/// Validates a cron expression.
///
/// # Errors
/// Returns an error if the cron expression is invalid.
pub fn validate_cron(cron: &str) -> Result<(), String> {
    parse_cron(cron).map(|_| ()).map_err(|e| e.to_string())
}

/// Validates a duration string (e.g., "1h30m", "30s", "2d").
///
/// # Errors
/// Returns an error if the duration string is invalid.
pub fn validate_duration(duration: &str) -> Result<(), String> {
    parse_duration(duration)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Validates a queue name.
///
/// # Errors
/// Returns an error if the queue name starts with "x-exclusive." or is "x-jobs".
pub fn validate_queue_name(queue: &str) -> Result<(), String> {
    parse_queue_name(queue)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Validates a retry limit.
///
/// # Errors
/// Returns an error if the retry limit is not between 1 and 10.
pub fn validate_retry(limit: i64) -> Result<(), String> {
    parse_retry(limit).map(|_| ()).map_err(|e| e.to_string())
}

/// Validates a priority value.
///
/// # Errors
/// Returns an error if the priority is not between 0 and 9.
pub fn validate_priority(priority: i64) -> Result<(), String> {
    parse_priority(priority)
        .map(|_| ())
        .map_err(|e| e.to_string())
}
