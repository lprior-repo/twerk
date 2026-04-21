use axum::http::StatusCode;
use serde_json::json;

use crate::support::{assert_empty_body, assert_json_message, assert_queue_state, TestHarness};

#[tokio::test]
async fn queues_list_returns_queue_list() {
    let harness = TestHarness::with_queue("default").await;

    let response = harness.get("/queues").await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(
        response.json(),
        &json!([{"name": "default", "size": 1, "subscribers": 0, "unacked": 0}])
    );
}

#[tokio::test]
async fn queues_get_returns_queue_info() {
    let harness = TestHarness::with_queue("default").await;

    let response = harness.get("/queues/default").await;

    assert_queue_state(&response, "default", 1);
}

#[tokio::test]
async fn queues_delete_returns_ok() {
    let harness = TestHarness::with_queue("test-queue").await;

    let delete_response = harness.delete("/queues/test-queue").await;
    let get_after_delete = harness.get("/queues/test-queue").await;

    assert_empty_body(&delete_response, StatusCode::OK);
    assert_json_message(
        &get_after_delete,
        StatusCode::NOT_FOUND,
        "queue test-queue not found",
    );
}

#[tokio::test]
async fn queues_get_missing_returns_404() {
    let harness = TestHarness::new().await;

    let response = harness.get("/queues/test-queue").await;

    assert_json_message(
        &response,
        StatusCode::NOT_FOUND,
        "queue test-queue not found",
    );
}

#[tokio::test]
async fn users_create_returns_ok() {
    let harness = TestHarness::new().await;

    let response = harness
        .post_json(
            "/users",
            &json!({"username": "testuser", "password": "testpassword"}),
        )
        .await;

    assert_empty_body(&response, StatusCode::OK);
}

#[tokio::test]
async fn users_create_returns_bad_request_without_username() {
    let harness = TestHarness::new().await;

    let response = harness
        .post_json("/users", &json!({"password": "testpassword"}))
        .await;

    assert_json_message(&response, StatusCode::BAD_REQUEST, "username is required");
}

#[tokio::test]
async fn users_create_returns_bad_request_without_password() {
    let harness = TestHarness::new().await;

    let response = harness
        .post_json("/users", &json!({"username": "testuser"}))
        .await;

    assert_json_message(&response, StatusCode::BAD_REQUEST, "password is required");
}
