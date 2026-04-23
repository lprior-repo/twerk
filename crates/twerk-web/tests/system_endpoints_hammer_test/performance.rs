use super::black_box_support::{
    assert_health_up, assert_metrics, node, repeated_gets, TestHarness,
};

#[tokio::test]
async fn repeated_health_requests_keep_exact_contract() {
    let harness = TestHarness::new().await;

    repeated_gets(&harness, "/health", 100)
        .await
        .into_iter()
        .for_each(|response| assert_health_up(&response));
}

#[tokio::test]
async fn repeated_metrics_requests_keep_seeded_counts() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    repeated_gets(&harness, "/metrics", 100)
        .await
        .into_iter()
        .for_each(|response| assert_metrics(&response, 0, 0, 1, 0.0));
}

#[tokio::test]
async fn repeated_nodes_requests_keep_seeded_node_projection() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    repeated_gets(&harness, "/nodes", 100)
        .await
        .into_iter()
        .for_each(|response| {
            response
                .assert_status(axum::http::StatusCode::OK)
                .assert_json_content_type();
            assert_eq!(response.json().as_array().unwrap()[0]["id"], "test-node-1");
            assert_eq!(response.json().as_array().unwrap()[0]["status"], "UP");
        });
}
