#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::semicolon_if_nothing_returned,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::unused_async,
    clippy::needless_raw_string_hashes
)]

use axum::http::{header, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::{create_router, AppState, Config};

// =============================================================================
// Setup and Helpers
// =============================================================================

async fn setup_state() -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState::new(broker, ds, Config::default())
}

async fn body_to_json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// Health Endpoint
// =============================================================================

#[tokio::test]
async fn health_returns_up_status() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "UP");
    assert!(body["version"].is_string());
}

// =============================================================================
// Jobs Endpoints
// =============================================================================

#[tokio::test]
async fn jobs_create_returns_job_summary() {
    let state = setup_state().await;
    let app = create_router(state);

    let job_input = json!({
        "name": "test-job",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "test-job");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn jobs_create_with_yaml_returns_job_summary() {
    let state = setup_state().await;
    let app = create_router(state);

    let yaml_input = "
name: test-job-yaml
tasks:
  - name: task-1
    image: alpine
    run: echo hello
";

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/x-yaml")
                .body(axum::body::Body::from(yaml_input))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "test-job-yaml");
}

#[tokio::test]
async fn jobs_list_returns_job_list() {
    let state = setup_state().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    // Create a job first
    let job = Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000001").unwrap()),
        name: Some("Test Job".to_string()),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["items"].is_array());
    assert!(!body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn jobs_get_returns_job() {
    let state = setup_state().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    // Create a job first
    let job = Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000002").unwrap()),
        name: Some("Test Job Get".to_string()),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/00000000-0000-0000-0000-000000000002")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "Test Job Get");
}

#[tokio::test]
async fn jobs_get_returns_404_for_nonexistent() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/non-existent-job")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn jobs_cancel_returns_ok_for_running_job() {
    let state = setup_state().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    // Create a running job (using string state for this version)
    let job = Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        name: Some("Cancel Test".to_string()),
        state: JobState::Running,
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000003/cancel")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn jobs_restart_returns_ok_for_failed_job() {
    let state = setup_state().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    // Create a failed job (using string state for this version)
    let job = Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000004").unwrap()),
        name: Some("Restart Test".to_string()),
        state: JobState::Failed,
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000004/restart")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Queues Endpoints
// =============================================================================

#[tokio::test]
async fn queues_list_returns_queue_list() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/queues")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    // In-memory broker should return empty or default queues
    let body = body_to_json(response).await;
    assert!(body.is_array());
}

#[tokio::test]
async fn queues_get_returns_queue_info() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/queues/default")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["name"].is_string());
}

#[tokio::test]
async fn queues_delete_returns_ok() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/queues/test-queue")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Nodes Endpoints
// =============================================================================

#[tokio::test]
async fn nodes_list_returns_node_list() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/nodes")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body.is_array());
}

// =============================================================================
// Metrics Endpoints
// =============================================================================

#[tokio::test]
async fn metrics_returns_metrics_data() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/metrics")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    // Metrics should have some structure
    assert!(body.is_object());
}

// =============================================================================
// Users Endpoints
// =============================================================================

#[tokio::test]
async fn users_create_returns_ok() {
    let state = setup_state().await;
    let app = create_router(state);

    let user_input = json!({
        "username": "testuser",
        "password": "testpassword"
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&user_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn users_create_returns_bad_request_without_username() {
    let state = setup_state().await;
    let app = create_router(state);

    let user_input = json!({
        "password": "testpassword"
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&user_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn users_create_returns_bad_request_without_password() {
    let state = setup_state().await;
    let app = create_router(state);

    let user_input = json!({
        "username": "testuser"
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&user_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =============================================================================
// Error Response Format
// =============================================================================

#[tokio::test]
async fn error_response_has_json_format() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/missing-job")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_to_json(response).await;
    assert!(body["message"].is_string());
}

// =============================================================================
// Input Validation
// =============================================================================

#[tokio::test]
async fn jobs_create_rejects_unsupported_content_type() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(axum::body::Body::from("plain text"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
