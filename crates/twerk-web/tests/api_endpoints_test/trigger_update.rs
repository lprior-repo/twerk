use axum::http::StatusCode;
use serde_json::json;

use super::shared::setup_state_with_triggers;

async fn update_trigger(path: &str, payload: &serde_json::Value) -> crate::support::TestResponse {
    let (state, _) = setup_state_with_triggers().await;
    crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::json_request(axum::http::Method::PUT, path, payload),
    )
    .await
}

#[tokio::test]
async fn update_trigger_returns_updated_trigger() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"name": "updated-trigger-name", "enabled": false, "event": "updated.event", "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.json()["name"], "updated-trigger-name");
    assert_eq!(response.json()["enabled"], false);
    assert_eq!(response.json()["event"], "updated.event");
    assert_eq!(response.json()["action"], "updated_action");
}

#[tokio::test]
async fn update_trigger_returns_404_when_not_found() {
    let response = update_trigger(
        "/api/v1/triggers/non-existent-trigger",
        &json!({"name": "updated-trigger", "enabled": true, "event": "updated.event", "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.json()["error"], "TriggerNotFound");
}

#[tokio::test]
async fn update_trigger_returns_400_for_invalid_id_format() {
    let response = update_trigger(
        "/api/v1/triggers/bad$id",
        &json!({"name": "updated-trigger", "enabled": true, "event": "updated.event", "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "InvalidIdFormat");
}

#[tokio::test]
async fn update_trigger_returns_400_with_id_mismatch() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"id": "different_id", "name": "updated-trigger", "enabled": true, "event": "updated.event", "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "IdMismatch");
}

#[tokio::test]
async fn update_trigger_returns_400_with_unsupported_content_type() {
    let (state, _) = setup_state_with_triggers().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request_with_content_type(
            axum::http::Method::PUT,
            "/api/v1/triggers/trg_test_1",
            "text/plain",
            axum::body::Body::from("plain text body"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "UnsupportedContentType");
}

#[tokio::test]
async fn update_trigger_returns_400_with_invalid_json() {
    let (state, _) = setup_state_with_triggers().await;
    let response = crate::support::call(
        &twerk_web::api::create_router(state),
        crate::support::request_with_content_type(
            axum::http::Method::PUT,
            "/api/v1/triggers/trg_test_1",
            "application/json",
            axum::body::Body::from("{invalid json"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "MalformedJson");
}

#[tokio::test]
async fn update_trigger_returns_400_when_enabled_has_wrong_type() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"name": "updated-trigger", "enabled": "yes", "event": "updated.event", "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "MalformedJson");
}

#[tokio::test]
async fn update_trigger_returns_400_without_name() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"enabled": true, "event": "updated.event", "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn update_trigger_returns_400_without_event() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"name": "updated-trigger", "enabled": true, "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn update_trigger_returns_400_without_action() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"name": "updated-trigger", "enabled": true, "event": "updated.event"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn update_trigger_returns_400_with_blank_name() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"name": "   ", "enabled": true, "event": "updated.event", "action": "updated_action"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn update_trigger_returns_400_with_non_ascii_metadata_key() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"name": "updated-trigger", "enabled": true, "event": "updated.event", "action": "updated_action", "metadata": {"ключ": "value"}}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.json()["error"], "ValidationFailed");
}

#[tokio::test]
async fn update_trigger_returns_400_with_version_zero() {
    let response = update_trigger(
        "/api/v1/triggers/trg_test_1",
        &json!({"name": "updated-trigger", "enabled": true, "event": "updated.event", "action": "updated_action", "version": 0}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.json()["error"], "VersionConflict");
}
