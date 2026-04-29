use axum::http::StatusCode;
use tokio::task::JoinSet;

use super::black_box_support::{assert_health_up, assert_metrics, node, TestHarness, TestResponse};

async fn spawn_concurrent_requests(
    harness: &TestHarness,
    uri: &'static str,
    count: usize,
) -> Vec<TestResponse> {
    let mut set = JoinSet::new();

    (0..count).for_each(|_| {
        let harness = harness.clone();
        set.spawn(async move { harness.get(uri).await });
    });

    set.join_all().await
}

#[tokio::test]
async fn health_handles_concurrent_requests() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;
    let results = spawn_concurrent_requests(&harness, "/health", 50).await;

    results.into_iter().for_each(|response| {
        assert_health_up(&response);
    });
}

#[tokio::test]
async fn metrics_handles_concurrent_requests() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;
    let results = spawn_concurrent_requests(&harness, "/metrics", 50).await;

    results.into_iter().for_each(|response| {
        assert_metrics(&response, 0, 0, 1, 0.0);
    });
}

#[tokio::test]
async fn nodes_handles_concurrent_requests() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;
    let results = spawn_concurrent_requests(&harness, "/nodes", 50).await;

    results.into_iter().for_each(|response| {
        response
            .assert_status(StatusCode::OK)
            .assert_json_content_type();
        assert_eq!(response.json().as_array().unwrap().len(), 1);
    });
}

#[tokio::test]
async fn all_three_endpoints_handle_concurrent_mixed_requests() {
    let harness = TestHarness::new().await;

    harness.seed_node(&node("test-node-1", "worker-1")).await;
    let health = spawn_concurrent_requests(&harness, "/health", 25).await;
    let metrics = spawn_concurrent_requests(&harness, "/metrics", 25).await;
    let nodes = spawn_concurrent_requests(&harness, "/nodes", 25).await;

    let health_count = health
        .into_iter()
        .map(|response| {
            assert_eq!(response.status(), StatusCode::OK);
            usize::from(
                response.json().get("status").is_some() && response.json().get("version").is_some(),
            )
        })
        .sum::<usize>();
    let metrics_count = metrics
        .into_iter()
        .map(|response| {
            assert_eq!(response.status(), StatusCode::OK);
            usize::from(
                response.json()
                    == &serde_json::json!({
                        "jobs": {"running": 0},
                        "tasks": {"running": 0},
                        "nodes": {"online": 1, "cpuPercent": 0.0}
                    }),
            )
        })
        .sum::<usize>();
    let nodes_count = nodes
        .into_iter()
        .map(|response| {
            assert_eq!(response.status(), StatusCode::OK);
            usize::from(
                response
                    .json()
                    .as_array()
                    .is_some_and(|nodes| nodes.len() == 1),
            )
        })
        .sum::<usize>();

    assert_eq!(health_count, 25);
    assert_eq!(metrics_count, 25);
    assert_eq!(nodes_count, 25);
}

#[tokio::test]
async fn high_concurrency_requests_preserve_contracts() {
    let harness = TestHarness::new().await;
    harness.seed_node(&node("test-node-1", "worker-1")).await;

    let health = spawn_concurrent_requests(&harness, "/health", 200).await;
    let metrics = spawn_concurrent_requests(&harness, "/metrics", 200).await;
    let nodes = spawn_concurrent_requests(&harness, "/nodes", 200).await;

    health
        .into_iter()
        .for_each(|response| assert_health_up(&response));
    metrics
        .into_iter()
        .for_each(|response| assert_metrics(&response, 0, 0, 1, 0.0));
    nodes.into_iter().for_each(|response| {
        response
            .assert_status(StatusCode::OK)
            .assert_json_content_type();
        assert_eq!(response.json().as_array().unwrap().len(), 1);
    });
}
