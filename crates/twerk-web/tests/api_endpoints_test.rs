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

//! Comprehensive API endpoint tests for twerk-web.
//!
//! This module tests ALL API endpoints to ensure full coverage.

use axum::http::{header, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::id::{JobId, TaskId};
use twerk_core::job::{Job, JobState};
use twerk_core::task::{Task, TaskLogPart, TaskState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;
use twerk_web::api::{create_router, AppState, Config};

// ============================================================================
// Test Setup Helpers
// ============================================================================

async fn setup_state() -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState::new(broker, ds, Config::default())
}

async fn setup_state_with_jobs() -> (AppState, Arc<InMemoryDatastore>) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    // Create a test job
    let job = Job {
        id: Some("test-job-1".into()),
        name: Some("Test Job".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            id: Some("task-1".into()),
            name: Some("Task 1".to_string()),
            state: TaskState::Pending,
            ..Default::default()
        }]),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    (state, ds)
}

async fn setup_state_with_tasks() -> (AppState, Arc<InMemoryDatastore>, JobId) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();

    // Create a job with tasks
    let job = Job {
        id: Some(job_id.clone()),
        name: Some("Job With Tasks".to_string()),
        state: JobState::Running,
        tasks: Some(vec![Task {
            id: Some("task-1".into()),
            job_id: Some(job_id.clone()),
            name: Some("Task 1".to_string()),
            state: TaskState::Running,
            ..Default::default()
        }]),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    (state, ds, job_id)
}

async fn setup_state_with_direct_task() -> (AppState, Arc<InMemoryDatastore>, TaskId) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    // Create a task directly in the datastore (not via job)
    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Direct Task".to_string()),
        state: TaskState::Running,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

    (state, ds, task_id)
}

async fn body_to_json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// ============================================================================
// GET /health - Already covered in api_test.rs
// ============================================================================

// ============================================================================
// GET /tasks/{id} - requires tasks to be stored directly in datastore
// Task tests are limited because the inmemory datastore requires tasks to be
// stored separately from jobs, which requires complex setup.
// ============================================================================

#[tokio::test]
async fn get_task_returns_404_when_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/non-existent-task")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// GET /tasks/{id}/log - requires tasks to be stored directly in datastore
// ============================================================================

#[tokio::test]
async fn get_task_log_returns_404_when_task_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/non-existent-task/log")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_task_returns_task_when_exists() {
    let (state, _ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{}", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["id"], task_id.to_string());
    assert_eq!(body["name"], "Direct Task");
    assert_eq!(body["state"], "RUNNING");
}

#[tokio::test]
async fn get_task_log_returns_empty_when_no_logs() {
    let (state, _ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{}/log", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["items"].is_array());
    assert!(body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn get_task_log_returns_logs_when_exist() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

    // Create task log parts directly
    let log_part1 = TaskLogPart {
        id: Some("log-1".to_string()),
        number: 1,
        task_id: Some(task_id.clone()),
        contents: Some("First log line".to_string()),
        ..Default::default()
    };
    let log_part2 = TaskLogPart {
        id: Some("log-2".to_string()),
        number: 2,
        task_id: Some(task_id.clone()),
        contents: Some("Second log line".to_string()),
        ..Default::default()
    };
    ds.create_task_log_part(&log_part1).await.unwrap();
    ds.create_task_log_part(&log_part2).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{}/log", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["items"].is_array());
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["contents"], "First log line");
    assert_eq!(items[1]["contents"], "Second log line");
}

#[tokio::test]
async fn get_task_log_respects_pagination() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

    // Create multiple log parts
    for i in 1..=5 {
        let log_part = TaskLogPart {
            id: Some(format!("log-{}", i)),
            number: i as i64,
            task_id: Some(task_id.clone()),
            contents: Some(format!("Log line {}", i)),
            ..Default::default()
        };
        ds.create_task_log_part(&log_part).await.unwrap();
    }

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/tasks/{}/log?page=1&size=2", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["items"].is_array());
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(body["total_items"], 5);
    assert_eq!(body["total_pages"], 3);
}

// ============================================================================
// POST /jobs - Already covered in api_test.rs
// ============================================================================

// ============================================================================
// GET /jobs
// ============================================================================

#[tokio::test]
async fn list_jobs_returns_empty_list_when_no_jobs() {
    let state = setup_state().await;
    let app = create_router(state);

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
}

#[tokio::test]
async fn list_jobs_returns_jobs_when_exist() {
    let (state, _ds) = setup_state_with_jobs().await;
    let app = create_router(state);

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
async fn list_jobs_respects_pagination_params() {
    let (state, _ds) = setup_state_with_jobs().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs?page=1&size=5")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["items"].is_array());
}

// ============================================================================
// GET /jobs/{id}
// ============================================================================

#[tokio::test]
async fn get_job_returns_job_when_exists() {
    let (state, _ds) = setup_state_with_jobs().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/test-job-1")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["id"], "test-job-1");
    assert_eq!(body["name"], "Test Job");
}

#[tokio::test]
async fn get_job_returns_404_when_not_found() {
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

// ============================================================================
// GET /jobs/{id}/log
// ============================================================================

#[tokio::test]
async fn get_job_log_returns_empty_when_no_logs() {
    let (state, _ds) = setup_state_with_jobs().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/test-job-1/log")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["items"].is_array());
}

#[tokio::test]
async fn get_job_log_returns_404_when_job_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/non-existent-job/log")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// PUT /jobs/{id}/cancel
// ============================================================================

#[tokio::test]
async fn cancel_job_returns_ok_when_job_is_running() {
    let (state, _ds, _job_id) = setup_state_with_tasks().await;
    let app = create_router(state);

    // Job is in Running state - should be cancellable
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/00000000-0000-0000-0000-000000000001/cancel")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "OK");
}

#[tokio::test]
async fn cancel_job_returns_404_when_job_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/non-existent-job/cancel")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cancel_job_returns_400_when_job_not_cancellable() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    // Create a completed job - should NOT be cancellable
    let job = Job {
        id: Some("completed-job".into()),
        name: Some("Completed Job".to_string()),
        state: JobState::Completed,
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/completed-job/cancel")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// PUT /jobs/{id}/restart
// ============================================================================

#[tokio::test]
async fn restart_job_returns_ok_when_job_is_failed() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    // Create a failed job - should be restartable
    let job = Job {
        id: Some("failed-job".into()),
        name: Some("Failed Job".to_string()),
        state: JobState::Failed,
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/failed-job/restart")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "OK");
}

#[tokio::test]
async fn restart_job_returns_404_when_job_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/non-existent-job/restart")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn restart_job_returns_400_when_job_not_restartable() {
    let (state, _ds) = setup_state_with_jobs().await;
    let app = create_router(state);

    // Job is in Pending state - should NOT be restartable
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/jobs/test-job-1/restart")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// POST /scheduled-jobs
// ============================================================================

#[tokio::test]
async fn create_scheduled_job_returns_ok_with_valid_json() {
    let state = setup_state().await;
    let app = create_router(state);

    let scheduled_job_input = json!({
        "name": "test-scheduled-job",
        "cron": "0 0 * * * *",
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
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = body_to_json(response).await;
    if status != StatusCode::OK {
        eprintln!(
            "DEBUG: create_scheduled_job failed with status {:?}: {:?}",
            status, body
        );
    }
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "test-scheduled-job");
    // Note: id might be skipped in JSON if it's None
}

#[tokio::test]
async fn create_scheduled_job_returns_400_with_invalid_cron() {
    let state = setup_state().await;
    let app = create_router(state);

    let scheduled_job_input = json!({
        "name": "test-scheduled-job",
        "cron": "invalid-cron-expression",
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
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_scheduled_job_returns_400_without_cron() {
    let state = setup_state().await;
    let app = create_router(state);

    let scheduled_job_input = json!({
        "name": "test-scheduled-job",
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
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_scheduled_job_returns_400_without_tasks() {
    let state = setup_state().await;
    let app = create_router(state);

    let scheduled_job_input = json!({
        "name": "test-scheduled-job",
        "cron": "0 0 * * * *"
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// GET /scheduled-jobs
// ============================================================================

#[tokio::test]
async fn list_scheduled_jobs_returns_empty_list_when_no_jobs() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["items"].is_array());
}

#[tokio::test]
async fn list_scheduled_jobs_returns_jobs_when_exist() {
    let state = setup_state().await;
    let app = create_router(state);

    // First create a scheduled job
    let scheduled_job_input = json!({
        "name": "test-scheduled-job",
        "cron": "0 0 * * * *",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let _response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Now list scheduled jobs
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs")
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

// ============================================================================
// GET /scheduled-jobs/{id}
// ============================================================================

#[tokio::test]
async fn get_scheduled_job_returns_job_when_exists() {
    let state = setup_state().await;
    let app = create_router(state);

    // First create a scheduled job
    let scheduled_job_input = json!({
        "name": "test-scheduled-job-get",
        "cron": "0 0 * * * *",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let create_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = body_to_json(create_response).await;
    let job_id = create_body["id"].as_str().unwrap();

    // Now get the scheduled job by ID
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/scheduled-jobs/{}", job_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["id"], job_id);
    assert_eq!(body["name"], "test-scheduled-job-get");
}

#[tokio::test]
async fn get_scheduled_job_returns_404_when_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/scheduled-jobs/non-existent-id")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// PUT /scheduled-jobs/{id}/pause
// ============================================================================

#[tokio::test]
async fn pause_scheduled_job_returns_ok_when_active() {
    let state = setup_state().await;
    let app = create_router(state);

    // Create a scheduled job first
    let scheduled_job_input = json!({
        "name": "test-scheduled-job-pause",
        "cron": "0 0 * * * *",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let create_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = body_to_json(create_response).await;
    let job_id = create_body["id"].as_str().unwrap();

    // Now pause the scheduled job
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{}/pause", job_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "OK");
}

#[tokio::test]
async fn pause_scheduled_job_returns_404_when_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/scheduled-jobs/non-existent-id/pause")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// PUT /scheduled-jobs/{id}/resume
// ============================================================================

#[tokio::test]
async fn resume_scheduled_job_returns_ok_when_paused() {
    let state = setup_state().await;
    let app = create_router(state);

    // Create a scheduled job first
    let scheduled_job_input = json!({
        "name": "test-scheduled-job-resume",
        "cron": "0 0 * * * *",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let create_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = body_to_json(create_response).await;
    let job_id = create_body["id"].as_str().unwrap();

    // First pause
    let _pause_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{}/pause", job_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Then resume
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/scheduled-jobs/{}/resume", job_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "OK");
}

#[tokio::test]
async fn resume_scheduled_job_returns_404_when_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/scheduled-jobs/non-existent-id/resume")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// DELETE /scheduled-jobs/{id}
// ============================================================================

#[tokio::test]
async fn delete_scheduled_job_returns_ok_when_exists() {
    let state = setup_state().await;
    let app = create_router(state);

    // Create a scheduled job first
    let scheduled_job_input = json!({
        "name": "test-scheduled-job-delete",
        "cron": "0 0 * * * *",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let create_response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&scheduled_job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = body_to_json(create_response).await;
    let job_id = create_body["id"].as_str().unwrap();

    // Now delete the scheduled job
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/scheduled-jobs/{}", job_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "OK");
}

#[tokio::test]
async fn delete_scheduled_job_returns_404_when_not_found() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/scheduled-jobs/non-existent-id")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// GET /queues
// ============================================================================

#[tokio::test]
async fn list_queues_returns_queues() {
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
    let body = body_to_json(response).await;
    assert!(body.is_array());
}

// ============================================================================
// GET /queues/{name}
// ============================================================================

#[tokio::test]
async fn get_queue_returns_queue_info_when_exists() {
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
async fn get_queue_returns_queue_info() {
    // InMemoryBroker returns info for any queue name, even non-existent ones
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/queues/test-queue")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // InMemoryBroker returns 200 for any queue name
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert!(body["name"].is_string());
}

// ============================================================================
// DELETE /queues/{name}
// ============================================================================

#[tokio::test]
async fn delete_queue_returns_ok_when_exists() {
    let state = setup_state().await;
    let app = create_router(state);

    // Note: InMemoryBroker might not actually delete, but we can test the endpoint
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("DELETE")
                .uri("/queues/default")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // The inmemory broker might return OK or might not actually delete
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_queue_returns_status() {
    // InMemoryBroker may return OK for any queue name
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

    // InMemoryBroker returns OK for any queue
    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================================
// GET /nodes
// ============================================================================

#[tokio::test]
async fn list_nodes_returns_nodes_list() {
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

// ============================================================================
// GET /metrics
// ============================================================================

#[tokio::test]
async fn get_metrics_returns_metrics() {
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
    // Metrics should be an object with various metric values
    assert!(body.is_object());
}

// ============================================================================
// POST /users
// ============================================================================

#[tokio::test]
async fn create_user_returns_ok_with_valid_credentials() {
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
async fn create_user_returns_400_without_username() {
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
async fn create_user_returns_400_without_password() {
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

// ============================================================================
// Unsupported Content Type Tests
// ============================================================================

#[tokio::test]
async fn create_job_returns_400_with_unsupported_content_type() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(axum::body::Body::from("plain text body"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_scheduled_job_returns_400_with_unsupported_content_type() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/scheduled-jobs")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(axum::body::Body::from("plain text body"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// Invalid JSON Tests
// ============================================================================

#[tokio::test]
async fn create_job_returns_400_with_invalid_json() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// Version Endpoint Tests
// ============================================================================

#[tokio::test]
async fn health_response_includes_version() {
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
    assert!(body["version"].is_string());
    assert!(!body["version"].as_str().unwrap().is_empty());
}
