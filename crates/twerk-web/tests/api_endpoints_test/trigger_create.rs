use axum::http::StatusCode;
use serde_json::json;

use super::shared::{setup_state, trigger_input};

async fn create_trigger(payload: &serde_json::Value) -> crate::support::TestResponse {
    crate::support::TestHarness::new()
        .await
        .post_json("/triggers", payload)
        .await
}

#[tokio::test]
async fn create_trigger_returns_201_with_valid_request() {
    let response = create_trigger(&trigger_input("test-trigger")).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.json()["name"], "test-trigger");
    assert_eq!(response.json()["event"], "test.event");
    assert_eq!(response.json()["action"], "test_action");
    assert!(response.json()["enabled"].as_bool().unwrap());
    assert!(response.json()["id"].is_string());
}

#[tokio::test]
async fn create_trigger_returns_201_with_custom_id() {
    let response = create_trigger(&json!({
        "id": "custom_trigger_id",
        "name": "test-trigger",
        "enabled": true,
        "event": "test.event",
        "action": "test_action"
    }))
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.json()["id"], "custom_trigger_id");
}

#[tokio::test]
async fn create_trigger_returns_400_without_name() {
    let response =
        create_trigger(&json!({"enabled": true, "event": "test.event", "action": "test_action"}))
            .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn create_trigger_returns_400_without_event() {
    let response =
        create_trigger(&json!({"name": "test-trigger", "enabled": true, "action": "test_action"}))
            .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn create_trigger_returns_400_without_action() {
    let response =
        create_trigger(&json!({"name": "test-trigger", "enabled": true, "event": "test.event"}))
            .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn create_trigger_returns_400_with_blank_name() {
    let response = create_trigger(
        &json!({"name": "   ", "enabled": true, "event": "test.event", "action": "test_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn create_trigger_returns_400_with_unsupported_content_type() {
    let response = crate::support::TestHarness::new()
        .await
        .post_yaml("/triggers", "plain text body", "text/plain")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "UnsupportedContentType");
}

#[tokio::test]
async fn create_trigger_returns_400_with_invalid_json() {
    let state = setup_state().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request_with_content_type(
            axum::http::Method::POST,
            "/triggers",
            "application/json",
            axum::body::Body::from("{invalid json"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "MalformedJson");
}

#[tokio::test]
async fn create_trigger_returns_400_with_unknown_field() {
    let response = create_trigger(&json!({
        "name": "test-trigger",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "unexpected": true
    }))
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "MalformedJson");
}

#[tokio::test]
async fn create_trigger_returns_400_with_invalid_id_format() {
    let response = create_trigger(&json!({
        "id": "bad$id",
        "name": "test-trigger",
        "enabled": true,
        "event": "test.event",
        "action": "test_action"
    }))
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "InvalidIdFormat");
}

#[tokio::test]
async fn create_trigger_with_condition_and_metadata() {
    let response = create_trigger(&json!({
        "name": "test-trigger",
        "enabled": true,
        "event": "test.event",
        "condition": "x > 5",
        "action": "test_action",
        "metadata": {"key": "value"}
    }))
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.json()["condition"], "x > 5");
    assert_eq!(response.json()["metadata"]["key"], "value");
}

#[tokio::test]
async fn create_trigger_returns_400_with_non_ascii_metadata_key() {
    let response = create_trigger(&json!({
        "name": "test-trigger",
        "enabled": true,
        "event": "test.event",
        "action": "test_action",
        "metadata": {"ключ": "value"}
    }))
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}
