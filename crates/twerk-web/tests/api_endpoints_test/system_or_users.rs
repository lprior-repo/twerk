use axum::http::StatusCode;
use serde_json::json;

use crate::support::TestHarness;

#[tokio::test]
async fn list_nodes_returns_nodes_list() {
    let response = TestHarness::new().await.get("/nodes").await;
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert!(response.json().is_array());
}

#[tokio::test]
async fn get_metrics_returns_metrics() {
    let response = TestHarness::new().await.get("/metrics").await;
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert!(response.json().is_object());
}

#[tokio::test]
async fn create_user_returns_ok_with_valid_credentials() {
    let response = TestHarness::new()
        .await
        .post_json(
            "/users",
            &json!({"username": "testuser", "password": "testpassword"}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_user_returns_400_without_username() {
    let response = TestHarness::new()
        .await
        .post_json("/users", &json!({"password": "testpassword"}))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_user_returns_400_without_password() {
    let response = TestHarness::new()
        .await
        .post_json("/users", &json!({"username": "testuser"}))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_job_returns_400_with_unsupported_content_type() {
    let response = TestHarness::new()
        .await
        .post_yaml("/jobs", "plain text body", "text/plain")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_scheduled_job_returns_400_with_unsupported_content_type() {
    let response = TestHarness::new()
        .await
        .post_yaml("/scheduled-jobs", "plain text body", "text/plain")
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_job_returns_400_with_invalid_json() {
    let response = TestHarness::new()
        .await
        .call(crate::support::request_with_content_type(
            axum::http::Method::POST,
            "/jobs",
            "application/json",
            axum::body::Body::from("{invalid json"),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn health_response_includes_version() {
    let response = TestHarness::new().await.get("/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.json()["version"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
}
