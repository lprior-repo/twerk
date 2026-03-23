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

use std::time::Duration;

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
/// 429 TooManyRequests, 500 InternalServerError,
/// 502 BadGateway, 503 ServiceUnavailable, 504 GatewayTimeout.
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

/// A webhook configuration.
///
/// Mirrors Go's `tork.Webhook` struct.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Webhook {
    /// The URL to send the webhook to.
    pub url: String,
    /// Optional custom headers to include in the request.
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
}

/// Executes a single webhook POST request.
///
/// Returns `Ok(status)` on an HTTP response, or `Err(())` on a connection /
/// transport error (DNS failure, timeout, connection refused, etc.).
///
/// Go equivalent: `client.Do(req)` — the error branch maps to `Err(())`.
fn execute_request(
    url: &str,
    headers: &Option<std::collections::HashMap<String, String>>,
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
/// 1. Success (2xx) → return nil           → `false`
/// 2. Non-retryable → return error         → `false`
/// 3. Connection error / retryable → sleep → `true`
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
/// Parity with Go's `func Call(wh *tork.Webhook, body any) error`:
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
pub fn call(wh: &Webhook, body: &impl serde::Serialize) -> Result<(), WebhookError> {
    let serialized = serde_json::to_string(body).map_err(|_| WebhookError::SerializationError)?;

    for attempt in 0..WEBHOOK_DEFAULT_MAX_ATTEMPTS {
        let current_attempt = attempt + 1;
        let remaining = WEBHOOK_DEFAULT_MAX_ATTEMPTS - current_attempt;

        let result = execute_request(&wh.url, &wh.headers, serialized.as_bytes());

        // Go parity: check success (2xx), non-retryable, then log + retry
        match result {
            Ok(status) if (200..300).contains(&status) => return Ok(()),
            Ok(status) if !is_retryable(status) => {
                return Err(WebhookError::NonRetryableError(wh.url.clone(), status));
            }
            Ok(status) => {
                info!(
                    webhook_url = %wh.url,
                    status,
                    attempt = current_attempt,
                    "[Webhook] request to {} failed with {}",
                    wh.url, status
                );
            }
            Err(()) => {
                info!(
                    webhook_url = %wh.url,
                    attempt = current_attempt,
                    "[Webhook] request to {} failed with connection error",
                    wh.url
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
        wh.url.clone(),
        WEBHOOK_DEFAULT_MAX_ATTEMPTS,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_constants_match_go() {
        assert_eq!("job.StateChange", EVENT_JOB_STATE_CHANGE);
        assert_eq!("job.Progress", EVENT_JOB_PROGRESS);
        assert_eq!("task.StateChange", EVENT_TASK_STATE_CHANGE);
        assert_eq!("task.Progress", EVENT_TASK_PROGRESS);
        assert_eq!("", EVENT_DEFAULT);
    }

    #[test]
    fn test_is_retryable() {
        // Retryable codes (Go: retryableStatusCodes)
        assert!(is_retryable(429));
        assert!(is_retryable(500));
        assert!(is_retryable(502));
        assert!(is_retryable(503));
        assert!(is_retryable(504));
        // Non-retryable codes
        assert!(!is_retryable(200));
        assert!(!is_retryable(201));
        assert!(!is_retryable(400));
        assert!(!is_retryable(401));
        assert!(!is_retryable(404));
        assert!(!is_retryable(422));
    }

    #[test]
    fn test_should_retry_success_no_retry() {
        // 2xx success — never retry
        assert!(!should_retry(Ok(200), 5));
        assert!(!should_retry(Ok(201), 5));
        assert!(!should_retry(Ok(204), 5));
        assert!(!should_retry(Ok(299), 5));
    }

    #[test]
    fn test_should_retry_non_retryable_no_retry() {
        // Non-retryable status — return immediately, no retry
        assert!(!should_retry(Ok(400), 5));
        assert!(!should_retry(Ok(401), 5));
        assert!(!should_retry(Ok(404), 5));
        assert!(!should_retry(Ok(422), 5));
    }

    #[test]
    fn test_should_retry_retryable_with_attempts() {
        // Retryable codes with remaining attempts → retry
        assert!(should_retry(Ok(429), 3));
        assert!(should_retry(Ok(500), 3));
        assert!(should_retry(Ok(502), 3));
        assert!(should_retry(Ok(503), 3));
        assert!(should_retry(Ok(504), 3));
    }

    #[test]
    fn test_should_retry_no_remaining_attempts() {
        // Even retryable codes don't retry when attempts exhausted
        assert!(!should_retry(Ok(500), 0));
        assert!(!should_retry(Ok(429), 0));
    }

    #[test]
    fn test_should_retry_connection_error() {
        // Connection errors are retried when attempts remain (Go parity)
        assert!(should_retry(Err(()), 4));
        assert!(should_retry(Err(()), 1));
        // But not when no attempts remain
        assert!(!should_retry(Err(()), 0));
    }

    #[test]
    fn test_backoff_duration_matches_go() {
        // Go: time.Second * time.Duration(attempts*2)
        // attempts starts at 1 → 2s, 4s, 6s, 8s, 10s
        assert_eq!(Duration::from_secs(2), backoff_duration(1));
        assert_eq!(Duration::from_secs(4), backoff_duration(2));
        assert_eq!(Duration::from_secs(6), backoff_duration(3));
        assert_eq!(Duration::from_secs(8), backoff_duration(4));
        assert_eq!(Duration::from_secs(10), backoff_duration(5));
    }

    #[test]
    fn test_webhook_error_display_messages() {
        assert_eq!(
            "[Webhook] error serializing body",
            WebhookError::SerializationError.to_string()
        );
        assert_eq!(
            "[Webhook] request to http://example.com failed with non-retryable status 404",
            WebhookError::NonRetryableError("http://example.com".to_string(), 404).to_string()
        );
        assert_eq!(
            "[Webhook] failed to call webhook http://example.com. max attempts: 5)",
            WebhookError::MaxAttemptsExceeded("http://example.com".to_string(), 5).to_string()
        );
    }

    #[test]
    fn test_retryable_status_codes_count() {
        // Go has exactly 5 retryable codes
        assert_eq!(5, RETRYABLE_STATUS_CODES.len());
    }

    #[test]
    fn test_webhook_struct_serde_roundtrip() {
        let wh = Webhook {
            url: "https://example.com/hook".to_string(),
            headers: {
                let mut m = std::collections::HashMap::new();
                m.insert("Authorization".to_string(), "Bearer token".to_string());
                Some(m)
            },
        };
        let json = serde_json::to_string(&wh).expect("serialize");
        let deserialized: Webhook = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(wh.url, deserialized.url);
        assert_eq!(wh.headers, deserialized.headers);
    }
}
