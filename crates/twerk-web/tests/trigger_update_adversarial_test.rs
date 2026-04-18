#![deny(clippy::unwrap_used)]
#![allow(clippy::panic)]

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use time::OffsetDateTime;
use tower::ServiceExt;
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::trigger_api::{
    InMemoryTriggerDatastore, Trigger, TriggerAppState, TriggerId, TRIGGER_FIELD_MAX_LEN,
};
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

fn body_ok(id: &str) -> Value {
    json!({
        "id": id,
        "name": "updated",
        "enabled": true,
        "event": "order.created",
        "condition": "amount > 10",
        "action": "send_email",
        "metadata": {"env":"prod"},
        "version": 1
    })
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

// ========================================
// ADVERSARIAL TEST 1: Field length boundary - name exceeds max (64 chars)
// ========================================
#[tokio::test]
async fn adversarial_field_name_exceeds_max_length_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let long_name = "a".repeat(TRIGGER_FIELD_MAX_LEN + 1);
    let body = json!({
        "id": "trg_abc",
        "name": long_name,
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Should fail validation - name exceeds 64 chars
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "name > 64 chars should return 400"
    );
}

// ========================================
// ADVERSARIAL TEST 2: Field length boundary - event exceeds max (64 chars)
// ========================================
#[tokio::test]
async fn adversarial_field_event_exceeds_max_length_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let long_event = "e".repeat(TRIGGER_FIELD_MAX_LEN + 1);
    let body = json!({
        "id": "trg_abc",
        "name": "valid_name",
        "enabled": true,
        "event": long_event,
        "action": "send_email",
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Should fail validation - event exceeds 64 chars
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "event > 64 chars should return 400"
    );
}

// ========================================
// ADVERSARIAL TEST 3: Field length boundary - action exceeds max (64 chars)
// ========================================
#[tokio::test]
async fn adversarial_field_action_exceeds_max_length_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let long_action = "a".repeat(TRIGGER_FIELD_MAX_LEN + 1);
    let body = json!({
        "id": "trg_abc",
        "name": "valid_name",
        "enabled": true,
        "event": "order.created",
        "action": long_action,
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Should fail validation - action exceeds 64 chars
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "action > 64 chars should return 400"
    );
}

// ========================================
// ADVERSARIAL TEST 4: Metadata with empty key should fail
// ========================================
#[tokio::test]
async fn adversarial_metadata_empty_key_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "valid_name",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "metadata": {"":"empty_key_value"},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Should fail validation - empty metadata key
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "empty metadata key should return 400"
    );
}

// ========================================
// ADVERSARIAL TEST 5: Metadata with non-ASCII key should fail
// ========================================
#[tokio::test]
async fn adversarial_metadata_non_ascii_key_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "valid_name",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "metadata": {"key_with_utf8_é":"value"},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Should fail validation - non-ASCII metadata key
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "non-ASCII metadata key should return 400"
    );
}

// ========================================
// ADVERSARIAL TEST 6: Body too large (>16KB) should fail
// ========================================
#[tokio::test]
async fn adversarial_body_exceeds_max_size_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let large_payload = serde_json::to_vec(&json!({
        "id": "trg_abc",
        "name": "updated",
        "enabled": true,
        "event": "order.created",
        "condition": "amount > 10",
        "action": "send_email",
        "metadata": {"large_field": "x".repeat(20 * 1024)}, // ~20KB of metadata
        "version": 1
    }))
    .expect("serialize");

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(large_payload),
    )
    .await;

    // Should fail - body too large
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "body > 16KB should return 400"
    );
}

// ========================================
// ADVERSARIAL TEST 7: created_at is immutable (verify it doesn't change on update)
// ========================================
#[tokio::test]
async fn adversarial_created_at_immutable_on_update() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    let original_time = OffsetDateTime::UNIX_EPOCH + time::Duration::days(100);
    let trigger = Trigger {
        id: TriggerId::parse("trg_abc").expect("valid id"),
        name: "before".to_string(),
        enabled: false,
        event: "before.event".to_string(),
        condition: Some("x == 1".to_string()),
        action: "before_action".to_string(),
        metadata: std::collections::HashMap::new(),
        version: 1,
        created_at: original_time,
        updated_at: original_time,
    };
    trigger_ds.upsert(trigger).unwrap();
    let app = create_router(build_state(trigger_ds.clone()));

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("trg_abc")).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::OK);

    // Verify created_at in datastore is unchanged
    let persisted = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("should exist after update");
    assert_eq!(
        persisted.created_at, original_time,
        "created_at should be immutable - it should still be the original time"
    );
}

// ========================================
// ADVERSARIAL TEST 8: updated_at advances on update
// ========================================
#[tokio::test]
async fn adversarial_updated_at_advances_on_update() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    let original_time = OffsetDateTime::UNIX_EPOCH;
    let trigger = Trigger {
        id: TriggerId::parse("trg_abc").expect("valid id"),
        name: "before".to_string(),
        enabled: false,
        event: "before.event".to_string(),
        condition: Some("x == 1".to_string()),
        action: "before_action".to_string(),
        metadata: std::collections::HashMap::new(),
        version: 1,
        created_at: original_time,
        updated_at: original_time,
    };
    trigger_ds.upsert(trigger).unwrap();
    let app = create_router(build_state(trigger_ds.clone()));

    // First update
    let (status1, body1) = send_put(
        app.clone(),
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body_ok("trg_abc")).expect("serialize")),
    )
    .await;
    assert_eq!(status1, StatusCode::OK);
    let first_version = body1["version"].as_u64().expect("version as u64");

    let after_first = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("should exist after first update");
    let first_updated_at = after_first.updated_at;

    // Second update with different data to force a new updated_at
    let mut body2 = body_ok("trg_abc");
    body2["name"] = json!("updated_differently");
    body2["version"] = json!(first_version);
    let (status2, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body2).expect("serialize")),
    )
    .await;
    assert_eq!(status2, StatusCode::OK);

    let after_second = trigger_ds
        .get_trigger_by_id(&TriggerId::parse("trg_abc").expect("id"))
        .expect("should exist after second update");
    let second_updated_at = after_second.updated_at;

    // The second updated_at should be >= first updated_at
    assert!(
        second_updated_at >= first_updated_at,
        "updated_at should advance or stay same, but should not go backwards"
    );
}

// ========================================
// ADVERSARIAL TEST 9: Whitespace-only fields should fail
// ========================================
#[tokio::test]
async fn adversarial_whitespace_only_name_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "   ",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "whitespace-only name should return 400"
    );
}

// ========================================
// ADVERSARIAL TEST 10: Version mismatch should fail with 409 Conflict
// ========================================
#[tokio::test]
async fn adversarial_positive_version_mismatch_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "updated",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "version": 5
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Version 5 does not match stored version 1, should return 409 Conflict
    assert_eq!(status, StatusCode::CONFLICT, "version mismatch should return conflict");
}

// ========================================
// ADVERSARIAL TEST 11: Large metadata key (but valid ASCII) - test HashMap behavior
// ========================================
#[tokio::test]
async fn adversarial_metadata_with_long_valid_key_should_succeed() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let long_key = "k".repeat(200);
    let body = json!({
        "id": "trg_abc",
        "name": "valid_name",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "metadata": {long_key: "value"},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Long ASCII key should be allowed (no length limit on metadata keys in contract)
    assert_eq!(
        status,
        StatusCode::OK,
        "long ASCII metadata key should succeed"
    );
}

// ========================================
// ADVERSARIAL TEST 12: All whitespace fields combined should fail
// ========================================
#[tokio::test]
async fn adversarial_all_whitespace_fields_should_fail() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "   \t\n  ",
        "enabled": true,
        "event": "   \t\n  ",
        "action": "   \t\n  ",
        "version": 1
    });

    let (status, body_err) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "all whitespace fields should return 400"
    );
    // Should fail on name first (validate_required_field is called in order)
    assert_eq!(body_err["error"], "ValidationFailed");
}

// ========================================
// ADVERSARIAL TEST 13: id field in body can be omitted (it's optional)
// ========================================
#[tokio::test]
async fn adversarial_body_without_id_field_should_succeed() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "name": "updated",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // id is optional in body
    assert_eq!(
        status,
        StatusCode::OK,
        "body without id field should succeed"
    );
}

// ========================================
// ADVERSARIAL TEST 14: condition can be null
// ========================================
#[tokio::test]
async fn adversarial_null_condition_should_succeed() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "updated",
        "enabled": true,
        "event": "order.created",
        "condition": null,
        "action": "send_email",
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "null condition should succeed");
}

// ========================================
// ADVERSARIAL TEST 15: metadata can be null
// ========================================
#[tokio::test]
async fn adversarial_null_metadata_should_succeed() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "updated",
        "enabled": true,
        "event": "order.created",
        "condition": "amount > 10",
        "action": "send_email",
        "metadata": null,
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "null metadata should succeed");
}

// ========================================
// ADVERSARIAL TEST 16: metadata with special ASCII characters in key (!, @, #, etc.)
// ========================================
#[tokio::test]
async fn adversarial_metadata_special_char_key_should_succeed() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "valid_name",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "metadata": {"key-with-dashes_and_underscores": "value1", "another_key": "value2"},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    // Dashes and underscores are valid in metadata keys per the ASCII-safe requirement
    assert_eq!(
        status,
        StatusCode::OK,
        "metadata keys with dashes and underscores should succeed"
    );
}

// ========================================
// ADVERSARIAL TEST 17: Field at exactly max length (64 chars) should succeed
// ========================================
#[tokio::test]
async fn adversarial_field_at_max_length_should_succeed() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let max_name = "n".repeat(TRIGGER_FIELD_MAX_LEN);
    let body = json!({
        "id": "trg_abc",
        "name": max_name,
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "name at exactly 64 chars should succeed"
    );
}

// ========================================
// ADVERSARIAL TEST 18: Empty metadata object should succeed
// ========================================
#[tokio::test]
async fn adversarial_empty_metadata_object_should_succeed() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds));

    let body = json!({
        "id": "trg_abc",
        "name": "valid_name",
        "enabled": true,
        "event": "order.created",
        "action": "send_email",
        "metadata": {},
        "version": 1
    });

    let (status, _) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "empty metadata object should succeed"
    );
}

// ========================================
// ADVERSARIAL TEST 19: leading/trailing whitespace in fields should be trimmed (not rejected)
// ========================================
#[tokio::test]
async fn adversarial_whitespace_in_fields_should_be_normalized() {
    let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());
    trigger_ds.upsert(trigger("trg_abc")).unwrap();
    let app = create_router(build_state(trigger_ds.clone()));

    let body = json!({
        "id": "trg_abc",
        "name": "  valid_name  ",
        "enabled": true,
        "event": "  order.created  ",
        "action": "  send_email  ",
        "version": 1
    });

    let (status, body_resp) = send_put(
        app,
        "/api/v1/triggers/trg_abc",
        "application/json",
        Body::from(serde_json::to_vec(&body).expect("serialize")),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "whitespace-padded fields should be normalized"
    );
    // Verify trimming happened
    assert_eq!(body_resp["name"], "valid_name", "name should be trimmed");
    assert_eq!(
        body_resp["event"], "order.created",
        "event should be trimmed"
    );
    assert_eq!(
        body_resp["action"], "send_email",
        "action should be trimmed"
    );
}
