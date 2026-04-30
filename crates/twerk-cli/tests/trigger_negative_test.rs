//! D2 — Missing Trigger Negative Tests (B11–B30)
//!
//! Tests for trigger handlers receiving non-success HTTP responses.
//! These tests SHOULD PASS because trigger.rs already reads response body
//! before checking status code.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::{routing, Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use twerk_cli::handlers::trigger::{
    trigger_create, trigger_delete, trigger_get, trigger_list, trigger_update,
};
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

// ---------------------------------------------------------------------------
// B11: trigger_list 500 with structured JSON
// ---------------------------------------------------------------------------

async fn b11_internal_error_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "internal",
            "message": "database unavailable"
        })),
    )
}

#[tokio::test]
async fn trigger_list_returns_api_error_when_server_returns_500_with_structured_json() {
    let router = Router::new().route("/api/v1/triggers", routing::get(b11_internal_error_json));
    let server = spawn_router(router).await;

    let result = trigger_list(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 500, ref message }) if message == "database unavailable"),
        "expected Err(ApiError {{ code: 500, message: \"database unavailable\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B12: trigger_list 500 with non-JSON body
// ---------------------------------------------------------------------------

async fn b12_plain_text_500() -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "Gateway Timeout")
}

#[tokio::test]
async fn trigger_list_returns_http_status_when_server_returns_500_with_non_json_body() {
    let router = Router::new().route("/api/v1/triggers", routing::get(b12_plain_text_500));
    let server = spawn_router(router).await;

    let result = trigger_list(&server.endpoint, true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 500, ref reason }) if reason == "Internal Server Error"),
        "expected Err(HttpStatus {{ status: 500, reason: \"Internal Server Error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B13: trigger_get 404 with structured JSON
// ---------------------------------------------------------------------------

async fn b13_not_found_json(Path(_id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "trigger 'nonexistent' not found"
        })),
    )
}

#[tokio::test]
async fn trigger_get_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b13_not_found_json));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, "nonexistent", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "trigger 'nonexistent' not found"),
        "expected Err(ApiError {{ code: 404, message: \"trigger 'nonexistent' not found\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B14: trigger_get 404 with non-JSON body
// ---------------------------------------------------------------------------

async fn b14_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn trigger_get_returns_not_found_when_server_returns_404_with_non_json_body() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b14_not_found_plain));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, "nonexistent", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "trigger nonexistent not found"),
        "expected Err(NotFound(\"trigger nonexistent not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B15: trigger_get 400 with structured JSON
// ---------------------------------------------------------------------------

async fn b15_bad_request_json(Path(_id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "bad_request",
            "message": "invalid trigger ID format"
        })),
    )
}

#[tokio::test]
async fn trigger_get_returns_api_error_when_server_returns_400_with_structured_json() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b15_bad_request_json));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, "ab", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 400, ref message }) if message == "invalid trigger ID format"),
        "expected Err(ApiError {{ code: 400, message: \"invalid trigger ID format\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B16: trigger_create 400 with structured JSON
// ---------------------------------------------------------------------------

async fn b16_create_bad_request_json() -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "bad_request",
            "message": "invalid JSON payload"
        })),
    )
}

#[tokio::test]
async fn trigger_create_returns_api_error_when_server_returns_400_with_structured_json() {
    let router = Router::new().route("/api/v1/triggers", routing::post(b16_create_bad_request_json));
    let server = spawn_router(router).await;

    let result = trigger_create(&server.endpoint, r#"{"name":"bad"}"#, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 400, ref message }) if message == "invalid JSON payload"),
        "expected Err(ApiError {{ code: 400, message: \"invalid JSON payload\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B17: trigger_update 409 with structured JSON
// ---------------------------------------------------------------------------

async fn b17_conflict_json(Path(_id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::CONFLICT,
        Json(json!({
            "error": "conflict",
            "message": "version mismatch"
        })),
    )
}

#[tokio::test]
async fn trigger_update_returns_api_error_when_server_returns_409_with_structured_json() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::put(b17_conflict_json));
    let server = spawn_router(router).await;

    let result = trigger_update(&server.endpoint, "trg1", "{}", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 409, ref message }) if message == "version mismatch"),
        "expected Err(ApiError {{ code: 409, message: \"version mismatch\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B18: trigger_update 404 with structured JSON
// ---------------------------------------------------------------------------

async fn b18_update_not_found_json(Path(_id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "trigger 'gone' not found"
        })),
    )
}

#[tokio::test]
async fn trigger_update_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::put(b18_update_not_found_json));
    let server = spawn_router(router).await;

    let result = trigger_update(&server.endpoint, "gone", "{}", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "trigger 'gone' not found"),
        "expected Err(ApiError {{ code: 404, message: \"trigger 'gone' not found\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B19: trigger_delete 404 with structured JSON
// ---------------------------------------------------------------------------

async fn b19_delete_not_found_json(Path(_id): Path<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "trigger 'gone' not found"
        })),
    )
}

#[tokio::test]
async fn trigger_delete_returns_api_error_when_server_returns_404_with_structured_json() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::delete(b19_delete_not_found_json));
    let server = spawn_router(router).await;

    let result = trigger_delete(&server.endpoint, "gone", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 404, ref message }) if message == "trigger 'gone' not found"),
        "expected Err(ApiError {{ code: 404, message: \"trigger 'gone' not found\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B20: trigger_delete 404 with non-JSON body
// ---------------------------------------------------------------------------

async fn b20_delete_not_found_plain() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

#[tokio::test]
async fn trigger_delete_returns_not_found_when_server_returns_404_with_non_json_body() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::delete(b20_delete_not_found_plain));
    let server = spawn_router(router).await;

    let result = trigger_delete(&server.endpoint, "gone", true).await;

    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "trigger gone not found"),
        "expected Err(NotFound(\"trigger gone not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B21: TriggerId 2 chars (below min) → server returns 400
// ---------------------------------------------------------------------------

async fn b21_id_too_short(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    assert_eq!(id.len(), 2, "expected 2-char id");
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "validation",
            "message": "trigger id must be at least 3 characters"
        })),
    )
}

#[tokio::test]
async fn trigger_get_returns_api_error_400_when_trigger_id_has_2_chars_below_minimum() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b21_id_too_short));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, "ab", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 400, ref message }) if message == "trigger id must be at least 3 characters"),
        "expected Err(ApiError {{ code: 400, message: \"trigger id must be at least 3 characters\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B22: TriggerId 3 chars (at min) → server returns 200 with valid trigger
// ---------------------------------------------------------------------------

fn make_valid_trigger_json(id: &str) -> String {
    serde_json::json!({
        "id": id,
        "name": format!("trigger-{id}"),
        "enabled": true,
        "event": "order.created",
        "condition": null,
        "action": "notify",
        "metadata": {},
        "version": 1,
        "created_at": "2026-04-22T00:00:00Z",
        "updated_at": "2026-04-22T00:00:00Z"
    })
    .to_string()
}

async fn b22_id_at_min(Path(id): Path<String>) -> (StatusCode, String) {
    assert_eq!(id.len(), 3, "expected 3-char id");
    (StatusCode::OK, make_valid_trigger_json(&id))
}

#[tokio::test]
async fn trigger_get_succeeds_when_trigger_id_has_3_chars_at_minimum_boundary() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b22_id_at_min));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, "abc", true).await;

    let body = result.expect("trigger_get should succeed for 3-char id");
    assert!(
        body.contains("abc"),
        "expected response body to contain \"abc\", got: {}",
        body
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B23: TriggerId 65 chars (above max) → server returns 400
// ---------------------------------------------------------------------------

async fn b23_id_too_long(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    assert_eq!(id.len(), 65, "expected 65-char id");
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "validation",
            "message": "trigger id must be at most 64 characters"
        })),
    )
}

#[tokio::test]
async fn trigger_get_returns_api_error_400_when_trigger_id_has_65_chars_above_maximum() {
    let id_65 = "a".repeat(65);
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b23_id_too_long));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, &id_65, true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 400, ref message }) if message == "trigger id must be at most 64 characters"),
        "expected Err(ApiError {{ code: 400, message: \"trigger id must be at most 64 characters\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B24: TriggerId 64 chars (at max) → server returns 200
// ---------------------------------------------------------------------------

async fn b24_id_at_max(Path(id): Path<String>) -> (StatusCode, String) {
    assert_eq!(id.len(), 64, "expected 64-char id");
    (StatusCode::OK, make_valid_trigger_json(&id))
}

#[tokio::test]
async fn trigger_get_succeeds_when_trigger_id_has_64_chars_at_maximum_boundary() {
    let id_64 = "a".repeat(64);
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b24_id_at_max));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, &id_64, true).await;

    let body = result.expect("trigger_get should succeed for 64-char id");
    assert!(
        body.contains(&id_64),
        "expected response body to contain the 64-char id, got: {}",
        body
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B25: TriggerId with special characters → server returns 400
// ---------------------------------------------------------------------------

async fn b25_invalid_charset(Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    assert!(id.contains('!'), "expected special char in id");
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "validation",
            "message": "trigger id contains invalid characters"
        })),
    )
}

#[tokio::test]
async fn trigger_get_returns_api_error_400_when_trigger_id_contains_special_characters() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b25_invalid_charset));
    let server = spawn_router(router).await;

    // "bad!chars" - axum will URL-decode percent-encoded chars in path
    let result = trigger_get(&server.endpoint, "bad!chars", true).await;

    assert!(
        matches!(result, Err(CliError::ApiError { code: 400, ref message }) if message == "trigger id contains invalid characters"),
        "expected Err(ApiError {{ code: 400, message: \"trigger id contains invalid characters\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B25.5: TriggerId empty string → no axum route match → 404 → NotFound
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trigger_get_returns_not_found_when_trigger_id_is_empty_string() {
    // Empty string "" passed as id; the URL becomes /api/v1/triggers/
    // Axum doesn't match this against /api/v1/triggers/{id}, returns a default 404.
    // trigger.rs sees 404, can't parse body as TriggerErrorResponse, returns NotFound.
    // We set up a route at /api/v1/triggers/{id} with a plain-text 404 handler
    // to simulate the server's response to a malformed path.
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b14_not_found_plain));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, "", true).await;

    // With empty id, the URL is /api/v1/triggers/ which axum doesn't route to {id}.
    // reqwest GETs /api/v1/triggers/ and gets 404 from axum's default handler.
    // trigger.rs: status == NOT_FOUND, body not parseable as TriggerErrorResponse
    // → returns NotFound("trigger  not found")
    assert!(
        matches!(result, Err(CliError::NotFound(ref msg)) if msg == "trigger  not found"),
        "expected Err(NotFound(\"trigger  not found\")), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B26: trigger_get non-JSON non-404 error (503)
// ---------------------------------------------------------------------------

async fn b26_service_unavailable_plain() -> (StatusCode, &'static str) {
    (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")
}

#[tokio::test]
async fn trigger_get_returns_http_status_when_server_returns_non_json_error() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::get(b26_service_unavailable_plain));
    let server = spawn_router(router).await;

    let result = trigger_get(&server.endpoint, "test", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 503, ref reason }) if reason == "Service Unavailable"),
        "expected Err(HttpStatus {{ status: 503, reason: \"Service Unavailable\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B27: trigger_create non-JSON non-400 error (500)
// ---------------------------------------------------------------------------

async fn b27_create_internal_error_plain() -> (StatusCode, &'static str) {
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
}

#[tokio::test]
async fn trigger_create_returns_http_status_when_server_returns_non_json_error() {
    let router = Router::new().route("/api/v1/triggers", routing::post(b27_create_internal_error_plain));
    let server = spawn_router(router).await;

    let result = trigger_create(&server.endpoint, "{}", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 500, ref reason }) if reason == "Internal Server Error"),
        "expected Err(HttpStatus {{ status: 500, reason: \"Internal Server Error\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B28: trigger_update non-JSON unrecognized status (418)
// ---------------------------------------------------------------------------

async fn b28_teapot_plain() -> (StatusCode, &'static str) {
    (StatusCode::from_u16(418).expect("418 is valid"), "I'm a teapot")
}

#[tokio::test]
async fn trigger_update_returns_http_status_when_server_returns_unrecognized_status() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::put(b28_teapot_plain));
    let server = spawn_router(router).await;

    let result = trigger_update(&server.endpoint, "test", "{}", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 418, ref reason }) if reason == "I'm a teapot"),
        "expected Err(HttpStatus {{ status: 418, reason: \"I'm a teapot\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B29: trigger_delete non-JSON non-404 non-204 error (502)
// ---------------------------------------------------------------------------

async fn b29_bad_gateway_plain() -> (StatusCode, &'static str) {
    (StatusCode::BAD_GATEWAY, "Bad Gateway")
}

#[tokio::test]
async fn trigger_delete_returns_http_status_when_server_returns_unrecognized_status() {
    let router = Router::new().route("/api/v1/triggers/{id}", routing::delete(b29_bad_gateway_plain));
    let server = spawn_router(router).await;

    let result = trigger_delete(&server.endpoint, "test", true).await;

    assert!(
        matches!(result, Err(CliError::HttpStatus { status: 502, ref reason }) if reason == "Bad Gateway"),
        "expected Err(HttpStatus {{ status: 502, reason: \"Bad Gateway\" }}), got {:?}",
        result
    );

    server.shutdown().await;
}

// ---------------------------------------------------------------------------
// B30: Mutation kill — verify TriggerErrorResponse parse path exists
// ---------------------------------------------------------------------------

#[test]
fn mutation_kill_trigger_error_response_parse_path_verified() {
    let source = include_str!("../src/handlers/trigger.rs");
    assert!(
        source.contains("from_str::<TriggerErrorResponse>"),
        "trigger.rs must contain the TriggerErrorResponse parse path; \
         removing it would cause B11–B20 to fail (returning HttpStatus instead of ApiError)"
    );
}
