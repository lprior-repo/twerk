use std::time::Duration;
use twerk_core::webhook::{backoff_duration, is_retryable, should_retry};

// ---------------------------------------------------------------------------
// is_retryable covers exactly five codes
// ---------------------------------------------------------------------------

#[kani::proof]
fn is_retryable_covers_exactly_five_codes() {
    // These are retryable
    assert!(is_retryable(429), "429 TooManyRequests is retryable");
    assert!(is_retryable(500), "500 InternalServerError is retryable");
    assert!(is_retryable(502), "502 BadGateway is retryable");
    assert!(is_retryable(503), "503 ServiceUnavailable is retryable");
    assert!(is_retryable(504), "504 GatewayTimeout is retryable");

    // These are NOT retryable
    assert!(!is_retryable(200), "200 OK is not retryable");
    assert!(!is_retryable(201), "201 Created is not retryable");
    assert!(!is_retryable(404), "404 NotFound is not retryable");
    assert!(!is_retryable(501), "501 NotImplemented is not retryable");
}

// ---------------------------------------------------------------------------
// should_retry rejects 2xx
// ---------------------------------------------------------------------------

#[kani::proof]
fn should_retry_rejects_2xx() {
    assert!(
        !should_retry(Ok(200), 5),
        "200 with remaining > 0 should not retry"
    );
    assert!(
        !should_retry(Ok(201), 5),
        "201 with remaining > 0 should not retry"
    );
    assert!(
        !should_retry(Ok(299), 5),
        "299 with remaining > 0 should not retry"
    );
    assert!(
        !should_retry(Ok(200), 1),
        "200 with remaining == 1 should not retry"
    );
}

// ---------------------------------------------------------------------------
// should_retry rejects zero remaining
// ---------------------------------------------------------------------------

#[kani::proof]
fn should_retry_rejects_zero_remaining() {
    assert!(
        !should_retry(Ok(429), 0),
        "Retryable code with remaining == 0 should not retry"
    );
    assert!(
        !should_retry(Ok(500), 0),
        "Retryable code with remaining == 0 should not retry"
    );
    assert!(
        !should_retry(Ok(503), 0),
        "Retryable code with remaining == 0 should not retry"
    );
}

// ---------------------------------------------------------------------------
// should_retry accepts retryable with remaining
// ---------------------------------------------------------------------------

#[kani::proof]
fn should_retry_accepts_retryable_with_remaining() {
    assert!(
        should_retry(Ok(429), 1),
        "429 with remaining == 1 should retry"
    );
    assert!(
        should_retry(Ok(500), 5),
        "500 with remaining == 5 should retry"
    );
    assert!(
        should_retry(Ok(502), 3),
        "502 with remaining == 3 should retry"
    );
    assert!(
        should_retry(Ok(503), 2),
        "503 with remaining == 2 should retry"
    );
    assert!(
        should_retry(Ok(504), 10),
        "504 with remaining == 10 should retry"
    );
}

// ---------------------------------------------------------------------------
// should_retry treats connection errors as retryable when remaining > 0
// ---------------------------------------------------------------------------

#[kani::proof]
fn should_retry_connection_error_with_remaining() {
    assert!(
        should_retry(Err(()), 1),
        "Connection error with remaining > 0 should retry"
    );
}

#[kani::proof]
fn should_retry_connection_error_zero_remaining() {
    assert!(
        !should_retry(Err(()), 0),
        "Connection error with remaining == 0 should not retry"
    );
}

// ---------------------------------------------------------------------------
// should_retry rejects non-retryable non-2xx
// ---------------------------------------------------------------------------

#[kani::proof]
fn should_retry_rejects_non_retryable() {
    assert!(
        !should_retry(Ok(400), 5),
        "400 with remaining > 0 is not retryable"
    );
    assert!(
        !should_retry(Ok(403), 5),
        "403 with remaining > 0 is not retryable"
    );
    assert!(
        !should_retry(Ok(404), 5),
        "404 with remaining > 0 is not retryable"
    );
}

// ---------------------------------------------------------------------------
// backoff_duration increases with attempt
// ---------------------------------------------------------------------------

#[kani::proof]
fn backoff_duration_increases_with_attempt() {
    let d1 = backoff_duration(1);
    let d2 = backoff_duration(2);
    let d3 = backoff_duration(3);
    let d4 = backoff_duration(4);
    let d5 = backoff_duration(5);

    assert!(d1 < d2, "backoff(1) < backoff(2)");
    assert!(d2 < d3, "backoff(2) < backoff(3)");
    assert!(d3 < d4, "backoff(3) < backoff(4)");
    assert!(d4 < d5, "backoff(4) < backoff(5)");
}

// ---------------------------------------------------------------------------
// backoff_duration exact values
// ---------------------------------------------------------------------------

#[kani::proof]
fn backoff_duration_values() {
    assert_eq!(
        backoff_duration(1),
        Duration::from_secs(2),
        "backoff(1) = 2s"
    );
    assert_eq!(
        backoff_duration(2),
        Duration::from_secs(4),
        "backoff(2) = 4s"
    );
    assert_eq!(
        backoff_duration(3),
        Duration::from_secs(6),
        "backoff(3) = 6s"
    );
    assert_eq!(
        backoff_duration(4),
        Duration::from_secs(8),
        "backoff(4) = 8s"
    );
    assert_eq!(
        backoff_duration(5),
        Duration::from_secs(10),
        "backoff(5) = 10s"
    );
}
