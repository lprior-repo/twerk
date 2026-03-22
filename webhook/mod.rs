//! Webhook execution with retry logic
//!
//! Provides functionality to call webhooks with automatic retry on transient failures.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use thiserror::Error;
use tracing::info;

/// Default maximum number of retry attempts.
const WEBHOOK_DEFAULT_MAX_ATTEMPTS: usize = 5;

/// Default timeout for webhook requests.
const WEBHOOK_DEFAULT_TIMEOUT_SECS: u64 = 5;

/// Event types for webhook notifications.
pub const EVENT_JOB_STATE_CHANGE: &str = "job.StateChange";
pub const EVENT_JOB_PROGRESS: &str = "job.Progress";
pub const EVENT_TASK_STATE_CHANGE: &str = "task.StateChange";
pub const EVENT_TASK_PROGRESS: &str = "task.Progress";
pub const EVENT_DEFAULT: &str = "";

/// Status codes that indicate a retryable error.
const RETRYABLE_STATUS_CODES: [u16; 5] = [
    429, // TooManyRequests
    500, // InternalServerError
    502, // BadGateway
    503, // ServiceUnavailable
    504, // GatewayTimeout
];

/// Checks if a status code indicates a retryable error.
#[must_use]
pub fn is_retryable(status_code: u16) -> bool {
    RETRYABLE_STATUS_CODES.contains(&status_code)
}

/// Errors that can occur during webhook execution.
#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("failed to serialize request body")]
    SerializationError,

    #[error("webhook request to {0} failed with non-retryable status {1}")]
    NonRetryableError(String, u16),

    #[error("webhook request to {0} failed after {1} attempts")]
    MaxAttemptsExceeded(String, usize),
}

/// A webhook configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Webhook {
    /// The URL to send the webhook to.
    pub url: String,
    /// Optional custom headers to include in the request.
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

/// Executes a single webhook request and returns the status code if one was received.
///
/// This is a pure calculation that doesn't perform I/O.
fn execute_request(
    url: &str,
    headers: &Option<std::collections::HashMap<String, String>>,
    body: &[u8],
) -> Result<u16, ()> {
    let request = ureq::post(url)
        .set("Content-Type", "application/json; charset=UTF-8")
        .timeout(Duration::from_secs(WEBHOOK_DEFAULT_TIMEOUT_SECS));

    // Apply custom headers using fold to avoid move error
    let request = if let Some(ref hdrs) = headers {
        hdrs.iter().fold(request, |req, (k, v)| req.set(k, v))
    } else {
        request
    };

    request
        .send_bytes(body)
        .map(|resp| resp.status())
        .map_err(|_| ())
}

/// Determines if a retry should be attempted based on status code.
#[must_use]
fn should_retry(status: Option<u16>, remaining_attempts: usize) -> bool {
    match status {
        Some(code) if (200..300).contains(&code) => false, // Success, no retry
        Some(code) if !is_retryable(code) => false,        // Non-retryable error
        _ if remaining_attempts <= 1 => false,             // No more attempts
        _ => true,
    }
}

/// Calculates the backoff duration for the given attempt number.
#[must_use]
fn backoff_duration(attempt: usize) -> Duration {
    Duration::from_secs(2 * attempt as u64)
}

/// Calls a webhook with the given body and retry logic.
///
/// # Arguments
/// * `wh` - The webhook configuration
/// * `body` - The request body to serialize and send
///
/// # Returns
/// `Ok(())` on success, or an error if the webhook call failed
pub fn call(wh: &Webhook, body: &impl serde::Serialize) -> Result<(), WebhookError> {
    let serialized = serde_json::to_string(body).map_err(|_| WebhookError::SerializationError)?;

    // Retry loop using fold to track state functionally
    let attempts = 0..WEBHOOK_DEFAULT_MAX_ATTEMPTS;
    let mut status: u16;

    for attempt in attempts {
        let current_attempt = attempt + 1;
        status = execute_request(&wh.url, &wh.headers, serialized.as_bytes()).map_or(0, |s| s);

        // Check if successful
        if (200..300).contains(&status) {
            return Ok(());
        }

        // Log the failure
        if status == 0 {
            info!(url = %wh.url, attempt = current_attempt, "webhook request failed with connection error");
        } else if !is_retryable(status) {
            return Err(WebhookError::NonRetryableError(wh.url.clone(), status));
        } else {
            info!(url = %wh.url, status = status, attempt = current_attempt, "webhook request failed with retryable status");
        }

        // Check if we should continue retrying
        let remaining = WEBHOOK_DEFAULT_MAX_ATTEMPTS - current_attempt;
        if !should_retry(Some(status), remaining) {
            break;
        }

        // Sleep with exponential backoff before retry
        std::thread::sleep(backoff_duration(current_attempt));
    }

    Err(WebhookError::MaxAttemptsExceeded(
        wh.url.clone(),
        WEBHOOK_DEFAULT_MAX_ATTEMPTS,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable(429));
        assert!(is_retryable(500));
        assert!(is_retryable(502));
        assert!(is_retryable(503));
        assert!(is_retryable(504));
        assert!(!is_retryable(200));
        assert!(!is_retryable(400));
        assert!(!is_retryable(404));
    }

    #[test]
    fn test_should_retry() {
        // Success - don't retry
        assert!(!should_retry(Some(200), 3));
        assert!(!should_retry(Some(201), 3));
        assert!(!should_retry(Some(204), 3));

        // Non-retryable errors - don't retry
        assert!(!should_retry(Some(400), 3));
        assert!(!should_retry(Some(401), 3));
        assert!(!should_retry(Some(404), 3));

        // Retryable errors - retry if attempts remain
        assert!(should_retry(Some(429), 3));
        assert!(should_retry(Some(500), 3));
        assert!(should_retry(Some(502), 3));

        // No more attempts - don't retry
        assert!(!should_retry(Some(500), 1));
        assert!(!should_retry(Some(429), 0));
    }

    #[test]
    fn test_backoff_duration() {
        assert_eq!(Duration::from_secs(2), backoff_duration(1));
        assert_eq!(Duration::from_secs(4), backoff_duration(2));
        assert_eq!(Duration::from_secs(6), backoff_duration(3));
    }
}
