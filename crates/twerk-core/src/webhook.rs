//! Webhook execution with retry logic (pure calculations).
//!
//! Provides pure calculation functions for webhook retry logic. This module
//! contains no I/O - the actual async HTTP calls are in twerk-app.
//!
//! For backward compatibility, a synchronous `call` function is provided that
//! wraps the async implementation using `tokio::runtime::Handle::current()`.
//!
//! Parity with Go's `internal/webhook/webhook.go`: identical event
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
use utoipa::ToSchema;

/// Default maximum number of retry attempts.
/// Go: `webhookDefaultMaxAttempts = 5`
pub const WEBHOOK_DEFAULT_MAX_ATTEMPTS: usize = 5;

/// Default timeout for webhook requests in seconds.
/// Go: `webhookDefaultTimeout = time.Second * 5`
pub const WEBHOOK_DEFAULT_TIMEOUT_SECS: u64 = 5;

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
    504, // GatewayTimeout,
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

    #[error("[Webhook] failed to build HTTP client")]
    ClientBuildError,

    #[error("[Webhook] request to {0} failed with non-retryable status {1}")]
    NonRetryableError(String, u16),

    #[error("[Webhook] failed to call webhook {0}. max attempts: {1})")]
    MaxAttemptsExceeded(String, usize),
}

/// Webhook defines webhook notification configuration.
<<<<<<< HEAD
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, utoipa::ToSchema)]
=======
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, ToSchema)]
>>>>>>> origin/tw-polecat/tau
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
pub fn should_retry(result: Result<u16, ()>, remaining_attempts: usize) -> bool {
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
pub fn backoff_duration(attempt: usize) -> Duration {
    Duration::from_secs(2 * attempt as u64)
}

/// Calls a webhook with the given body and retry logic (async version).
///
/// Parity with Go's `func Call(wh *twerk.Webhook, body any) error`:
/// - Serializes body as JSON
/// - POSTs with `Content-Type: application/json; charset=UTF-8`
/// - Applies custom headers from `wh.Headers`
/// - Retries up to `WEBHOOK_DEFAULT_MAX_ATTEMPTS` on connection errors and
///   retryable status codes (429, 500, 502, 503, 504)
/// - Returns immediately on non-retryable status codes
/// - Uses linear backoff with `tokio::time::sleep` (non-blocking)
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
pub async fn call_async(wh: &Webhook, body: &impl serde::Serialize) -> Result<(), WebhookError> {
    let url = wh
        .url
        .as_ref()
        .ok_or_else(|| WebhookError::NonRetryableError("missing url".to_string(), 0))?;
    let serialized = serde_json::to_string(body).map_err(|_| WebhookError::SerializationError)?;

    for attempt in 0..WEBHOOK_DEFAULT_MAX_ATTEMPTS {
        let current_attempt = attempt + 1;
        let remaining = WEBHOOK_DEFAULT_MAX_ATTEMPTS - current_attempt;

        // Execute HTTP request using ureq (sync, but called via spawn_blocking)
        let http_response = tokio::task::spawn_blocking({
            let url = url.clone();
            let serialized = serialized.clone();
            let headers = wh.headers.clone();
            move || {
                let request = ureq::post(&url)
                    .set("Content-Type", "application/json; charset=UTF-8")
                    .timeout(Duration::from_secs(WEBHOOK_DEFAULT_TIMEOUT_SECS));

                let request = if let Some(ref hdrs) = headers {
                    hdrs.iter().fold(request, |req, (k, v)| req.set(k, v))
                } else {
                    request
                };

                match request.send_bytes(serialized.as_bytes()) {
                    Ok(resp) => Ok(resp.status()),
                    Err(ureq::Error::Status(code, _resp)) => Ok(code),
                    Err(_) => Err(()),
                }
            }
        })
        .await
        .map_err(|_| WebhookError::MaxAttemptsExceeded(url.clone(), WEBHOOK_DEFAULT_MAX_ATTEMPTS))?
        .map_err(|_| {
            tracing::info!(
                webhook_url = %url,
                attempt = current_attempt,
                "[Webhook] request to {} failed with connection error",
                url
            );
            WebhookError::MaxAttemptsExceeded(url.clone(), WEBHOOK_DEFAULT_MAX_ATTEMPTS)
        })?;

        // Check result - success (2xx), non-retryable, or retryable
        if (200..300).contains(&http_response) {
            return Ok(());
        }

        if !is_retryable(http_response) {
            return Err(WebhookError::NonRetryableError(url.clone(), http_response));
        }

        tracing::info!(
            webhook_url = %url,
            status = http_response,
            attempt = current_attempt,
            "[Webhook] request to {} failed with {}",
            url, http_response
        );

        if !should_retry(Ok(http_response), remaining) {
            break;
        }

        // Non-blocking async sleep - does NOT block the thread
        tokio::time::sleep(backoff_duration(current_attempt)).await;
    }

    Err(WebhookError::MaxAttemptsExceeded(
        url.clone(),
        WEBHOOK_DEFAULT_MAX_ATTEMPTS,
    ))
}

/// Calls a webhook with the given body and retry logic.
///
/// This is a synchronous wrapper around `call_async` that uses
/// `tokio::runtime::Handle::current().block_on()` to execute the async
/// retry loop. This allows the function to be used in synchronous contexts
/// (like tests) while still using non-blocking `tokio::time::sleep`.
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
    // Try to use the current Tokio runtime if available (async context)
    // Otherwise spawn a new runtime (sync context like plain #[test])
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.block_on(call_async(wh, body))
    } else {
        // No runtime available - spawn one for sync context
        let runtime = tokio::runtime::Runtime::new().map_err(|_| {
            WebhookError::MaxAttemptsExceeded(
                wh.url.clone().unwrap_or_default(),
                WEBHOOK_DEFAULT_MAX_ATTEMPTS,
            )
        })?;
        runtime.block_on(call_async(wh, body))
    }
}
