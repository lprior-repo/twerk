//! Webhook execution with retry logic.
//!
//! Provides functionality to call webhooks with automatic retry on transient
//! failures. Parity with Go's `internal/webhook/webhook.go`: identical event
//! constants, retryable status codes, retry loop semantics (including
//! connection-error retries), and backoff timing (`2 * attempt` seconds).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![allow(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

/// Default maximum number of retry attempts.
/// Go: `webhookDefaultMaxAttempts = 5`
const WEBHOOK_DEFAULT_MAX_ATTEMPTS: usize = 5;

/// Default timeout for webhook requests.
/// Go: `webhookDefaultTimeout = time.Second * 5`
const WEBHOOK_DEFAULT_TIMEOUT_SECS: u64 = 5;

/// Event types for webhook notifications.
///
/// These must match Go's constants exactly:
/// - `EventJobStateChange  = "job.StateChange"`
/// - `EventJobProgress     = "job.Progress"`
/// - `EventTaskStateChange = "task.StateChange"`
/// - `EventTaskProgress    = "task.Progress"`
/// - `EventDefault         = ""`
pub const EVENT_JOB_STATE_CHANGE: &str = "job.StateChange";
pub const EVENT_JOB_PROGRESS: &str = "job.Progress";
pub const EVENT_TASK_STATE_CHANGE: &str = "task.StateChange";
pub const EVENT_TASK_PROGRESS: &str = "task.Progress";
pub const EVENT_DEFAULT: &str = "";

/// Status codes that indicate a retryable error.
///
/// Matches Go's `retryableStatusCodes` exactly:
/// `429` Too Many Requests, `500` Internal Server Error,
/// `502` Bad Gateway, `503` Service Unavailable, `504` Gateway Timeout.
const RETRYABLE_STATUS_CODES: [u16; 5] = [
    429, // TooManyRequests
    500, // InternalServerError
    502, // BadGateway
    503, // ServiceUnavailable
    504, // GatewayTimeout
];

/// Checks if a status code indicates a retryable error.
///
/// Go: `func isRetryable(statusCode int) bool`
#[must_use]
pub fn is_retryable(status_code: u16) -> bool {
    RETRYABLE_STATUS_CODES.contains(&status_code)
}

/// Errors that can occur during webhook execution.
#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("[Webhook] error serializing body")]
    SerializationError,

    #[error("[Webhook] request to {0} failed with non-retryable status {1}")]
    NonRetryableError(String, u16),

    #[error("[Webhook] failed to call webhook {0}. max attempts: {1})")]
    MaxAttemptsExceeded(String, usize),
}

/// Webhook defines webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,
}

/// Executes a single webhook POST request.
///
/// Returns `Ok(status)` on an HTTP response, or `Err(())` on a connection /
/// transport error (DNS failure, timeout, connection refused, etc.).
///
/// Go equivalent: `client.Do(req)` — the error branch maps to `Err(())`.
fn execute_request(
    url: &str,
    headers: Option<&HashMap<String, String>>,
    body: &[u8],
) -> Result<u16, ()> {
    let request = ureq::post(url)
        .set("Content-Type", "application/json; charset=UTF-8")
        .timeout(Duration::from_secs(WEBHOOK_DEFAULT_TIMEOUT_SECS));

    // Apply custom headers. `ureq::Request` is not `Copy`, so we branch.
    let request = if let Some(hdrs) = headers {
        hdrs.iter().fold(request, |req, (k, v)| req.set(k, v))
    } else {
        request
    };

    match request.send_bytes(body) {
        Ok(resp) => Ok(resp.status()),
        Err(ureq::Error::Status(code, _resp)) => Ok(code),
        Err(_) => Err(()),
    }
}

/// Determines if a retry should be attempted based on the request result.
///
/// Connection errors (`Err`) are treated as retryable, matching Go's behavior
/// where `client.Do` errors trigger a retry with the same backoff.
///
/// Go equivalent: the three branches inside the retry loop:
/// 1. Success (2xx) -> return nil           -> `false`
/// 2. Non-retryable -> return error         -> `false`
/// 3. Connection error / retryable -> sleep -> `true`
#[must_use]
fn should_retry(result: Result<u16, ()>, remaining_attempts: usize) -> bool {
    match result {
        Ok(code) if (200..300).contains(&code) => false,
        Ok(code) if !is_retryable(code) => false,
        _ if remaining_attempts == 0 => false,
        _ => true,
    }
}

/// Calculates the backoff duration for the given attempt number.
///
/// Go: `time.Sleep(time.Second * time.Duration(attempts*2))`
/// where `attempts` starts at 1, yielding 2s, 4s, 6s, 8s, 10s.
#[must_use]
fn backoff_duration(attempt: usize) -> Duration {
    Duration::from_secs(2 * attempt as u64)
}

/// Calls a webhook with the given body and retry logic.
///
/// Parity with Go's `func Call(wh *twerk.Webhook, body any) error`:
/// - Serializes body as JSON
/// - POSTs with `Content-Type: application/json; charset=UTF-8`
/// - Applies custom headers from `wh.Headers`
/// - Retries up to `WEBHOOK_DEFAULT_MAX_ATTEMPTS` on connection errors and
///   retryable status codes (429, 500, 502, 503, 504)
/// - Returns immediately on non-retryable status codes
/// - Uses linear backoff: `2 * attempt_number` seconds
///
/// # Arguments
/// * `wh` - The webhook configuration
/// * `body` - The request body to serialize and send
///
/// # Returns
/// `Ok(())` on success, or an error if the webhook call failed
///
/// # Errors
/// Returns `WebhookError::SerializationError` if the body cannot be serialized.
/// Returns `WebhookError::NonRetryableError` on non-retryable HTTP status codes.
/// Returns `WebhookError::MaxAttemptsExceeded` if all retry attempts are exhausted.
pub fn call(wh: &Webhook, body: &impl serde::Serialize) -> Result<(), WebhookError> {
    let url = wh
        .url
        .as_ref()
        .ok_or_else(|| WebhookError::NonRetryableError("missing url".to_string(), 0))?;
    let serialized = serde_json::to_string(body).map_err(|_| WebhookError::SerializationError)?;

    for attempt in 0..WEBHOOK_DEFAULT_MAX_ATTEMPTS {
        let current_attempt = attempt + 1;
        let remaining = WEBHOOK_DEFAULT_MAX_ATTEMPTS - current_attempt;

        let result = execute_request(url, wh.headers.as_ref(), serialized.as_bytes());

        // Go parity: check success (2xx), non-retryable, then log + retry
        match result {
            Ok(status) if (200..300).contains(&status) => return Ok(()),
            Ok(status) if !is_retryable(status) => {
                return Err(WebhookError::NonRetryableError(url.clone(), status));
            }
            Ok(status) => {
                info!(
                    webhook_url = %url,
                    status,
                    attempt = current_attempt,
                    "[Webhook] request to {} failed with {}",
                    url, status
                );
            }
            Err(()) => {
                info!(
                    webhook_url = %url,
                    attempt = current_attempt,
                    "[Webhook] request to {} failed with connection error",
                    url
                );
            }
        }

        // `Result<u16, ()>` is `Copy` (both `u16` and `()` are `Copy`),
        // so `result` is still available after the match above.
        if !should_retry(result, remaining) {
            break;
        }

        // Go: `time.Sleep(time.Second * time.Duration(attempts*2))`
        std::thread::sleep(backoff_duration(current_attempt));
    }

    Err(WebhookError::MaxAttemptsExceeded(
        url.clone(),
        WEBHOOK_DEFAULT_MAX_ATTEMPTS,
    ))
}
