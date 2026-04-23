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

//! 12-Step QA Test Suite
//!
//! Exercises every API endpoint and end-user workflow via in-memory
//! broker + datastore (no Docker, no RabbitMQ, no network).
//!
//! Step  1: Health check & bootstrap
//! Step  2: Submit job via YAML
//! Step  3: Submit job via JSON
//! Step  4: List jobs, pagination, search
//! Step  5: Get job details, task details, task logs
//! Step  6: Cancel & restart job lifecycle
//! Step  7: Edge cases — unsupported content-type, missing fields
//! Step  8: Triggers CRUD
//! Step  9: Scheduled jobs CRUD
//! Step 10: Queues, nodes, metrics
//! Step 11: User creation & validation
//! Step 12: Error handling — nonexistent resources, invalid input

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_core::task::{Task, TaskLogPart, TaskState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::{create_router, AppState, Config};

// UUID constants for test data
const JOB_1: &str = "00000000-0000-0000-0000-000000000001";
const JOB_2: &str = "00000000-0000-0000-0000-000000000002";
const JOB_3: &str = "00000000-0000-0000-0000-000000000003";
const JOB_4: &str = "00000000-0000-0000-0000-000000000004";
const JOB_5: &str = "00000000-0000-0000-0000-000000000005";
const JOB_6: &str = "00000000-0000-0000-0000-000000000006";
const JOB_7: &str = "00000000-0000-0000-0000-000000000007";
const JOB_8: &str = "00000000-0000-0000-0000-000000000008";
const MISSING_JOB: &str = "00000000-0000-0000-0000-000000000404";
const TASK_1: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const TASK_2: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
const LOGPART_1: &str = "cccccccc-cccc-cccc-cccc-cccccccccccc";

async fn setup() -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState::new(broker, ds, Config::default())
}

async fn body_json(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn make_job(id: &str, name: &str, state: JobState) -> Job {
    Job {
        id: Some(JobId::new(id).unwrap()),
        name: Some(name.to_string()),
        state,
        ..Default::default()
    }
}

fn make_task(id: &str, job_id: &str, name: &str, state: TaskState) -> Task {
    Task {
        id: Some(id.into()),
        job_id: Some(JobId::new(job_id).unwrap()),
        name: Some(name.to_string()),
        state,
        ..Default::default()
    }
}

// =============================================================================
// Step 1: Health Check & Bootstrap
// =============================================================================

#[tokio::test]
async fn step_01_health_check_returns_up() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["status"], "UP");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn step_01_openapi_spec_served() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["info"]["title"], "Twerk API");
    assert_eq!(
        body["paths"]["/jobs"]["post"]["requestBody"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/Job"
    );
}

// =============================================================================
// Step 2: Submit Job via YAML
// =============================================================================

#[tokio::test]
async fn step_02_submit_job_yaml() {
    let state = setup().await;
    let app = create_router(state);

    let yaml = r#"
name: qa-02-yaml
output: "{{ tasks.hello }}"
tasks:
  - var: hello
    name: simple task
    image: ubuntu:mantic
    run: echo -n "hello world" > $TORK_OUTPUT
"#;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "text/yaml")
                .body(Body::from(yaml))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["name"], "qa-02-yaml");
    assert!(body["id"].is_string());
}

// =============================================================================
// Step 3: Submit Job via JSON
// =============================================================================

#[tokio::test]
async fn step_03_submit_job_json() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "qa-03-json",
                        "tasks": [{"name": "echo", "image": "ubuntu:mantic", "run": "echo hello"}]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["name"], "qa-03-json");
}

// =============================================================================
// Step 4: List Jobs + Pagination + Search
// =============================================================================

#[tokio::test]
async fn step_04_list_jobs_returns_array() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body["items"].is_array());
}

#[tokio::test]
async fn step_04_list_jobs_pagination() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_4, "page-job-a", JobState::Pending))
        .await
        .unwrap();
    ds.create_job(&make_job(JOB_5, "page-job-b", JobState::Pending))
        .await
        .unwrap();
    ds.create_job(&make_job(JOB_6, "page-job-c", JobState::Pending))
        .await
        .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs?page=1&size=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(
        body["items"].is_array(),
        "response must contain items array"
    );
    let items = body["items"].as_array().unwrap();
    assert!(
        items.len() <= 2,
        "page size=2 must return at most 2 items, got {}",
        items.len()
    );
}

#[tokio::test]
async fn step_04_list_jobs_invalid_page_rejected() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs?page=abc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["message"], "page must be a positive integer");
}

#[tokio::test]
async fn step_04_list_jobs_search_query() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs?q=qa")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body["items"].is_array());
}

#[tokio::test]
async fn step_04_created_job_appears_in_list() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_1, "visible-job", JobState::Pending))
        .await
        .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty());
}

// =============================================================================
// Step 5: Job Details, Task Details, Logs
// =============================================================================

#[tokio::test]
async fn step_05_get_job_by_id() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_2, "detail-job", JobState::Pending))
        .await
        .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_2}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["name"], "detail-job");
}

#[tokio::test]
async fn step_05_get_nonexistent_job_404() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{MISSING_JOB}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn step_05_get_job_log() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_3, "log-job", JobState::Pending))
        .await
        .unwrap();
    ds.create_task(&make_task(TASK_1, JOB_3, "log-task", TaskState::Completed))
        .await
        .unwrap();
    ds.create_task_log_part(&TaskLogPart {
        id: Some(LOGPART_1.into()),
        task_id: Some(TASK_1.into()),
        number: 1,
        contents: Some("job log line 1".to_string()),
        ..Default::default()
    })
    .await
    .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_3}/log"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(
        body["items"].is_array(),
        "response must contain items array"
    );
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty(), "expected at least one log part");
    assert_eq!(items[0]["contents"], "job log line 1");
}

#[tokio::test]
async fn step_05_get_task_by_id() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_4, "task-job", JobState::Pending))
        .await
        .unwrap();
    ds.create_task(&make_task(TASK_1, JOB_4, "my-task", TaskState::Completed))
        .await
        .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{TASK_1}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["name"], "my-task");
}

#[tokio::test]
async fn step_05_get_task_log_paginated() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_5, "tlog-job", JobState::Pending))
        .await
        .unwrap();
    ds.create_task(&make_task(
        TASK_2,
        JOB_5,
        "logged-task",
        TaskState::Completed,
    ))
    .await
    .unwrap();
    ds.create_task_log_part(&TaskLogPart {
        id: Some(LOGPART_1.into()),
        task_id: Some(TASK_2.into()),
        number: 1,
        contents: Some("line 1 output".to_string()),
        ..Default::default()
    })
    .await
    .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{TASK_2}/log?page=1&size=10"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body["items"].is_array());
    let items = body["items"].as_array().unwrap();
    assert!(!items.is_empty(), "expected at least one log part");
    assert_eq!(items[0]["contents"], "line 1 output");
}

// =============================================================================
// Step 6: Cancel & Restart Job Lifecycle
// =============================================================================

#[tokio::test]
async fn step_06_cancel_pending_job() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_6, "cancel-me", JobState::Scheduled))
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/jobs/{JOB_6}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["status"], "OK");

    let verify = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_6}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify.status(), StatusCode::OK);
    let body = body_json(verify).await;
    assert_eq!(body["state"], "CANCELLED");
}

#[tokio::test]
async fn step_06_cancel_completed_returns_400() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_7, "already-done", JobState::Completed))
        .await
        .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/jobs/{JOB_7}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_06_restart_cancelled_job() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_8, "restart-me", JobState::Cancelled))
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/jobs/{JOB_8}/restart"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["status"], "OK");

    let verify = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_8}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify.status(), StatusCode::OK);
    let body = body_json(verify).await;
    assert_eq!(body["state"], "RESTART");
}

#[tokio::test]
async fn step_06_restart_running_job_returns_400() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_1, "still-going", JobState::Running))
        .await
        .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/jobs/{JOB_1}/restart"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_06_post_cancel_endpoint_works() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    ds.create_job(&make_job(JOB_2, "post-cancel", JobState::Scheduled))
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/jobs/{JOB_2}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let verify = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_2}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify.status(), StatusCode::OK);
    let body = body_json(verify).await;
    assert_eq!(body["state"], "CANCELLED");
}

// =============================================================================
// Step 7: Edge Cases
// =============================================================================

#[tokio::test]
async fn step_07_unsupported_content_type_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("not a job"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_07_invalid_json_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{not valid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_07_empty_job_body_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_07_nonexistent_task_404() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/no-such-task")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Step 8: Triggers CRUD
// =============================================================================

#[tokio::test]
async fn step_08_triggers_list_empty() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/triggers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body.as_array().is_some());
}

#[tokio::test]
async fn step_08_triggers_full_crud() {
    let state = setup().await;
    let app = create_router(state);

    // Create
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/triggers")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "qa-trigger",
                        "enabled": true,
                        "event": "job.completed",
                        "action": "notify",
                        "metadata": {"channel": "slack"}
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    let trigger_id = body["id"].as_str().unwrap().to_string();
    let version = body["version"].as_u64().unwrap();
    assert!(!trigger_id.is_empty());
    assert_eq!(body["metadata"], json!({"channel": "slack"}));

    // Get
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/v1/triggers/{trigger_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["name"], "qa-trigger");

    // Update
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/triggers/{trigger_id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "qa-trigger-v2",
                        "enabled": true,
                        "event": "job.failed",
                        "action": "alert",
                        "metadata": {},
                        "version": version
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["name"], "qa-trigger-v2");
    let new_version = body["version"].as_u64().unwrap();
    assert_eq!(new_version, version + 1);

    // Stale version -> 409
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/triggers/{trigger_id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "stale",
                        "enabled": true,
                        "event": "x",
                        "action": "y",
                        "version": version
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = body_json(response).await;
    assert_eq!(body["error"], "VersionConflict");
    assert_eq!(body["message"], "stale version supplied");

    // Delete
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/triggers/{trigger_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Get after delete -> 404
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/v1/triggers/{trigger_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn step_08_trigger_blank_name_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/triggers")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(
                        &json!({"name": "", "enabled": true, "event": "x", "action": "y"}),
                    )
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "ValidationFailed");
    assert_eq!(body["message"], "name must be non-empty after trim");
}

#[tokio::test]
async fn step_08_trigger_field_too_long_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/triggers")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "x".repeat(100), "enabled": true, "event": "e", "action": "a"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "ValidationFailed");
    assert_eq!(body["message"], "name exceeds max length");
}

#[tokio::test]
async fn step_08_trigger_invalid_id_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/triggers/bad%20id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "InvalidIdFormat");
    assert_eq!(body["message"], "bad id");
}

#[tokio::test]
async fn step_08_trigger_id_mismatch_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/api/v1/triggers/trg-abc")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "n", "enabled": true, "event": "e", "action": "a", "id": "trg-xyz"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "IdMismatch");
}

#[tokio::test]
async fn step_08_trigger_update_nonexistent_404() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/api/v1/triggers/trg-nonexistent")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(
                        &json!({"name": "n", "enabled": true, "event": "e", "action": "a"}),
                    )
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"], "TriggerNotFound");
    assert_eq!(body["message"], "trg-nonexistent");
}

#[tokio::test]
async fn step_08_trigger_delete_nonexistent_404() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/api/v1/triggers/trg-nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"], "TriggerNotFound");
    assert_eq!(body["message"], "trg-nonexistent");
}

#[tokio::test]
async fn step_08_trigger_malformed_json_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/triggers")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{not json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "MalformedJson");
    assert_eq!(body["message"], "malformed JSON body");
}

#[tokio::test]
async fn step_08_trigger_unsupported_content_type_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/api/v1/triggers/trg-1")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("not json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "UnsupportedContentType");
    assert_eq!(body["message"], "text/plain");
}

#[tokio::test]
async fn step_08_trigger_body_too_large_400() {
    let state = setup().await;
    let app = create_router(state);

    let payload = serde_json::to_vec(&json!({
        "name": "x".repeat(20_000),
        "enabled": true,
        "event": "e",
        "action": "a"
    }))
    .unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/triggers")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "ValidationFailed");
    assert_eq!(body["message"], "request body exceeds max size");
}

// =============================================================================
// Step 9: Scheduled Jobs CRUD
// =============================================================================

#[tokio::test]
async fn step_09_scheduled_jobs_full_lifecycle() {
    let state = setup().await;
    let app = create_router(state);

    // Create
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "qa-scheduled",
                        "cron": "0 */5 * * * *",
                        "tasks": [{"name": "tick", "image": "ubuntu:mantic", "run": "echo tick"}]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let sj_id = body["id"].as_str().unwrap().to_string();

    // List
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|sj| sj["id"] == sj_id));

    // Get
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/scheduled-jobs/{sj_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["name"], "qa-scheduled");

    // Pause
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{sj_id}/pause"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify paused
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/scheduled-jobs/{sj_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_json(response).await;
    assert_eq!(body["state"], "PAUSED");

    // Double pause -> 400
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{sj_id}/pause"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Resume
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{sj_id}/resume"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Delete
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/scheduled-jobs/{sj_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn step_09_missing_cron_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "no-cron",
                        "tasks": [{"name": "t", "image": "alpine", "run": "echo hi"}]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_09_missing_tasks_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "no-tasks", "cron": "*/5 * * * *"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_09_invalid_cron_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "bad-cron",
                        "cron": "not-a-cron",
                        "tasks": [{"name": "t", "image": "alpine", "run": "echo"}]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_09_double_resume_returns_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "qa-double-resume",
                        "cron": "0 */5 * * * *",
                        "tasks": [{"name": "tick", "image": "ubuntu:mantic", "run": "echo tick"}]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let sj_id = body["id"].as_str().unwrap().to_string();

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{sj_id}/pause"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{sj_id}/resume"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{sj_id}/resume"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =============================================================================
// Step 10: Queues, Nodes, Metrics
// =============================================================================

#[tokio::test]
async fn step_10_list_queues() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/queues")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body.is_array(), "queues response must be a JSON array");
    let queues = body.as_array().unwrap();
    assert!(queues.is_empty(), "InMemoryBroker starts with no queues");
}

#[tokio::test]
async fn step_10_list_nodes() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body.is_array(), "nodes response must be a JSON array");
    let nodes = body.as_array().unwrap();
    assert!(nodes.is_empty(), "in-memory datastore starts with no nodes");
}

#[tokio::test]
async fn step_10_get_metrics() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body.is_object(), "metrics response must be a JSON object");
    assert!(body["jobs"].is_object(), "metrics must contain 'jobs' key");
    assert!(
        body["tasks"].is_object(),
        "metrics must contain 'tasks' key"
    );
    assert!(
        body["nodes"].is_object(),
        "metrics must contain 'nodes' key"
    );
}

#[tokio::test]
async fn step_10_get_queue_by_name_404_for_nonexistent() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/queues/no-such-queue")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn step_10_delete_queue_404_for_nonexistent() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/queues/no-such-queue")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Step 11: User Creation & Validation
// =============================================================================

#[tokio::test]
async fn step_11_create_valid_user() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"username": "qauser", "password": "password123"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let user = ds.get_user("qauser").await.unwrap();
    assert_eq!(user.username.as_deref(), Some("qauser"));
    assert!(user.password_hash.is_some());
}

#[tokio::test]
async fn step_11_short_password_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"username": "baduser", "password": "short"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_11_missing_fields_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn step_11_short_username_400() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/users")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"username": "ab", "password": "password123"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// =============================================================================
// Step 12: Error Handling
// =============================================================================

#[tokio::test]
async fn step_12_nonexistent_scheduled_job_404() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs/nonexistent-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn step_12_nonexistent_task_log_404() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/no-such-task/log")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn step_12_nonexistent_job_log_404() {
    let state = setup().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{MISSING_JOB}/log"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn step_12_job_secrets_redacted() {
    let state = setup().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    let job = Job {
        id: Some(JobId::new(JOB_3).unwrap()),
        name: Some("secret-job".to_string()),
        secrets: Some([("my_secret".to_string(), "password123".to_string())].into()),
        inputs: Some([("api_key".to_string(), "password123".to_string())].into()),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{JOB_3}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["secrets"]["my_secret"], "[REDACTED]");
    assert_eq!(body["inputs"]["api_key"], "[REDACTED]");
}
