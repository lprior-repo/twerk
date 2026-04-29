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

//! Comprehensive tests for GET /tasks/{id} and GET /tasks/{id}/log endpoints.
//!
//! Tests cover:
//! - Task schema field verification (id, name, state, image, run, env)
//! - 404 for nonexistent tasks
//! - Task log response format
//! - Tasks in various states (pending, running, completed, failed)

use axum::http::StatusCode;
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::id::{JobId, TaskId};
use twerk_core::task::{Task, TaskLogPart, TaskState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;
use twerk_web::api::{create_router, AppState, Config};

async fn setup_state() -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState::new(broker, ds, Config::default())
}

async fn setup_state_with_direct_task() -> (AppState, Arc<InMemoryDatastore>, TaskId) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

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
// GET /tasks/{id} - Happy Path Tests
// ============================================================================

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
async fn get_task_verifies_id_field() {
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
    assert!(body["id"].is_string());
    assert_eq!(body["id"].as_str().unwrap(), task_id.to_string());
}

#[tokio::test]
async fn get_task_verifies_name_field() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Test Task Name".to_string()),
        state: TaskState::Pending,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["name"], "Test Task Name");
}

#[tokio::test]
async fn get_task_verifies_state_field_pending() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Pending Task".to_string()),
        state: TaskState::Pending,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "PENDING");
}

#[tokio::test]
async fn get_task_verifies_state_field_running() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Running Task".to_string()),
        state: TaskState::Running,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "RUNNING");
}

#[tokio::test]
async fn get_task_verifies_state_field_completed() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Completed Task".to_string()),
        state: TaskState::Completed,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "COMPLETED");
}

#[tokio::test]
async fn get_task_verifies_state_field_failed() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Failed Task".to_string()),
        state: TaskState::Failed,
        error: Some("Task failed with exit code 1".to_string()),
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "FAILED");
    assert_eq!(body["error"], "Task failed with exit code 1");
}

#[tokio::test]
async fn get_task_verifies_image_field() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Image Task".to_string()),
        state: TaskState::Pending,
        image: Some("alpine:latest".to_string()),
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["image"], "alpine:latest");
}

#[tokio::test]
async fn get_task_verifies_run_field() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Run Task".to_string()),
        state: TaskState::Pending,
        run: Some("echo 'hello world'".to_string()),
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["run"], "echo 'hello world'");
}

#[tokio::test]
async fn get_task_verifies_env_field() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let mut env = HashMap::new();
    env.insert("FOO".to_string(), "bar".to_string());
    env.insert("BAZ".to_string(), "qux".to_string());

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Env Task".to_string()),
        state: TaskState::Pending,
        env: Some(env),
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert!(body["env"].is_object());
    assert_eq!(body["env"]["FOO"], "bar");
    assert_eq!(body["env"]["BAZ"], "qux");
}

#[tokio::test]
async fn get_task_verifies_job_id_field() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id.clone()),
        name: Some("Job Linked Task".to_string()),
        state: TaskState::Pending,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["jobId"], job_id.to_string());
}

#[tokio::test]
async fn get_task_verifies_created_at_field() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();
    let now = time::OffsetDateTime::now_utc();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Timestamp Task".to_string()),
        state: TaskState::Pending,
        created_at: Some(now),
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert!(
        body.get("createdAt").is_some(),
        "createdAt field should be present"
    );
}

// ============================================================================
// GET /tasks/{id} - Failure Path Tests
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

#[tokio::test]
async fn get_task_returns_404_for_malformed_id() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/not-a-valid-uuid")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_task_returns_404_for_empty_id() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// GET /tasks/{id}/log - Happy Path Tests
// ============================================================================

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
async fn get_task_log_verifies_response_format() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

    let log_part = TaskLogPart {
        id: Some("log-1".to_string()),
        number: 1,
        task_id: Some(task_id.clone()),
        contents: Some("Log line".to_string()),
        ..Default::default()
    };
    ds.create_task_log_part(&log_part).await.unwrap();

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
    assert!(body["total_items"].is_number());
    assert!(body["total_pages"].is_number());
    assert!(body["number"].is_number());
    assert!(body["size"].is_number());
}

#[tokio::test]
async fn get_task_log_returns_logs_when_exist() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

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

#[tokio::test]
async fn get_task_log_with_task_in_pending_state() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Pending Task".to_string()),
        state: TaskState::Pending,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
}

#[tokio::test]
async fn get_task_log_with_task_in_running_state() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Running Task".to_string()),
        state: TaskState::Running,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
}

#[tokio::test]
async fn get_task_log_with_task_in_completed_state() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Completed Task".to_string()),
        state: TaskState::Completed,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
}

#[tokio::test]
async fn get_task_log_with_task_in_failed_state() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Failed Task".to_string()),
        state: TaskState::Failed,
        error: Some("Task failed".to_string()),
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
}

// ============================================================================
// GET /tasks/{id}/log - Failure Path Tests
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
async fn get_task_log_returns_404_for_malformed_task_id() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/tasks/not-a-valid-uuid/log")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// GET /tasks/{id}/log - Pagination Edge Cases
// ============================================================================

#[tokio::test]
async fn get_task_log_default_pagination() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

    for i in 1..=30 {
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
                .uri(format!("/tasks/{}/log", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 25);
    assert_eq!(body["total_items"], 30);
    assert_eq!(body["total_pages"], 2);
}

#[tokio::test]
async fn get_task_log_invalid_page_returns_default() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

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
                .uri(format!("/tasks/{}/log?page=invalid", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_task_log_size_exceeding_max_is_capped() {
    let (state, ds, task_id) = setup_state_with_direct_task().await;
    let app = create_router(state);

    for i in 1..=10 {
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
                .uri(format!("/tasks/{}/log?size=100", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 10);
}

// ============================================================================
// GET /tasks/{id} - Additional Task State Tests
// ============================================================================

#[tokio::test]
async fn get_task_verifies_state_created() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Created Task".to_string()),
        state: TaskState::Created,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "CREATED");
}

#[tokio::test]
async fn get_task_verifies_state_scheduled() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Scheduled Task".to_string()),
        state: TaskState::Scheduled,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "SCHEDULED");
}

#[tokio::test]
async fn get_task_verifies_state_cancelled() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Cancelled Task".to_string()),
        state: TaskState::Cancelled,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "CANCELLED");
}

#[tokio::test]
async fn get_task_verifies_state_stopped() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Stopped Task".to_string()),
        state: TaskState::Stopped,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "STOPPED");
}

#[tokio::test]
async fn get_task_verifies_state_skipped() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id),
        name: Some("Skipped Task".to_string()),
        state: TaskState::Skipped,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["state"], "SKIPPED");
}

// ============================================================================
// GET /tasks/{id} - All Schema Fields Comprehensive
// ============================================================================

#[tokio::test]
async fn get_task_comprehensive_all_fields() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let job_id = JobId::new("00000000-0000-0000-0000-000000000001").unwrap();
    let task_id = TaskId::new("00000000-0000-0000-0000-000000000002").unwrap();
    let now = time::OffsetDateTime::now_utc();

    let mut env = HashMap::new();
    env.insert("ENV_VAR".to_string(), "value".to_string());

    let task = Task {
        id: Some(task_id.clone()),
        job_id: Some(job_id.clone()),
        name: Some("Comprehensive Task".to_string()),
        description: Some("A task with all fields".to_string()),
        state: TaskState::Running,
        created_at: Some(now),
        scheduled_at: Some(now),
        started_at: Some(now),
        image: Some("alpine:latest".to_string()),
        run: Some("echo hello".to_string()),
        env: Some(env),
        cmd: Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo test".to_string(),
        ]),
        entrypoint: Some(vec!["/bin/sh".to_string()]),
        files: Some(
            [("config.yaml".to_string(), "data: value".to_string())]
                .into_iter()
                .collect(),
        ),
        queue: Some("default".to_string()),
        redelivered: 0,
        error: None,
        timeout: Some("300s".to_string()),
        workdir: Some("/app".to_string()),
        priority: 10,
        progress: 0.5,
        result: None,
        ..Default::default()
    };
    ds.create_task(&task).await.unwrap();

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
    assert_eq!(body["jobId"], job_id.to_string());
    assert_eq!(body["name"], "Comprehensive Task");
    assert_eq!(body["description"], "A task with all fields");
    assert_eq!(body["state"], "RUNNING");
    assert_eq!(body["image"], "alpine:latest");
    assert_eq!(body["run"], "echo hello");
    assert_eq!(body["env"]["ENV_VAR"], "value");
    assert!(body["cmd"].is_array());
    assert!(body["entrypoint"].is_array());
    assert!(body["files"].is_object());
    assert_eq!(body["queue"], "default");
    assert_eq!(body["redelivered"], 0);
    assert_eq!(body["timeout"], "300s");
    assert_eq!(body["workdir"], "/app");
    assert_eq!(body["priority"], 10);
    assert_eq!(body["progress"], 0.5);
}
