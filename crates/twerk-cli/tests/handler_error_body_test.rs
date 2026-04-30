//! D3 — Handler Error-Body Drop Tests (B31–B58)
//!
//! Tests that queue/task/node/metrics/user handlers:
//! 1. Read response body BEFORE checking status code
//! 2. Parse structured JSON error responses into CliError::ApiError
//! 3. Fall back to CliError::HttpStatus for non-JSON error bodies
//!
//! These tests MUST FAIL with the current code because handlers drop
//! error bodies — they check status code first and never read the body.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::{routing, Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use twerk_cli::handlers::metrics::metrics_get;
use twerk_cli::handlers::node::{node_get, node_list};
use twerk_cli::handlers::queue::{queue_delete, queue_get, queue_list};
use twerk_cli::handlers::task::{task_get, task_log};
use twerk_cli::handlers::user::user_create;
use twerk_cli::CliError;

// ---------------------------------------------------------------------------
// Test server infrastructure
// ---------------------------------------------------------------------------

struct HttpTestServer {
    endpoint: String,
    shutdown_tx: oneshot::Sender<()>,
}

impl HttpTestServer {
    async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

async fn spawn_router(router: Router) -> HttpTestServer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("serve test router");
    });

    HttpTestServer {
        endpoint: format!("http://{addr}"),
        shutdown_tx,
    }
}

// ===========================================================================
// QUEUE LIST TESTS (B31, B32)
// ===========================================================================

// ---------------------------------------------------------------------------
// B31: queue_list 500 structured JSON
// ---------------------------------------------------------------------------

async fn b31_queue_list_internal_error() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "database connection lost"
        })),
    )
}

#[tokio::test]
async fn queue_list_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route("/queues", routing::get(b31_queue_list_internal_error));
    let server = spawn_router(router).await;

    let result = queue_list(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "database connection lost"),
        "expected Err(ApiError {{ code: 500, message: \"database connection lost\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B32: queue_list 500 non-JSON
// ---------------------------------------------------------------------------

async fn b32_queue_list_plain_error() -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "not json")
}

#[tokio::test]
async fn queue_list_returns_http_status_when_server_returns_500_with_non_json_body() {
    let router = Router::new().route("/queues", routing::get(b32_queue_list_plain_error));
    let server = spawn_router(router).await;

    let result = queue_list(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 500, ref reason }) if reason == "Internal Server Error"),
        "expected Err(HttpStatus {{ status: 500, reason: \"Internal Server Error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// QUEUE GET TESTS (B33–B36)
// ===========================================================================

// ---------------------------------------------------------------------------
// B33: queue_get 404 structured JSON
// ---------------------------------------------------------------------------

async fn b33_queue_get_not_found_json(Path(name): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": format!("queue '{}' does not exist", name)
        })),
    )
}

#[tokio::test]
async fn queue_get_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route("/queues/{name}", routing::get(b33_queue_get_not_found_json));
    let server = spawn_router(router).await;

    let result = queue_get(&server.endpoint, "nonexistent", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "queue 'nonexistent' does not exist"),
        "expected Err(ApiError {{ code: 404, message: \"queue 'nonexistent' does not exist\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B34: queue_get 404 non-JSON
// ---------------------------------------------------------------------------

async fn b34_queue_get_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn queue_get_returns_not_found_when_server_returns_404_with_non_json_body() {
    let router = Router::new().route("/queues/{name}", routing::get(b34_queue_get_not_found_plain));
    let server = spawn_router(router).await;

    let result = queue_get(&server.endpoint, "nonexistent", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "queue nonexistent not found"),
        "expected Err(NotFound(\"queue nonexistent not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B35: queue_get 500 structured JSON
// ---------------------------------------------------------------------------

async fn b35_queue_get_internal_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "internal error"
        })),
    )
}

#[tokio::test]
async fn queue_get_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route("/queues/{name}", routing::get(b35_queue_get_internal_json));
    let server = spawn_router(router).await;

    let result = queue_get(&server.endpoint, "broken", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "internal error"),
        "expected Err(ApiError {{ code: 500, message: \"internal error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B35.5: queue_get empty name boundary
// ---------------------------------------------------------------------------

async fn b35_5_queue_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn queue_get_returns_error_when_name_is_empty_string() {
    let router = Router::new().route("/queues", routing::get(b35_5_queue_not_found_plain));
    let server = spawn_router(router).await;

    let result = queue_get(&server.endpoint, "", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "queue  not found"),
        "expected Err(NotFound(\"queue  not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B36: queue_get non-JSON non-404 error (503)
// ---------------------------------------------------------------------------

async fn b36_queue_get_service_unavailable() -> (StatusCode, &'static str) {
    (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")
}

#[tokio::test]
async fn queue_get_returns_http_status_when_server_returns_non_json_non_404_error() {
    let router = Router::new().route("/queues/{name}", routing::get(b36_queue_get_service_unavailable));
    let server = spawn_router(router).await;

    let result = queue_get(&server.endpoint, "broken", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 503, ref reason }) if reason == "Service Unavailable"),
        "expected Err(HttpStatus {{ status: 503, reason: \"Service Unavailable\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// QUEUE DELETE TESTS (B37–B40)
// ===========================================================================

// ---------------------------------------------------------------------------
// B37: queue_delete 404 structured JSON
// ---------------------------------------------------------------------------

async fn b37_queue_delete_not_found_json(Path(name): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": format!("queue '{}' does not exist", name)
        })),
    )
}

#[tokio::test]
async fn queue_delete_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route("/queues/{name}", routing::delete(b37_queue_delete_not_found_json));
    let server = spawn_router(router).await;

    let result = queue_delete(&server.endpoint, "gone", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "queue 'gone' does not exist"),
        "expected Err(ApiError {{ code: 404, message: \"queue 'gone' does not exist\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B38: queue_delete 404 non-JSON
// ---------------------------------------------------------------------------

async fn b38_queue_delete_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn queue_delete_returns_not_found_when_server_returns_404_with_non_json_body() {
    let router = Router::new().route("/queues/{name}", routing::delete(b38_queue_delete_not_found_plain));
    let server = spawn_router(router).await;

    let result = queue_delete(&server.endpoint, "gone", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "queue gone not found"),
        "expected Err(NotFound(\"queue gone not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B39: queue_delete 500 structured JSON
// ---------------------------------------------------------------------------

async fn b39_queue_delete_internal_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "server error"
        })),
    )
}

#[tokio::test]
async fn queue_delete_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route("/queues/{name}", routing::delete(b39_queue_delete_internal_json));
    let server = spawn_router(router).await;

    let result = queue_delete(&server.endpoint, "broken", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "server error"),
        "expected Err(ApiError {{ code: 500, message: \"server error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B39.5: queue_delete empty name boundary
// ---------------------------------------------------------------------------

async fn b39_5_queue_delete_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn queue_delete_returns_error_when_name_is_empty_string() {
    let router = Router::new().route("/queues", routing::delete(b39_5_queue_delete_not_found_plain));
    let server = spawn_router(router).await;

    let result = queue_delete(&server.endpoint, "", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "queue  not found"),
        "expected Err(NotFound(\"queue  not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B40: queue_delete non-JSON non-404 error (502)
// ---------------------------------------------------------------------------

async fn b40_queue_delete_bad_gateway() -> (StatusCode, &'static str) {
    (StatusCode::BAD_GATEWAY, "Bad Gateway")
}

#[tokio::test]
async fn queue_delete_returns_http_status_when_server_returns_non_json_non_404_error() {
    let router = Router::new().route("/queues/{name}", routing::delete(b40_queue_delete_bad_gateway));
    let server = spawn_router(router).await;

    let result = queue_delete(&server.endpoint, "broken", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 502, ref reason }) if reason == "Bad Gateway"),
        "expected Err(HttpStatus {{ status: 502, reason: \"Bad Gateway\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// TASK GET TESTS (B41–B44)
// ===========================================================================

// ---------------------------------------------------------------------------
// B41: task_get 404 structured JSON
// ---------------------------------------------------------------------------

async fn b41_task_get_not_found_json(Path(task_id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": format!("task '{}' not found", task_id)
        })),
    )
}

#[tokio::test]
async fn task_get_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route("/tasks/{task_id}", routing::get(b41_task_get_not_found_json));
    let server = spawn_router(router).await;

    let result = task_get(&server.endpoint, "abc-123", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "task 'abc-123' not found"),
        "expected Err(ApiError {{ code: 404, message: \"task 'abc-123' not found\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B42: task_get 404 non-JSON
// ---------------------------------------------------------------------------

async fn b42_task_get_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn task_get_returns_not_found_when_server_returns_404_with_non_json_body() {
    let router = Router::new().route("/tasks/{task_id}", routing::get(b42_task_get_not_found_plain));
    let server = spawn_router(router).await;

    let result = task_get(&server.endpoint, "missing", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "task missing not found"),
        "expected Err(NotFound(\"task missing not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B43: task_get 500 structured JSON
// ---------------------------------------------------------------------------

async fn b43_task_get_internal_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "database connection lost"
        })),
    )
}

#[tokio::test]
async fn task_get_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route("/tasks/{task_id}", routing::get(b43_task_get_internal_json));
    let server = spawn_router(router).await;

    let result = task_get(&server.endpoint, "broken", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "database connection lost"),
        "expected Err(ApiError {{ code: 500, message: \"database connection lost\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B43.5: task_get with empty task_id
// ---------------------------------------------------------------------------

async fn b43_5_task_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn task_get_returns_error_when_task_id_is_empty_string() {
    let router = Router::new().route("/tasks", routing::get(b43_5_task_not_found_plain));
    let server = spawn_router(router).await;

    let result = task_get(&server.endpoint, "", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "task  not found"),
        "expected Err(NotFound(\"task  not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B44: task_get non-JSON non-404 error (503)
// ---------------------------------------------------------------------------

async fn b44_task_get_service_unavailable() -> (StatusCode, &'static str) {
    (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")
}

#[tokio::test]
async fn task_get_returns_http_status_when_server_returns_non_json_non_404_error() {
    let router = Router::new().route("/tasks/{task_id}", routing::get(b44_task_get_service_unavailable));
    let server = spawn_router(router).await;

    let result = task_get(&server.endpoint, "stuck", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 503, ref reason }) if reason == "Service Unavailable"),
        "expected Err(HttpStatus {{ status: 503, reason: \"Service Unavailable\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// TASK LOG TESTS (B45–B48)
// ===========================================================================

// ---------------------------------------------------------------------------
// B45: task_log 404 structured JSON
// ---------------------------------------------------------------------------

async fn b45_task_log_not_found_json(Path(task_id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": format!("task '{}' not found", task_id)
        })),
    )
}

#[tokio::test]
async fn task_log_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route(
        "/tasks/{task_id}/log",
        routing::get(b45_task_log_not_found_json),
    );
    let server = spawn_router(router).await;

    let result = task_log(&server.endpoint, "gone", None, None, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "task 'gone' not found"),
        "expected Err(ApiError {{ code: 404, message: \"task 'gone' not found\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B46: task_log 404 non-JSON
// ---------------------------------------------------------------------------

async fn b46_task_log_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn task_log_returns_not_found_when_server_returns_404_with_non_json_body() {
    let router = Router::new().route(
        "/tasks/{task_id}/log",
        routing::get(b46_task_log_not_found_plain),
    );
    let server = spawn_router(router).await;

    let result = task_log(&server.endpoint, "missing", None, None, true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "task missing not found"),
        "expected Err(NotFound(\"task missing not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B47: task_log 500 structured JSON
// ---------------------------------------------------------------------------

async fn b47_task_log_internal_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "server error"
        })),
    )
}

#[tokio::test]
async fn task_log_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route(
        "/tasks/{task_id}/log",
        routing::get(b47_task_log_internal_json),
    );
    let server = spawn_router(router).await;

    let result = task_log(&server.endpoint, "broken", None, None, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "server error"),
        "expected Err(ApiError {{ code: 500, message: \"server error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B47.5: task_log with empty task_id
// ---------------------------------------------------------------------------

async fn b47_5_task_log_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn task_log_returns_error_when_task_id_is_empty_string() {
    let router = Router::new().route("/tasks/log", routing::get(b47_5_task_log_not_found_plain));
    let server = spawn_router(router).await;

    let result = task_log(&server.endpoint, "", None, None, true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "task  not found"),
        "expected Err(NotFound(\"task  not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B48: task_log non-JSON non-404 error (502)
// ---------------------------------------------------------------------------

async fn b48_task_log_bad_gateway() -> (StatusCode, &'static str) {
    (StatusCode::BAD_GATEWAY, "Bad Gateway")
}

#[tokio::test]
async fn task_log_returns_http_status_when_server_returns_non_json_non_404_error() {
    let router = Router::new().route(
        "/tasks/{task_id}/log",
        routing::get(b48_task_log_bad_gateway),
    );
    let server = spawn_router(router).await;

    let result = task_log(&server.endpoint, "broken", None, None, true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 502, ref reason }) if reason == "Bad Gateway"),
        "expected Err(HttpStatus {{ status: 502, reason: \"Bad Gateway\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// NODE HANDLER TESTS (node_list, node_get)
// ===========================================================================

// ---------------------------------------------------------------------------
// node_list 500 structured JSON — body drop verification
// ---------------------------------------------------------------------------

async fn node_list_internal_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "cluster unreachable"
        })),
    )
}

#[tokio::test]
async fn node_list_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route("/nodes", routing::get(node_list_internal_json));
    let server = spawn_router(router).await;

    let result = node_list(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "cluster unreachable"),
        "expected Err(ApiError {{ code: 500, message: \"cluster unreachable\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// node_list 500 non-JSON — body drop verification
// ---------------------------------------------------------------------------

async fn node_list_internal_plain() -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "not json")
}

#[tokio::test]
async fn node_list_returns_http_status_when_server_returns_500_with_non_json_body() {
    let router = Router::new().route("/nodes", routing::get(node_list_internal_plain));
    let server = spawn_router(router).await;

    let result = node_list(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 500, ref reason }) if reason == "Internal Server Error"),
        "expected Err(HttpStatus {{ status: 500, reason: \"Internal Server Error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// node_get 404 structured JSON — body drop verification
// ---------------------------------------------------------------------------

async fn node_get_not_found_json(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": format!("node '{}' not found", id)
        })),
    )
}

#[tokio::test]
async fn node_get_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route("/nodes/{id}", routing::get(node_get_not_found_json));
    let server = spawn_router(router).await;

    let result = node_get(&server.endpoint, "n1", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "node 'n1' not found"),
        "expected Err(ApiError {{ code: 404, message: \"node 'n1' not found\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// node_get 404 non-JSON — body drop verification
// ---------------------------------------------------------------------------

async fn node_get_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn node_get_returns_not_found_when_server_returns_404_with_non_json_body() {
    let router = Router::new().route("/nodes/{id}", routing::get(node_get_not_found_plain));
    let server = spawn_router(router).await;

    let result = node_get(&server.endpoint, "missing", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "node missing not found"),
        "expected Err(NotFound(\"node missing not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// METRICS HANDLER TEST
// ===========================================================================

// ---------------------------------------------------------------------------
// metrics_get 500 structured JSON — body drop verification
// ---------------------------------------------------------------------------

async fn metrics_get_internal_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "monitoring system down"
        })),
    )
}

#[tokio::test]
async fn metrics_get_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route("/metrics", routing::get(metrics_get_internal_json));
    let server = spawn_router(router).await;

    let result = metrics_get(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "monitoring system down"),
        "expected Err(ApiError {{ code: 500, message: \"monitoring system down\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// metrics_get 500 non-JSON — body drop verification
// ---------------------------------------------------------------------------

async fn metrics_get_internal_plain() -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "not json")
}

#[tokio::test]
async fn metrics_get_returns_http_status_when_server_returns_500_with_non_json_body() {
    let router = Router::new().route("/metrics", routing::get(metrics_get_internal_plain));
    let server = spawn_router(router).await;

    let result = metrics_get(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 500, ref reason }) if reason == "Internal Server Error"),
        "expected Err(HttpStatus {{ status: 500, reason: \"Internal Server Error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// USER HANDLER TEST
// ===========================================================================

// ---------------------------------------------------------------------------
// user_create 400 structured JSON — body drop verification
// ---------------------------------------------------------------------------

async fn user_create_bad_request_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "bad_request",
            "message": "username contains invalid characters"
        })),
    )
}

#[tokio::test]
async fn user_create_returns_api_error_when_server_returns_400_with_structured_json() {
    let router = Router::new().route("/users", routing::post(user_create_bad_request_json));
    let server = spawn_router(router).await;

    let result = user_create(&server.endpoint, "bad!user", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 400, ref message }) if message == "username contains invalid characters"),
        "expected Err(ApiError {{ code: 400, message: \"username contains invalid characters\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// user_create 400 non-JSON — body drop verification
// ---------------------------------------------------------------------------

async fn user_create_bad_request_plain() -> (StatusCode, &'static str) {
    (StatusCode::BAD_REQUEST, "Bad Request")
}

#[tokio::test]
async fn user_create_returns_http_status_when_server_returns_400_with_non_json_body() {
    let router = Router::new().route("/users", routing::post(user_create_bad_request_plain));
    let server = spawn_router(router).await;

    let result = user_create(&server.endpoint, "bad", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 400, ref reason }) if reason == "Bad Request"),
        "expected Err(HttpStatus {{ status: 400, reason: \"Bad Request\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ===========================================================================
// HAPPY PATH TESTS (B56–B58.3)
// ===========================================================================

// ---------------------------------------------------------------------------
// B56: queue_list returns Ok with body when server returns 200
// ---------------------------------------------------------------------------

async fn b56_queue_list_ok() -> Json<Value> {
    Json(json!([{
        "name": "q1",
        "size": 0,
        "subscribers": 0,
        "unacked": 0
    }]))
}

#[tokio::test]
async fn queue_list_returns_ok_with_body_when_server_returns_200() {
    let router = Router::new().route("/queues", routing::get(b56_queue_list_ok));
    let server = spawn_router(router).await;

    let result = queue_list(&server.endpoint, true).await;

    let body = result.expect("queue_list should succeed for 200 response");
    assert!(
        body.contains("q1"),
        "expected response body to contain \"q1\", got: {}",
        body
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B58.1: queue_list returns Ok with empty array when server returns 200 with []
// ---------------------------------------------------------------------------

async fn b58_1_queue_list_empty() -> Json<Value> {
    Json(json!([]))
}

#[tokio::test]
async fn queue_list_returns_ok_with_empty_array_when_server_returns_200_empty() {
    let router = Router::new().route("/queues", routing::get(b58_1_queue_list_empty));
    let server = spawn_router(router).await;

    let result = queue_list(&server.endpoint, true).await;

    let body = result.expect("queue_list should succeed for empty 200 response");
    assert_eq!(
        body, "[]",
        "expected response body to be \"[]\", got: {}",
        body
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B57: queue_get returns Ok with body when server returns 200
// ---------------------------------------------------------------------------

async fn b57_queue_get_ok() -> Json<Value> {
    Json(json!({
        "name": "q1",
        "size": 5,
        "subscribers": 2,
        "unacked": 1
    }))
}

#[tokio::test]
async fn queue_get_returns_ok_with_body_when_server_returns_200() {
    let router = Router::new().route("/queues/{name}", routing::get(b57_queue_get_ok));
    let server = spawn_router(router).await;

    let result = queue_get(&server.endpoint, "q1", true).await;

    let body = result.expect("queue_get should succeed for 200 response");
    assert!(
        body.contains("q1"),
        "expected response body to contain \"q1\", got: {}",
        body
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B58: queue_delete returns Ok with body when server returns 200
// ---------------------------------------------------------------------------

async fn b58_queue_delete_ok() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({"deleted": true, "name": "q1"})),
    )
}

#[tokio::test]
async fn queue_delete_returns_ok_with_body_when_server_returns_200() {
    let router = Router::new().route("/queues/{name}", routing::delete(b58_queue_delete_ok));
    let server = spawn_router(router).await;

    let result = queue_delete(&server.endpoint, "q1", true).await;

    let body = result.expect("queue_delete should succeed for 200 response");
    assert!(
        body.contains("q1"),
        "expected response body to contain \"q1\", got: {}",
        body
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B58.2: task_get returns Ok with body when server returns 200
// ---------------------------------------------------------------------------

async fn b58_2_task_get_ok() -> Json<Value> {
    Json(json!({
        "id": "t1",
        "status": "completed",
        "result": "ok"
    }))
}

#[tokio::test]
async fn task_get_returns_ok_with_body_when_server_returns_200() {
    let router = Router::new().route("/tasks/{task_id}", routing::get(b58_2_task_get_ok));
    let server = spawn_router(router).await;

    let result = task_get(&server.endpoint, "t1", true).await;

    let body = result.expect("task_get should succeed for 200 response");
    assert!(
        body.contains("t1"),
        "expected response body to contain \"t1\", got: {}",
        body
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B58.3: task_log returns Ok with body when server returns 200
// ---------------------------------------------------------------------------

async fn b58_3_task_log_ok() -> Json<Value> {
    Json(json!({
        "items": [
            {
                "id": "l1",
                "task_id": "t1",
                "number": 1,
                "contents": "task started",
                "created_at": "2026-01-01T00:00:00Z"
            },
            {
                "id": "l2",
                "task_id": "t1",
                "number": 2,
                "contents": "task completed",
                "created_at": "2026-01-01T00:01:00Z"
            }
        ],
        "number": 1,
        "size": 10,
        "total_pages": 1,
        "total_items": 2
    }))
}

#[tokio::test]
async fn task_log_returns_ok_with_body_when_server_returns_200() {
    let router = Router::new().route("/tasks/{task_id}/log", routing::get(b58_3_task_log_ok));
    let server = spawn_router(router).await;

    let result = task_log(&server.endpoint, "t1", None, None, true).await;

    let body = result.expect("task_log should succeed for 200 response");
    assert!(
        body.contains("task started"),
        "expected response body to contain \"task started\", got: {}",
        body
    );

    server.shutdown().await;
}
