use axum::http::StatusCode;

use crate::support::{assert_health_up, assert_metrics, assert_node_entry, node, TestHarness};

#[tokio::test]
async fn health_returns_200_with_status_up() {
    let harness = TestHarness::new().await;
    let response = harness.get("/health").await;

    assert_health_up(&response);
}

#[tokio::test]
async fn health_contract_has_exact_two_fields() {
    let harness = TestHarness::new().await;
    let response = harness.get("/health").await;

    assert_health_up(&response);
    assert_eq!(response.json().as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn metrics_are_zeroed_for_empty_state() {
    let harness = TestHarness::new().await;
    let response = harness.get("/metrics").await;

    assert_metrics(&response, 0, 0, 0, 0.0);
}

#[tokio::test]
async fn metrics_reflect_seeded_running_work_and_nodes() {
    let harness = TestHarness::new().await;
    harness
        .seed_job(&crate::support::job_with_state(
            "00000000-0000-0000-0000-000000000101",
            "running-job",
            twerk_core::job::JobState::Running,
        ))
        .await;
    harness
        .seed_task(&crate::support::direct_task(
            "00000000-0000-0000-0000-000000000101",
            "00000000-0000-0000-0000-000000000201",
            "running-task",
            twerk_core::task::TaskState::Running,
        ))
        .await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    let response = harness.get("/metrics").await;

    assert_metrics(&response, 1, 1, 1, 0.0);
}

#[tokio::test]
async fn nodes_returns_empty_array_for_empty_state() {
    let harness = TestHarness::new().await;
    let response = harness.get("/nodes").await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert!(response.json().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn nodes_return_seeded_node_projection() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    let response = harness.get("/nodes").await;
    let nodes = response.json().as_array().unwrap();

    assert_eq!(nodes.len(), 1);
    assert_node_entry(&nodes[0], "test-node-1", "worker-1", "UP");
}

#[tokio::test]
async fn multiple_health_requests_return_same_version() {
    let harness = TestHarness::new().await;
    let first = harness.get("/health").await;
    let second = harness.get("/health").await;

    assert_eq!(first.json()["version"], second.json()["version"]);
}

#[tokio::test]
async fn nodes_endpoint_does_not_return_null_elements() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;
    let response = harness.get("/nodes").await;

    response
        .json()
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .for_each(|(index, node)| assert!(!node.is_null(), "nodes[{index}] should not be null"));
}
