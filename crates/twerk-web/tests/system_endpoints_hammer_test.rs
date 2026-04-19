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

use axum::http::StatusCode;
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;
use twerk_web::api::{create_router, AppState, Config};

async fn setup_state() -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState::new(broker, ds, Config::default())
}

async fn body_to_json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

fn create_request(uri: &str) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap()
}

async fn spawn_request(
    app: &axum::Router,
    uri: &'static str,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(create_request(uri))
        .await
        .unwrap();
    let status = response.status();
    let body = body_to_json(response).await;
    (status, body)
}

async fn spawn_concurrent_requests(
    app: &axum::Router,
    uri: &'static str,
    count: usize,
) -> Vec<(StatusCode, Value)> {
    let mut handles = Vec::with_capacity(count);
    for _ in 0..count {
        let app = app.clone();
        handles.push(tokio::spawn(async move { spawn_request(&app, uri).await }));
    }
    let mut results = Vec::with_capacity(count);
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    results
}

// =============================================================================
// GET /health Tests
// =============================================================================

#[tokio::test]
async fn health_returns_200_with_status_up() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app.oneshot(create_request("/health")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn health_response_has_status_field() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/health").await;

    assert!(body.get("status").is_some(), "response must have 'status' field");
}

#[tokio::test]
async fn health_response_has_version_field() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/health").await;

    assert!(body.get("version").is_some(), "response must have 'version' field");
}

#[tokio::test]
async fn health_status_is_up_when_healthy() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/health").await;

    assert_eq!(body["status"], "UP", "healthy state must have status=UP");
}

#[tokio::test]
async fn health_returns_valid_json_schema() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/health").await;

    assert!(body.is_object(), "health response must be a JSON object");
    assert!(
        body["status"].is_string(),
        "status field must be a string"
    );
    assert!(
        body["version"].is_string(),
        "version field must be a string"
    );
}

#[tokio::test]
async fn health_endpoint_is_reachable() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn health_status_field_is_string() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/health").await;

    assert!(
        body["status"].is_string(),
        "status must be a string, got: {:?}",
        body["status"]
    );
}

#[tokio::test]
async fn health_version_field_is_string() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/health").await;

    assert!(
        body["version"].is_string(),
        "version must be a string, got: {:?}",
        body["version"]
    );
}

// =============================================================================
// GET /metrics Tests
// =============================================================================

#[tokio::test]
async fn metrics_returns_200() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app.oneshot(create_request("/metrics")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_response_is_valid_object() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/metrics").await;

    assert!(body.is_object(), "metrics response must be a JSON object");
}

#[tokio::test]
async fn metrics_response_contains_expected_fields() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/metrics").await;

    assert!(
        body.get("jobs").is_some(),
        "metrics must have 'jobs' field"
    );
    assert!(
        body.get("tasks").is_some(),
        "metrics must have 'tasks' field"
    );
    assert!(
        body.get("nodes").is_some(),
        "metrics must have 'nodes' field"
    );
}

#[tokio::test]
async fn metrics_endpoint_is_reachable() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/metrics")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_jobs_has_running_field() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/metrics").await;

    assert!(
        body["jobs"].get("running").is_some(),
        "metrics.jobs must have 'running' field"
    );
    assert!(
        body["jobs"]["running"].is_number(),
        "metrics.jobs.running must be a number"
    );
}

#[tokio::test]
async fn metrics_tasks_has_running_field() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/metrics").await;

    assert!(
        body["tasks"].get("running").is_some(),
        "metrics.tasks must have 'running' field"
    );
    assert!(
        body["tasks"]["running"].is_number(),
        "metrics.tasks.running must be a number"
    );
}

#[tokio::test]
async fn metrics_nodes_has_online_and_cpu_percent_fields() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/metrics").await;

    assert!(
        body["nodes"].get("online").is_some() || body["nodes"].get("running").is_some(),
        "metrics.nodes must have 'online' or 'running' field"
    );
    assert!(
        body["nodes"].get("cpuPercent").is_some() || body["nodes"].get("cpu_percent").is_some(),
        "metrics.nodes must have 'cpuPercent' or 'cpu_percent' field"
    );
}

// =============================================================================
// GET /nodes Tests
// =============================================================================

#[tokio::test]
async fn nodes_returns_200() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app.oneshot(create_request("/nodes")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn nodes_response_is_array() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/nodes").await;

    assert!(body.is_array(), "nodes response must be a JSON array");
}

#[tokio::test]
async fn nodes_response_array_contains_node_objects() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/nodes").await;

    for node in body.as_array().unwrap() {
        assert!(
            node.is_object(),
            "each node must be a JSON object, got: {:?}",
            node
        );
    }
}

#[tokio::test]
async fn nodes_response_contains_node_id_field() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/nodes").await;

    let nodes = body.as_array().unwrap();
    if !nodes.is_empty() {
        assert!(
            nodes[0].get("id").is_some() || nodes[0].get("nodeId").is_some(),
            "node objects should have id or nodeId field"
        );
    }
}

#[tokio::test]
async fn nodes_endpoint_is_reachable() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/nodes")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Concurrent Tests
// =============================================================================

#[tokio::test]
async fn health_handles_concurrent_requests() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 50;

    let results = spawn_concurrent_requests(&app, "/health", count).await;

    for (status, body) in results {
        assert_eq!(status, StatusCode::OK, "concurrent health requests should all return 200");
        assert_eq!(body["status"], "UP", "concurrent health should still be UP");
    }
}

#[tokio::test]
async fn health_concurrent_responses_are_consistent() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 20;

    let results = spawn_concurrent_requests(&app, "/health", count).await;

    for (_, body) in results {
        assert!(body.get("status").is_some(), "all concurrent responses must have status");
        assert!(body.get("version").is_some(), "all concurrent responses must have version");
    }
}

#[tokio::test]
async fn metrics_handles_concurrent_requests() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 50;

    let results = spawn_concurrent_requests(&app, "/metrics", count).await;

    for (status, body) in results {
        assert_eq!(status, StatusCode::OK, "concurrent metrics requests should all return 200");
        assert!(body.is_object(), "concurrent metrics should all return valid objects");
    }
}

#[tokio::test]
async fn metrics_concurrent_responses_are_consistent() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 20;

    let results = spawn_concurrent_requests(&app, "/metrics", count).await;

    for (_, body) in results {
        assert!(body.is_object(), "all concurrent metrics responses must be objects");
    }
}

#[tokio::test]
async fn nodes_handles_concurrent_requests() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 50;

    let results = spawn_concurrent_requests(&app, "/nodes", count).await;

    for (status, body) in results {
        assert_eq!(status, StatusCode::OK, "concurrent nodes requests should all return 200");
        assert!(body.is_array(), "concurrent nodes should all return arrays");
    }
}

#[tokio::test]
async fn nodes_concurrent_responses_are_consistent() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 20;

    let results = spawn_concurrent_requests(&app, "/nodes", count).await;

    for (_, body) in results {
        assert!(body.is_array(), "all concurrent nodes responses must be arrays");
    }
}

#[tokio::test]
async fn all_three_endpoints_handle_concurrent_mixed_requests() {
    let state = setup_state().await;
    let app = create_router(state);

    let mut handles = Vec::new();

    for _ in 0..25 {
        let app_clone = app.clone();
        handles.push(tokio::spawn(async move { spawn_request(&app_clone, "/health").await }));
    }
    for _ in 0..25 {
        let app_clone = app.clone();
        handles.push(tokio::spawn(async move { spawn_request(&app_clone, "/metrics").await }));
    }
    for _ in 0..25 {
        let app_clone = app.clone();
        handles.push(tokio::spawn(async move { spawn_request(&app_clone, "/nodes").await }));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    let mut health_count = 0;
    let mut metrics_count = 0;
    let mut nodes_count = 0;

    for (i, (status, body)) in results.into_iter().enumerate() {
        assert_eq!(status, StatusCode::OK, "mixed concurrent request {} should return 200", i);

        if body.is_array() {
            nodes_count += 1;
        } else if body.get("jobs").is_some() && body.get("tasks").is_some() {
            metrics_count += 1;
        } else if body.get("status").is_some() && body.get("version").is_some() {
            health_count += 1;
        }
    }

    assert_eq!(health_count, 25, "should have 25 health responses, got {}", health_count);
    assert_eq!(metrics_count, 25, "should have 25 metrics responses, got {}", metrics_count);
    assert_eq!(nodes_count, 25, "should have 25 nodes responses, got {}", nodes_count);
}

// =============================================================================
// Stress Tests
// =============================================================================

#[tokio::test]
async fn health_very_high_concurrency_stress() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 200;

    let start = std::time::Instant::now();
    let results = spawn_concurrent_requests(&app, "/health", count).await;
    let elapsed = start.elapsed();

    for (status, _) in results {
        assert_eq!(status, StatusCode::OK, "high concurrency health should still return 200");
    }

    println!("health {} requests completed in {:?}", count, elapsed);
}

#[tokio::test]
async fn metrics_very_high_concurrency_stress() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 200;

    let start = std::time::Instant::now();
    let results = spawn_concurrent_requests(&app, "/metrics", count).await;
    let elapsed = start.elapsed();

    for (status, _) in results {
        assert_eq!(status, StatusCode::OK, "high concurrency metrics should still return 200");
    }

    println!("metrics {} requests completed in {:?}", count, elapsed);
}

#[tokio::test]
async fn nodes_very_high_concurrency_stress() {
    let state = setup_state().await;
    let app = create_router(state);
    let count = 200;

    let start = std::time::Instant::now();
    let results = spawn_concurrent_requests(&app, "/nodes", count).await;
    let elapsed = start.elapsed();

    for (status, _) in results {
        assert_eq!(status, StatusCode::OK, "high concurrency nodes should still return 200");
    }

    println!("nodes {} requests completed in {:?}", count, elapsed);
}

// =============================================================================
// Rapid Fire Tests
// =============================================================================

#[tokio::test]
async fn health_rapid_fire_requests_all_succeed() {
    let state = setup_state().await;
    let app = create_router(state);

    for _ in 0..100 {
        let response = app.clone().oneshot(create_request("/health")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn metrics_rapid_fire_requests_all_succeed() {
    let state = setup_state().await;
    let app = create_router(state);

    for _ in 0..100 {
        let response = app.clone().oneshot(create_request("/metrics")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn nodes_rapid_fire_requests_all_succeed() {
    let state = setup_state().await;
    let app = create_router(state);

    for _ in 0..100 {
        let response = app.clone().oneshot(create_request("/nodes")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

// =============================================================================
// Performance Tests
// =============================================================================

#[tokio::test]
async fn health_no_blocking_between_requests() {
    let state = setup_state().await;
    let app = create_router(state);

    let mut last_time = std::time::Instant::now();
    for _ in 0..50 {
        let response = app.clone().oneshot(create_request("/health")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let now = std::time::Instant::now();
        assert!(
            now.duration_since(last_time) < Duration::from_millis(100),
            "each request should complete quickly"
        );
        last_time = now;
    }
}

#[tokio::test]
async fn health_response_time_is_reasonable() {
    let state = setup_state().await;
    let app = create_router(state);

    let mut times = Vec::new();
    for _ in 0..20 {
        let start = std::time::Instant::now();
        let response = app.clone().oneshot(create_request("/health")).await.unwrap();
        let elapsed = start.elapsed();
        assert_eq!(response.status(), StatusCode::OK);
        times.push(elapsed);
    }

    let avg = times.iter().sum::<Duration>() / times.len() as u32;
    assert!(
        avg < Duration::from_millis(50),
        "average health response time should be < 50ms, got {:?}",
        avg
    );
}

#[tokio::test]
async fn metrics_response_time_is_reasonable() {
    let state = setup_state().await;
    let app = create_router(state);

    let mut times = Vec::new();
    for _ in 0..20 {
        let start = std::time::Instant::now();
        let response = app.clone().oneshot(create_request("/metrics")).await.unwrap();
        let elapsed = start.elapsed();
        assert_eq!(response.status(), StatusCode::OK);
        times.push(elapsed);
    }

    let avg = times.iter().sum::<Duration>() / times.len() as u32;
    assert!(
        avg < Duration::from_millis(50),
        "average metrics response time should be < 50ms, got {:?}",
        avg
    );
}

#[tokio::test]
async fn nodes_response_time_is_reasonable() {
    let state = setup_state().await;
    let app = create_router(state);

    let mut times = Vec::new();
    for _ in 0..20 {
        let start = std::time::Instant::now();
        let response = app.clone().oneshot(create_request("/nodes")).await.unwrap();
        let elapsed = start.elapsed();
        assert_eq!(response.status(), StatusCode::OK);
        times.push(elapsed);
    }

    let avg = times.iter().sum::<Duration>() / times.len() as u32;
    assert!(
        avg < Duration::from_millis(50),
        "average nodes response time should be < 50ms, got {:?}",
        avg
    );
}

// =============================================================================
// Schema Validation Tests
// =============================================================================

#[tokio::test]
async fn health_schema_validation_all_fields_present() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/health").await;

    assert!(
        body.get("status").is_some() && body["status"].is_string(),
        "health response must have string 'status' field"
    );
    assert!(
        body.get("version").is_some() && body["version"].is_string(),
        "health response must have string 'version' field"
    );
}

#[tokio::test]
async fn metrics_schema_validation_structure() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/metrics").await;

    assert!(body.is_object(), "metrics must be an object");

    assert!(body.get("jobs").is_some() && body["jobs"].is_object(), "jobs must be an object");
    assert!(body.get("tasks").is_some() && body["tasks"].is_object(), "tasks must be an object");
    assert!(body.get("nodes").is_some() && body["nodes"].is_object(), "nodes must be an object");

    assert!(
        body["jobs"].get("running").is_some() && body["jobs"]["running"].is_i64(),
        "jobs.running must be an integer"
    );
    assert!(
        body["tasks"].get("running").is_some() && body["tasks"]["running"].is_i64(),
        "tasks.running must be an integer"
    );
}

#[tokio::test]
async fn nodes_schema_validation_empty_array_is_valid() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/nodes").await;

    assert!(body.is_array(), "nodes response must be an array");
}

#[tokio::test]
async fn nodes_schema_validation_non_empty_array_elements_are_objects() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());

    let node = twerk_core::node::Node {
        id: Some(twerk_core::id::NodeId::new("test-node-1").unwrap()),
        name: Some("test-node".to_string()),
        status: Some(twerk_core::node::NodeStatus::UP),
        ..Default::default()
    };
    ds.create_node(&node).await.unwrap();

    let app = create_router(state);
    let (_, body) = spawn_request(&app, "/nodes").await;

    let nodes = body.as_array().unwrap();
    assert!(!nodes.is_empty(), "nodes array should not be empty after adding a node");

    for node_val in nodes {
        assert!(node_val.is_object(), "each node must be an object");
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[tokio::test]
async fn health_multiple_requests_return_same_version() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body1) = spawn_request(&app, "/health").await;
    let (_, body2) = spawn_request(&app, "/health").await;

    assert_eq!(body1["version"], body2["version"], "version should be consistent");
}

#[tokio::test]
async fn metrics_running_counts_are_non_negative() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/metrics").await;

    let jobs_running = body["jobs"]["running"].as_i64().unwrap_or(-1);
    let tasks_running = body["tasks"]["running"].as_i64().unwrap_or(-1);

    assert!(jobs_running >= 0, "jobs.running should be non-negative");
    assert!(tasks_running >= 0, "tasks.running should be non-negative");
}

#[tokio::test]
async fn nodes_endpoint_does_not_return_null_elements() {
    let state = setup_state().await;
    let app = create_router(state);

    let (_, body) = spawn_request(&app, "/nodes").await;

    let nodes = body.as_array().unwrap();
    for (i, node) in nodes.iter().enumerate() {
        assert!(
            !node.is_null(),
            "nodes[{}] should not be null",
            i
        );
    }
}
