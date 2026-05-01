use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use tracing_subscriber::fmt::MakeWriter;
use twerk_app::engine::coordinator::middleware::{http_log_middleware, HttpLogConfig};

struct TestWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl TestWriter {
    fn new() -> (Self, Arc<Mutex<Vec<u8>>>) {
        let buf = Arc::new(Mutex::new(Vec::new()));
        (Self { buf: buf.clone() }, buf)
    }
}

impl<'a> MakeWriter<'a> for TestWriter {
    type Writer = &'a Arc<Mutex<Vec<u8>>>;

    fn make_writer(&'a self) -> Self::Writer {
        &self.buf
    }
}

async fn test_handler() -> axum::response::Json<&'static str> {
    axum::response::Json("OK")
}

async fn not_found_handler() -> StatusCode {
    StatusCode::NOT_FOUND
}

fn build_app_with_logging(config: HttpLogConfig, buf: Arc<Mutex<Vec<u8>>>) -> Router {
    let writer = TestWriter { buf };
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(writer)
        .try_init();

    Router::new()
        .route("/api/health", get(test_handler))
        .route("/nonexistent", get(not_found_handler))
        .layer(axum::middleware::from_fn_with_state(
            config,
            |st, req, next| Box::pin(async move { http_log_middleware(st, req, next).await }),
        ))
}

fn get_captured_logs(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    let guard = buf.lock().unwrap();
    String::from_utf8_lossy(&guard).to_string()
}

#[tokio::test]
async fn http_log_middleware_captures_latency_for_health_endpoint() {
    let (writer, buf) = TestWriter::new();
    let config = HttpLogConfig::default();
    let app = build_app_with_logging(config, buf);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let logs = get_captured_logs(&buf);
    assert!(
        logs.contains("method=GET"),
        "Log should contain method=GET, got: {}",
        logs
    );
    assert!(
        logs.contains("path=/api/health"),
        "Log should contain path=/api/health, got: {}",
        logs
    );
    assert!(
        logs.contains("status=200"),
        "Log should contain status=200, got: {}",
        logs
    );
    assert!(
        logs.contains("duration_ms="),
        "Log should contain duration_ms=, got: {}",
        logs
    );
}

#[tokio::test]
async fn http_log_middleware_records_positive_duration() {
    let (writer, buf) = TestWriter::new();
    let config = HttpLogConfig::default();
    let app = build_app_with_logging(config, buf);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let logs = get_captured_logs(&buf);
    let duration_pattern = "duration_ms=";
    if let Some(pos) = logs.find(duration_pattern) {
        let after_duration = &logs[pos + duration_pattern.len()..];
        let duration_str: String = after_duration.chars().take_while(|c| c.is_ascii_digit()).collect();
        let duration_ms: u64 = duration_str.parse().unwrap_or(0);
        assert!(
            duration_ms > 0,
            "duration_ms should be > 0, got {}",
            duration_ms
        );
    } else {
        panic!("duration_ms not found in logs: {}", logs);
    }
}

#[tokio::test]
async fn http_log_middleware_logs_404_requests() {
    let (writer, buf) = TestWriter::new();
    let config = HttpLogConfig::default();
    let app = build_app_with_logging(config, buf);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let logs = get_captured_logs(&buf);
    assert!(
        logs.contains("method=GET"),
        "Log should contain method=GET for 404, got: {}",
        logs
    );
    assert!(
        logs.contains("path=/nonexistent"),
        "Log should contain path=/nonexistent for 404, got: {}",
        logs
    );
    assert!(
        logs.contains("status=404"),
        "Log should contain status=404, got: {}",
        logs
    );
    assert!(
        logs.contains("duration_ms="),
        "Log should contain duration_ms= for 404, got: {}",
        logs
    );
}

#[tokio::test]
async fn http_log_middleware_skips_health_endpoint_by_default() {
    let (writer, buf) = TestWriter::new();
    let config = HttpLogConfig::default();
    let app = build_app_with_logging(config, buf);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let logs = get_captured_logs(&buf);
    assert!(
        !logs.contains("http_request"),
        "GET /health should be skipped by default, got: {}",
        logs
    );
}