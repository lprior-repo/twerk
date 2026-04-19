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

//! Batch-scale YAML parsing tests for 100-task workflow files.
//!
//! These tests verify:
//! 1. Parser handles large task arrays (100 tasks) without issues
//! 2. Each task in the array has correct structure
//! 3. POST /jobs accepts YAML with 100 tasks

use axum::http::{header, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::job::Job;
use twerk_core::task::TaskState;
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::yaml::from_slice;
use twerk_web::api::{create_router, AppState, Config};

const TWERK_NOOP_100_YAML: &str = include_str!("../../../examples/twerk-noop-100.yaml");
const TWERK_POKEMON_SHELL_100_YAML: &str = include_str!("../../../examples/twerk-pokemon-shell-100.yaml");

async fn setup_state() -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState::new(broker, ds, Config::default())
}

async fn body_to_json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// ============================================================================
// YAML Parsing Tests - twerk-noop-100.yaml
// ============================================================================

#[tokio::test]
async fn parse_twerk_noop_100_yaml_success() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok(), "Should parse successfully: {:?}", result.err());
    let job = result.unwrap();
    assert_eq!(job.name.as_deref(), Some("twerk-noop-stress"));
    assert_eq!(job.description.as_deref(), Some("Stress test twerk with no-op tasks (no containers, no API calls)"));
}

#[tokio::test]
async fn parse_twerk_noop_100_has_exactly_100_tasks() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    assert_eq!(tasks.len(), 100, "Should have exactly 100 tasks");
}

#[tokio::test]
async fn parse_twerk_noop_100_all_tasks_have_names() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        assert!(task.name.is_some(), "Task {} should have a name", i);
        let name = task.name.as_ref().unwrap();
        assert!(name.starts_with("noop-"), "Task {} name should start with 'noop-': {}", i, name);
    }
}

#[tokio::test]
async fn parse_twerk_noop_100_all_tasks_have_run_commands() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        assert!(task.run.is_some(), "Task {} should have a run command", i);
        let run = task.run.as_ref().unwrap();
        assert!(run.contains("echo"), "Task {} run should contain 'echo': {}", i, run);
    }
}

#[tokio::test]
async fn parse_twerk_noop_100_task_names_are_sequential() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        let name = task.name.as_ref().unwrap();
        let expected_suffix = format!("{:03}", i + 1);
        assert!(name.ends_with(&expected_suffix), "Task {} name should end with {}: {}", i, expected_suffix, name);
    }
}

#[tokio::test]
async fn parse_twerk_noop_100_no_tasks_have_images() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        assert!(task.image.is_none(), "Task {} should NOT have an image (noop tasks): {:?}", i, task.image);
    }
}

#[tokio::test]
async fn parse_twerk_noop_100_job_has_no_id_before_submission() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    assert!(job.id.is_none(), "Job should not have an ID before submission");
    assert!(job.created_at.is_none(), "Job should not have created_at before submission");
}

// ============================================================================
// YAML Parsing Tests - twerk-pokemon-shell-100.yaml
// ============================================================================

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_yaml_success() {
    let result: Result<Job, _> = from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes());
    assert!(result.is_ok(), "Should parse successfully: {:?}", result.err());
    let job = result.unwrap();
    assert_eq!(job.name.as_deref(), Some("twerk-pokemon-shell-stress"));
    assert_eq!(job.description.as_deref(), Some("Stress test twerk calling Pokemon API via shell"));
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_has_exactly_100_tasks() {
    let result: Result<Job, _> = from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    assert_eq!(tasks.len(), 100, "Should have exactly 100 tasks");
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_all_tasks_have_names() {
    let result: Result<Job, _> = from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        assert!(task.name.is_some(), "Task {} should have a name", i);
        let name = task.name.as_ref().unwrap();
        assert!(name.starts_with("fetch-"), "Task {} name should start with 'fetch-': {}", i, name);
    }
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_all_tasks_have_images() {
    let result: Result<Job, _> = from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        assert!(task.image.is_some(), "Task {} should have an image", i);
        let image = task.image.as_ref().unwrap();
        assert_eq!(image, "ubuntu:mantic", "Task {} image should be 'ubuntu:mantic': {}", i, image);
    }
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_all_tasks_have_run_commands() {
    let result: Result<Job, _> = from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        assert!(task.run.is_some(), "Task {} should have a run command", i);
        let run = task.run.as_ref().unwrap();
        assert!(run.contains("curl"), "Task {} run should contain 'curl': {}", i, run);
        assert!(run.contains("/api/pokemon/"), "Task {} run should contain '/api/pokemon/': {}", i, run);
    }
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_task_names_are_sequential() {
    let result: Result<Job, _> = from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        let name = task.name.as_ref().unwrap();
        let expected_suffix = format!("{:03}", i + 1);
        assert!(name.ends_with(&expected_suffix), "Task {} name should end with {}: {}", i, expected_suffix, name);
    }
}

#[tokio::test]
async fn parse_twerk_pokemon_shell_100_pokemon_ids_range_from_1_to_100() {
    let result: Result<Job, _> = from_slice(TWERK_POKEMON_SHELL_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.expect("Job should have tasks");
    for (i, task) in tasks.iter().enumerate() {
        let run = task.run.as_ref().unwrap();
        let pokemon_id = i + 1;
        assert!(run.contains(&format!("/api/pokemon/{}", pokemon_id)),
            "Task {} run should contain '/api/pokemon/{}': {}", i, pokemon_id, run);
    }
}

// ============================================================================
// POST /jobs Endpoint Tests - 100-task YAML
// ============================================================================

#[tokio::test]
async fn post_jobs_accepts_twerk_noop_100_yaml() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::from(TWERK_NOOP_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "POST /jobs should accept YAML with 100 tasks");
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "twerk-noop-stress");
    assert!(body["id"].is_string(), "Job should have an ID assigned");
    assert!(body["taskCount"].is_number(), "Job should have taskCount");
}

#[tokio::test]
async fn post_jobs_accepts_twerk_pokemon_shell_100_yaml() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::from(TWERK_POKEMON_SHELL_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "POST /jobs should accept YAML with 100 tasks");
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "twerk-pokemon-shell-stress");
    assert!(body["id"].is_string(), "Job should have an ID assigned");
    assert!(body["taskCount"].is_number(), "Job should have taskCount");
}

#[tokio::test]
async fn post_jobs_with_text_yaml_content_type_twerk_noop_100() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "text/yaml")
                .body(axum::body::Body::from(TWERK_NOOP_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "POST /jobs should accept text/yaml content type");
}

#[tokio::test]
async fn post_jobs_with_application_x_yaml_content_type_twerk_noop_100() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/x-yaml")
                .body(axum::body::Body::from(TWERK_NOOP_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "POST /jobs should accept application/x-yaml content type");
}

#[tokio::test]
async fn post_jobs_twerk_noop_100_creates_pending_job() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::from(TWERK_NOOP_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["state"], "PENDING", "Job should be in PENDING state");
}

#[tokio::test]
async fn post_jobs_twerk_pokemon_shell_100_creates_pending_job() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::from(TWERK_POKEMON_SHELL_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["state"], "PENDING", "Job should be in PENDING state");
}

// ============================================================================
// Validation Tests - Empty/Missing Fields
// ============================================================================

#[tokio::test]
async fn post_jobs_rejects_yaml_with_empty_body() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "POST /jobs should reject empty YAML body");
}

#[tokio::test]
async fn post_jobs_rejects_unsupported_content_type() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(axum::body::Body::from("some content"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "POST /jobs should reject unsupported content type");
}

// ============================================================================
// Large Task Array Boundary Tests
// ============================================================================

#[tokio::test]
async fn parse_yaml_with_100_tasks_completes_without_error() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    assert!(job.tasks.is_some());
    assert_eq!(job.tasks.unwrap().len(), 100);
}

#[tokio::test]
async fn parse_yaml_with_100_tasks_all_tasks_have_valid_structure() {
    let result: Result<Job, _> = from_slice(TWERK_NOOP_100_YAML.as_bytes());
    assert!(result.is_ok());
    let job = result.unwrap();
    let tasks = job.tasks.unwrap();

    for (i, task) in tasks.iter().enumerate() {
        assert!(task.name.is_some(), "Task {} missing name", i);
        assert!(task.run.is_some(), "Task {} missing run command", i);
        assert_eq!(task.state, TaskState::Created, "Task {} should have Created state by default", i);
    }
}

#[tokio::test]
async fn post_jobs_100_tasks_job_can_be_retrieved_by_id() {
    let state = setup_state().await;
    let app = create_router(state);

    let create_response = app.clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::from(TWERK_NOOP_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = body_to_json(create_response).await;
    let job_id = create_body["id"].as_str().expect("Job should have an ID");

    let get_response = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/jobs/{}", job_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = body_to_json(get_response).await;
    assert_eq!(get_body["name"], "twerk-noop-stress");

    let tasks = get_body["tasks"].as_array().expect("GET response should include tasks array");
    assert_eq!(tasks.len(), 100, "GET response should include all 100 tasks");
}

// ============================================================================
// Task Count Field Accuracy
// Note: taskCount is not auto-computed from tasks.len() in the YAML
// It is whatever is specified in the YAML (defaults to 0 if not specified)
// ============================================================================

#[tokio::test]
async fn post_jobs_twerk_noop_100_yaml_task_count_reflects_yaml_value() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::from(TWERK_NOOP_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    let task_count = body["taskCount"].as_i64().unwrap();
    assert_eq!(task_count, 0, "taskCount reflects YAML value (0 since not specified in YAML)");
}

#[tokio::test]
async fn post_jobs_twerk_pokemon_shell_100_yaml_task_count_reflects_yaml_value() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/yaml")
                .body(axum::body::Body::from(TWERK_POKEMON_SHELL_100_YAML.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_to_json(response).await;
    let task_count = body["taskCount"].as_i64().unwrap();
    assert_eq!(task_count, 0, "taskCount reflects YAML value (0 since not specified in YAML)");
}
