use crate::support::{assert_health_up, assert_metrics, node, TestHarness};

#[tokio::test]
async fn repeated_health_requests_keep_exact_contract() {
    let harness = TestHarness::new().await;

    for _ in 0..100 {
        assert_health_up(&harness.get("/health").await);
    }
}

#[tokio::test]
async fn repeated_metrics_requests_keep_seeded_counts() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    for _ in 0..100 {
        assert_metrics(&harness.get("/metrics").await, 0, 0, 1, 0.0);
    }
}

#[tokio::test]
async fn repeated_nodes_requests_keep_seeded_node_projection() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    for _ in 0..100 {
        let response = harness.get("/nodes").await;
        response
            .assert_status(axum::http::StatusCode::OK)
            .assert_json_content_type();
        assert_eq!(response.json().as_array().unwrap()[0]["id"], "test-node-1");
        assert_eq!(response.json().as_array().unwrap()[0]["status"], "UP");
    }
}
