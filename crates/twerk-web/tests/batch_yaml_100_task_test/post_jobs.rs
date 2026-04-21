use axum::http::StatusCode;
use twerk_core::job::Job;
use twerk_web::api::yaml::from_slice;

use crate::support::{assert_job_summary, TestHarness};

const TWERK_NOOP_100_YAML: &str = include_str!("../../../../examples/twerk-noop-100.yaml");
const TWERK_POKEMON_SHELL_100_YAML: &str =
    include_str!("../../../../examples/twerk-pokemon-shell-100.yaml");

fn parse_noop_job() -> Job {
    from_slice(TWERK_NOOP_100_YAML.as_bytes()).unwrap()
}

fn assert_task_slice(job_json: &serde_json::Value, first: &str, last: &str) {
    let tasks = job_json["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 100);
    assert_eq!(tasks.first().expect("first task")["name"], first);
    assert_eq!(tasks.last().expect("last task")["name"], last);
}

#[tokio::test]
async fn post_jobs_accepts_twerk_noop_100_yaml() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml("/jobs", TWERK_NOOP_100_YAML.to_string(), "application/yaml")
        .await;

    assert_job_summary(&response, "twerk-noop-stress", "PENDING");
    assert_eq!(response.json()["taskCount"], 100);
}

#[tokio::test]
async fn post_jobs_accepts_twerk_pokemon_shell_100_yaml() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml(
            "/jobs",
            TWERK_POKEMON_SHELL_100_YAML.to_string(),
            "application/yaml",
        )
        .await;

    assert_job_summary(&response, "twerk-pokemon-shell-stress", "PENDING");
    assert_eq!(response.json()["taskCount"], 100);
}

#[tokio::test]
async fn post_jobs_with_text_yaml_content_type_twerk_noop_100() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml("/jobs", TWERK_NOOP_100_YAML.to_string(), "text/yaml")
        .await;

    assert_job_summary(&response, "twerk-noop-stress", "PENDING");
}

#[tokio::test]
async fn post_jobs_with_application_x_yaml_content_type_twerk_noop_100() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml(
            "/jobs",
            TWERK_NOOP_100_YAML.to_string(),
            "application/x-yaml",
        )
        .await;

    assert_job_summary(&response, "twerk-noop-stress", "PENDING");
}

#[tokio::test]
async fn post_jobs_twerk_noop_100_creates_pending_job() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml("/jobs", TWERK_NOOP_100_YAML.to_string(), "application/yaml")
        .await;

    assert_job_summary(&response, "twerk-noop-stress", "PENDING");
}

#[tokio::test]
async fn post_jobs_twerk_pokemon_shell_100_creates_pending_job() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml(
            "/jobs",
            TWERK_POKEMON_SHELL_100_YAML.to_string(),
            "application/yaml",
        )
        .await;

    assert_job_summary(&response, "twerk-pokemon-shell-stress", "PENDING");
}

#[tokio::test]
async fn post_jobs_100_tasks_job_can_be_retrieved_by_id() {
    let harness = TestHarness::new().await;
    let create_response = harness
        .post_yaml("/jobs", TWERK_NOOP_100_YAML.to_string(), "application/yaml")
        .await;
    let job_id = create_response.json()["id"].as_str().expect("job id");
    let get_response = harness.get(&format!("/jobs/{job_id}")).await;

    assert_job_summary(&create_response, "twerk-noop-stress", "PENDING");
    get_response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(get_response.json()["name"], "twerk-noop-stress");
    assert_task_slice(get_response.json(), "noop-001", "noop-100");
}

#[tokio::test]
async fn post_jobs_twerk_noop_100_yaml_task_count_reflects_yaml_value() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml("/jobs", TWERK_NOOP_100_YAML.to_string(), "application/yaml")
        .await;

    assert_eq!(response.json()["taskCount"].as_i64().unwrap(), 100);
}

#[tokio::test]
async fn post_jobs_twerk_pokemon_shell_100_yaml_task_count_reflects_yaml_value() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml(
            "/jobs",
            TWERK_POKEMON_SHELL_100_YAML.to_string(),
            "application/yaml",
        )
        .await;

    assert_eq!(response.json()["taskCount"].as_i64().unwrap(), 100);
}

#[tokio::test]
async fn parse_yaml_with_100_tasks_completes_without_error() {
    let job = parse_noop_job();

    assert!(job.tasks.is_some());
    assert_eq!(job.tasks.unwrap().len(), 100);
}
