use twerk_core::domain::{GoDuration, Hostname, QueueName, WebhookUrl};
use twerk_core::id::TaskId;

// ---------------------------------------------------------------------------
// Hostname
// ---------------------------------------------------------------------------

#[kani::proof]
fn hostname_rejects_empty() {
    let result = Hostname::new("");
    assert!(result.is_err(), "Empty hostname should be rejected");
}

#[kani::proof]
fn hostname_rejects_colon() {
    let result = Hostname::new("host:8080");
    assert!(
        result.is_err(),
        "Hostname with colon should be rejected"
    );
}

// ---------------------------------------------------------------------------
// QueueName
// ---------------------------------------------------------------------------

#[kani::proof]
fn queue_name_rejects_empty() {
    let result = QueueName::new("");
    assert!(result.is_err(), "Empty queue name should be rejected");
}

#[kani::proof]
fn queue_name_rejects_uppercase() {
    let result = QueueName::new("MyQueue");
    assert!(
        result.is_err(),
        "Queue name with uppercase should be rejected"
    );
}

// ---------------------------------------------------------------------------
// GoDuration
// ---------------------------------------------------------------------------

#[kani::proof]
fn go_duration_rejects_empty() {
    let result = GoDuration::new("");
    assert!(result.is_err(), "Empty GoDuration should be rejected");
}

#[kani::proof]
fn go_duration_accepts_valid() {
    let result = GoDuration::new("30s");
    assert!(result.is_ok(), "Valid GoDuration '30s' should be accepted");
}

// ---------------------------------------------------------------------------
// WebhookUrl
// ---------------------------------------------------------------------------

#[kani::proof]
fn webhook_url_rejects_ftp() {
    let result = WebhookUrl::new("ftp://example.com");
    assert!(
        result.is_err(),
        "FTP scheme should be rejected for WebhookUrl"
    );
}

// ---------------------------------------------------------------------------
// TaskId (wraps validate_id)
// ---------------------------------------------------------------------------

#[kani::proof]
fn validate_id_rejects_empty() {
    let result = TaskId::new("");
    assert!(result.is_err(), "Empty ID should be rejected");
}

#[kani::proof]
fn validate_id_rejects_invalid_chars() {
    let result = TaskId::new("abc@def");
    assert!(
        result.is_err(),
        "ID with '@' should be rejected"
    );
}

#[kani::proof]
fn validate_id_accepts_valid() {
    let result = TaskId::new("my-id_123");
    assert!(
        result.is_ok(),
        "Valid ID with alphanumeric, dash, underscore should be accepted"
    );
}
