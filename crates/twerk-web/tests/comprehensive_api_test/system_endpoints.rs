use axum::http::StatusCode;

use super::super::support::{
    assert_health_up, assert_metrics, assert_node_entry, node, TestHarness,
};

#[tokio::test]
async fn health_returns_up_status() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;
    let response = harness.get("/health").await;

    assert_health_up(&response);
}

#[tokio::test]
async fn nodes_list_returns_seeded_node() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    let response = harness.get("/nodes").await;

    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert_eq!(response.json().as_array().unwrap().len(), 1);
    assert_node_entry(
        &response.json().as_array().unwrap()[0],
        "test-node-1",
        "worker-1",
        "UP",
    );
}

#[tokio::test]
async fn metrics_returns_zeroed_counts_for_empty_state() {
    let harness = TestHarness::new().await;

    let response = harness.get("/metrics").await;

    assert_metrics(&response, 0, 0, 0, 0.0);
}
