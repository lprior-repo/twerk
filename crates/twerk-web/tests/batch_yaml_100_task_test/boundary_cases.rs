use axum::http::StatusCode;
use twerk_core::task::TaskState;
use twerk_web::api::yaml::from_slice;

use crate::support::{assert_json_message, TestHarness};

const TWERK_NOOP_100_YAML: &str = include_str!("../../../../examples/twerk-noop-100.yaml");

#[tokio::test]
async fn post_jobs_rejects_yaml_with_empty_body() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml("/jobs", axum::body::Body::empty(), "application/yaml")
        .await;

    response
        .assert_status(StatusCode::BAD_REQUEST)
        .assert_json_content_type();
    assert!(response.json()["message"]
        .as_str()
        .is_some_and(|message| !message.is_empty()));
}

#[tokio::test]
async fn post_jobs_rejects_unsupported_content_type() {
    let harness = TestHarness::new().await;
    let response = harness
        .post_yaml("/jobs", "some content", "text/plain")
        .await;

    assert_json_message(
        &response,
        StatusCode::BAD_REQUEST,
        "unsupported content type",
    );
}

#[tokio::test]
async fn parse_yaml_with_100_tasks_all_tasks_have_valid_structure() {
    let job = from_slice::<twerk_core::job::Job>(TWERK_NOOP_100_YAML.as_bytes()).unwrap();

    job.tasks
        .unwrap()
        .iter()
        .enumerate()
        .for_each(|(index, task)| {
            assert!(task.name.is_some(), "Task {index} missing name");
            assert!(task.run.is_some(), "Task {index} missing run command");
            assert_eq!(
                task.state,
                TaskState::Created,
                "Task {index} wrong default state"
            );
        });
}
