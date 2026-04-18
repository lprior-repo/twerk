// Adversarial test cases for trigger UPDATE implementation
// These tests probe edge cases, contract violations, and failure modes

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use time::OffsetDateTime;
use tower::ServiceExt;
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::trigger_api::{InMemoryTriggerDatastore, Trigger, TriggerAppState, TriggerId};
use twerk_web::api::{create_router, AppState, Config};

fn trigger(id: &str) -> Trigger {
    let now = OffsetDateTime::UNIX_EPOCH;
    Trigger {
        id: TriggerId::parse(id).expect("valid id"),
        name: "before".to_string(),
        enabled: false,
        event: "before.event".to_string(),
        condition: Some("x == 1".to_string()),
        action: "before_action".to_string(),
        metadata: std::collections::HashMap::from([("k".to_string(), "v".to_string())]),
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

fn build_state(trigger_ds: Arc<InMemoryTriggerDatastore>) -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState {
        trigger_state: TriggerAppState { trigger_ds },
        ..AppState::new(broker, ds, Config::default())
    }
}

async fn send_put(
    app: axum::Router,
    path: &str,
    content_type: &str,
    payload: Body,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(path)
                .header(header::CONTENT_TYPE, content_type)
                .body(payload)
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = serde_json::from_slice(&bytes).expect("json body");
    (status, json)
}

// ===== ADVERSARIAL TEST CASES =====

// Test 1: Unicode characters in ID - should be rejected
#[tokio::test]
async fn adversarial_id_with_unicode_characters() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid\u{1F600}id", // emoji in path
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should reject unicode in ID
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Unicode in ID should be rejected"
    );
}

// Test 2: SQL injection-like characters in fields
#[tokio::test]
async fn adversarial_sql_injection_in_name() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "'; DROP TABLE triggers; --",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should accept or reject consistently - if accepted, should not cause SQL issues
    // In-memory store should handle this safely
    assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST);
}

// Test 3: Extremely long field values (beyond 64 char limit)
#[tokio::test]
async fn adversarial_field_exceeds_max_length() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let long_name = "a".repeat(100); // Exceeds 64 char limit

    let body = json!({
        "id": "valid-id",
        "name": long_name,
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, body_json) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Field exceeding max length should be rejected"
    );
    assert!(body_json["message"]
        .as_str()
        .unwrap()
        .contains("max length"));
}

// Test 4: Whitespace-only fields that trim to empty
#[tokio::test]
async fn adversarial_whitespace_only_fields() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "   ",  // Only whitespace
        "enabled": true,
        "event": "   ",  // Only whitespace
        "action": "   ",  // Only whitespace
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Whitespace-only fields should be rejected"
    );
}

// Test 5: Null bytes in strings
#[tokio::test]
async fn adversarial_null_bytes_in_fields() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "hello\x00world",  // Null byte in string
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should handle null bytes gracefully - reject or accept consistently
    assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST);
}

// Test 6: Very large body (exceeds MAX_BODY_BYTES)
#[tokio::test]
async fn adversarial_body_exceeds_max_size() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let large_body = "x".repeat(20 * 1024); // 20KB, exceeds 16KB limit

    let body = json!({
        "id": "valid-id",
        "name": large_body,
        "enabled": true,
        "event": large_body.clone(),
        "action": large_body.clone(),
        "metadata": {},
        "version": 1
    });

    let (status, body_json) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Oversized body should be rejected"
    );
    assert!(body_json["message"].as_str().unwrap().contains("max size"));
}

// Test 7: Invalid metadata keys - empty string
#[tokio::test]
async fn adversarial_metadata_empty_key() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {"": "value"},  // Empty key
        "version": 1
    });

    let (status, body_json) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Empty metadata key should be rejected"
    );
    assert!(body_json["message"].as_str().unwrap().contains("metadata"));
}

// Test 8: Non-ASCII metadata keys
#[tokio::test]
async fn adversarial_metadata_non_ascii_key() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {"clé": "value"},  // Non-ASCII key
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Non-ASCII metadata key should be rejected"
    );
}

// Test 9: ID with special shell characters
#[tokio::test]
async fn adversarial_id_with_shell_special_chars() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    // Path with semicolon (shell command separator)
    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid;id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Semicolon in ID should be rejected"
    );
}

// Test 10: ID with path traversal attempt
#[tokio::test]
async fn adversarial_id_path_traversal() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    // Try path traversal - this may fail at URI parsing level
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/triggers/../valid-id")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .expect("request"),
        )
        .await
        .expect("response");

    let status = response.status();
    // Path traversal should either be rejected at routing or handler level
    assert!(
        status == StatusCode::BAD_REQUEST
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::INTERNAL_SERVER_ERROR,
        "Path traversal should be handled gracefully, got: {}",
        status
    );
}

// Test 11: Empty body
#[tokio::test]
async fn adversarial_empty_body() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from("".to_string()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Empty body should be rejected"
    );
}

// Test 12: Completely malformed JSON (not just truncated)
#[tokio::test]
async fn adversarial_completely_malformed_json() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from("this is not json at all!!!"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Malformed JSON should be rejected"
    );
}

// Test 13: ID with newlines - attempts to inject header
#[tokio::test]
async fn adversarial_id_with_newlines() {
    // NOTE: Newlines in URI are invalid according to HTTP spec.
    // The HTTP library (http crate) validates URIs at construction time
    // and will panic if invalid. This is correct behavior - malformed
    // requests should be rejected at the HTTP layer before reaching
    // the application. In production, a reverse proxy would typically
    // handle this before the request reaches the application.
    //
    // The implementation is safe against newline injection in URIs.

    // This test passes by documenting that the HTTP library handles
    // invalid URIs appropriately at construction time.
    let result: Result<Request<Body>, axum::http::Error> = Request::builder()
        .method("PUT")
        .uri("/api/v1/triggers/valid\nid")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::empty());

    assert!(
        result.is_err(),
        "URI with newline should be rejected by HTTP library"
    );
}

// Test 14: Version with negative number
#[tokio::test]
async fn adversarial_negative_version() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": -1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should handle negative version gracefully
    assert!(
        status == StatusCode::OK
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::CONFLICT
    );
}

// Test 15: Boolean instead of string for name field
#[tokio::test]
async fn adversarial_wrong_type_for_name() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": false,  // Wrong type
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should handle type mismatch gracefully
    assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST);
}

// Test 16: Condition field as number instead of string
#[tokio::test]
async fn adversarial_condition_as_number() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "condition": 42,  // Should be Option<String>
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should handle type mismatch
    assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST);
}

// Test 17: Enabled field as string instead of boolean
#[tokio::test]
async fn adversarial_enabled_as_string() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": "yes",  // Should be boolean
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, body_json) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should default to false when type is wrong
    assert_eq!(
        status,
        StatusCode::OK,
        "Wrong type for enabled should default to false"
    );
    assert_eq!(
        body_json["enabled"], false,
        "Enabled should be false when parsed from wrong type"
    );
}

// Test 18: ID in body with different case (case sensitivity)
#[tokio::test]
async fn adversarial_id_case_mismatch() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("Valid-Id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",  // Different case
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/Valid-Id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "ID case mismatch should be rejected"
    );
}

// Test 19: Very large metadata value
#[tokio::test]
async fn adversarial_large_metadata_value() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let large_value = "x".repeat(10000);

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {"key": large_value},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    // Should handle large metadata values (no explicit limit on value size)
    assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST);
}

// Test 20: Multiple errors returned (name and event both blank)
#[tokio::test]
async fn adversarial_multiple_field_errors() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "",
        "enabled": true,
        "event": "",
        "action": "",
        "metadata": {},
        "version": 1
    });

    let (status, body_json) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Multiple blank fields should be rejected"
    );
    // Only first error is returned - this is a potential issue for debugging
    let message = body_json["message"].as_str().unwrap();
    assert!(message.contains("name") || message.contains("event") || message.contains("action"));
}

// Test 21: ID at maximum length boundary (64 chars)
#[tokio::test]
async fn adversarial_id_max_boundary() {
    let max_id = "a".repeat(64);
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger(&max_id)).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": max_id,
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        &format!("/api/v1/triggers/{}", max_id),
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "Max length ID (64 chars) should be accepted"
    );
}

// Test 22: Content-Type with charset parameter
#[tokio::test]
async fn adversarial_content_type_with_charset() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json; charset=utf-8", // With charset
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "Content-Type with charset should be accepted"
    );
}

// Test 23: Very old timestamp (year 1970)
#[tokio::test]
async fn adversarial_very_old_timestamp() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    let old_time = OffsetDateTime::UNIX_EPOCH;
    let trigger = Trigger {
        id: TriggerId::parse("valid-id").unwrap(),
        name: "before".to_string(),
        enabled: false,
        event: "before.event".to_string(),
        condition: Some("x == 1".to_string()),
        action: "before_action".to_string(),
        metadata: std::collections::HashMap::new(),
        version: 1,
        created_at: old_time,
        updated_at: old_time,
    };
    trigger_ds.upsert(trigger).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "valid-id",
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    // This would work since we're using current time for the update
    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "Updating trigger with old timestamp should work"
    );
}

// Test 24: All allowed ID characters
#[tokio::test]
async fn adversarial_id_all_allowed_chars() {
    let allowed_id = "abc123-_DEF_GHI-jkl0123456789";
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger(allowed_id)).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": allowed_id,
        "name": "test",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        &format!("/api/v1/triggers/{}", allowed_id),
        "application/json",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "ID with all allowed characters should be accepted"
    );
}

// Test 25: XML content type (should be rejected)
#[tokio::test]
async fn adversarial_xml_content_type() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("valid-id")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/valid-id",
        "application/xml",
        Body::from("<root></root>"),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "XML content type should be rejected"
    );
}
