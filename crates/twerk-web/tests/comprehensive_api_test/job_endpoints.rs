use axum::http::StatusCode;
use serde_json::json;
use twerk_core::job::JobState;

use crate::support::{assert_job_summary, assert_json_message, job, job_with_state, TestHarness};

const JOB_JSON_ID: &str = "00000000-0000-0000-0000-000000000001";
const JOB_GET_ID: &str = "00000000-0000-0000-0000-000000000002";
const JOB_CANCEL_ID: &str = "00000000-0000-0000-0000-000000000003";
const JOB_RESTART_ID: &str = "00000000-0000-0000-0000-000000000004";

fn sample_job_json(name: &str) -> serde_json::Value {
    json!({
        "name": name,
        "tasks": [{
            "name": "task-1",
            "image": "alpine",
            "run": "echo hello"
        }]
    })
}

#[tokio::test]
async fn jobs_create_returns_job_summary() {
    let harness = TestHarness::new().await;

    let response = harness
        .post_json("/jobs", &sample_job_json("test-job"))
        .await;

    assert_job_summary(&response, "test-job", "PENDING");
}

#[tokio::test]
async fn jobs_create_with_yaml_returns_job_summary() {
    let harness = TestHarness::new().await;

    let response = harness
        .post_yaml(
            "/jobs",
            r#"
name: test-job-yaml
tasks:
  - name: task-1
    image: alpine
    run: echo hello
"#,
            "application/x-yaml",
        )
        .await;

    assert_job_summary(&response, "test-job-yaml", "PENDING");
}

#[tokio::test]
async fn jobs_list_returns_job_list() {
    let harness = TestHarness::new().await;
    harness.seed_job(&job(JOB_JSON_ID, "Test Job")).await;

    let response = harness.get("/jobs").await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(response.json()["items"].as_array().unwrap().len(), 1);
    assert_eq!(response.json()["items"][0]["id"], JOB_JSON_ID);
    assert_eq!(response.json()["items"][0]["name"], "Test Job");
}

#[tokio::test]
async fn jobs_get_returns_job() {
    let harness = TestHarness::new().await;
    harness.seed_job(&job(JOB_GET_ID, "Test Job Get")).await;

    let response = harness.get(&format!("/jobs/{JOB_GET_ID}")).await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(response.json()["id"], JOB_GET_ID);
    assert_eq!(response.json()["name"], "Test Job Get");
}

#[tokio::test]
async fn jobs_get_returns_404_for_nonexistent() {
    let harness = TestHarness::new().await;

    let response = harness
        .get("/jobs/00000000-0000-0000-0000-000000000404")
        .await;

    assert_json_message(&response, StatusCode::NOT_FOUND, "job not found");
}

#[tokio::test]
async fn jobs_cancel_returns_ok_for_running_job() {
    let harness = TestHarness::new().await;
    harness
        .seed_job(&job_with_state(
            JOB_CANCEL_ID,
            "Cancel Test",
            JobState::Running,
        ))
        .await;

    let response = harness
        .put_empty(&format!("/jobs/{JOB_CANCEL_ID}/cancel"))
        .await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(response.json(), &json!({ "status": "OK" }));
}

#[tokio::test]
async fn jobs_restart_returns_ok_for_failed_job() {
    let harness = TestHarness::new().await;
    harness
        .seed_job(&job_with_state(
            JOB_RESTART_ID,
            "Restart Test",
            JobState::Failed,
        ))
        .await;

    let response = harness
        .put_empty(&format!("/jobs/{JOB_RESTART_ID}/restart"))
        .await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(response.json(), &json!({ "status": "OK" }));
}

#[tokio::test]
async fn jobs_create_rejects_unsupported_content_type() {
    let harness = TestHarness::new().await;

    let response = harness.post_yaml("/jobs", "plain text", "text/plain").await;

    assert_json_message(
        &response,
        StatusCode::BAD_REQUEST,
        "unsupported content type",
    );
}

#[tokio::test]
async fn error_response_has_json_format() {
    let harness = TestHarness::new().await;

    let response = harness
        .get("/jobs/00000000-0000-0000-0000-000000000405")
        .await;

    assert_json_message(&response, StatusCode::NOT_FOUND, "job not found");
}
