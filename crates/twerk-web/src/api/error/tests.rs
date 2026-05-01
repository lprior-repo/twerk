use axum::body::to_bytes;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tracing_subscriber::prelude::*;

use super::super::trigger_api::TriggerUpdateError;
use super::ApiError;

const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

struct TestLogWriter {
    logs: Mutex<Vec<String>>,
}

impl TestLogWriter {
    fn new() -> Self {
        Self {
            logs: Mutex::new(Vec::new()),
        }
    }
    fn get_logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().clone()
    }
    fn clear(&self) {
        self.logs.lock().unwrap().clear();
    }
}

impl std::io::Write for TestLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let msg = String::from_utf8_lossy(buf).to_string();
        if !msg.trim().is_empty() {
            self.logs.lock().unwrap().push(msg);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn setup_log_capture() -> (Arc<TestLogWriter>, tracing::dispatcher::DefaultGuard) {
    let writer = Arc::new(TestLogWriter::new());
    let writer_clone = writer.clone();

    let subscriber = tracing_subscriber::fmt::fmt()
        .with_writer(move || TestLogWriterWrapper(writer_clone.clone()))
        .with_ansi(false)
        .finish();

    let dispatcher = tracing::dispatcher::Dispatch::new(subscriber);
    let guard = tracing::dispatcher::set_default(&dispatcher);

    (writer, guard)
}

struct TestLogWriterWrapper(Arc<TestLogWriter>);

impl std::io::Write for TestLogWriterWrapper {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.logs.lock().unwrap().push(String::from_utf8_lossy(buf).to_string());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn get_logs_containing(writer: &TestLogWriter, substr: &str) -> Vec<String> {
    writer
        .get_logs()
        .into_iter()
        .filter(|log| log.contains(substr))
        .collect()
}

async fn extract_response_body(response: Response) -> (String, String, StatusCode) {
    let status = response.status();
    let headers = HeaderMap::from_iter(response.headers().iter().map(|(k, v)| (k.clone(), v.clone())));
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    let body = String::from_utf8(bytes.to_vec()).unwrap_or_default();
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    (body, content_type, status)
}

fn parse_problem_body(body: &str) -> serde_json::Value {
    serde_json::from_str(body).expect("response body should be valid JSON")
}

#[tokio::test]
async fn into_response_behaves_as_expected() {
    let internal = ApiError::Internal(
        "database connection failed".to_string(),
    )
    .into_response();
    let (internal_body, content_type, _) = extract_response_body(internal).await;
    assert!(!internal_body.contains("password"));
    assert!(!internal_body.contains("secret"));
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    let json = parse_problem_body(&internal_body);
    assert_eq!(json["title"], "Internal Server Error");
    assert_eq!(json["status"], 500);
    assert!(json["type"].is_string());

    let bad_request = ApiError::bad_request("invalid input").into_response();
    assert_eq!(bad_request.status(), StatusCode::BAD_REQUEST);
    let (body, content_type, _) = extract_response_body(bad_request).await;
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    let json = parse_problem_body(&body);
    assert_eq!(json["title"], "Bad Request");
    assert_eq!(json["status"], 400);
    assert_eq!(json["detail"], "invalid input");

    let not_found = ApiError::not_found("resource gone").into_response();
    assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    let (body, content_type, _) = extract_response_body(not_found).await;
    assert_eq!(content_type, PROBLEM_CONTENT_TYPE);
    let json = parse_problem_body(&body);
    assert_eq!(json["title"], "Not Found");
    assert_eq!(json["status"], 404);
    assert_eq!(json["detail"], "resource gone");
}

#[test]
fn datastore_errors_map_correctly() {
    let cases = [
        (
            twerk_infrastructure::datastore::Error::UserNotFound,
            ApiError::NotFound("user not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::JobNotFound,
            ApiError::NotFound("job not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::TaskNotFound,
            ApiError::NotFound("task not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::ScheduledJobNotFound,
            ApiError::NotFound("scheduled job not found".to_string()),
        ),
        (
            twerk_infrastructure::datastore::Error::NodeNotFound,
            ApiError::NotFound("node not found".to_string()),
        ),
    ];

    for (input, expected) in cases {
        let api_err: ApiError = input.into();
        assert_eq!(api_err, expected);
    }

    let internal: ApiError =
        twerk_infrastructure::datastore::Error::Database("table not found".to_string()).into();
    assert_eq!(
        internal,
        ApiError::Internal("database error: table not found".to_string())
    );
}

#[test]
fn generic_error_conversions_map_correctly() {
    let anyhow_err: ApiError = anyhow::anyhow!("something broke").into();
    assert_eq!(
        anyhow_err,
        ApiError::Internal("something broke".to_string())
    );

    let trigger_cases = [
        (
            TriggerUpdateError::InvalidIdFormat("bad$id".to_string()),
            ApiError::BadRequest("bad$id".to_string()),
        ),
        (
            TriggerUpdateError::UnsupportedContentType("application/xml".to_string()),
            ApiError::BadRequest("application/xml".to_string()),
        ),
        (
            TriggerUpdateError::MalformedJson("unexpected token".to_string()),
            ApiError::BadRequest("unexpected token".to_string()),
        ),
        (
            TriggerUpdateError::ValidationFailed("name is required".to_string()),
            ApiError::BadRequest("name is required".to_string()),
        ),
        (
            TriggerUpdateError::TriggerNotFound("trg_123".to_string()),
            ApiError::NotFound("trg_123".to_string()),
        ),
        (
            TriggerUpdateError::VersionConflict("optimistic lock".to_string()),
            ApiError::BadRequest("optimistic lock".to_string()),
        ),
        (
            TriggerUpdateError::Persistence("db connection lost".to_string()),
            ApiError::Internal("db connection lost".to_string()),
        ),
        (
            TriggerUpdateError::Serialization("json encode failed".to_string()),
            ApiError::Internal("json encode failed".to_string()),
        ),
    ];

    for (input, expected) in trigger_cases {
        let api_err: ApiError = input.into();
        assert_eq!(api_err, expected);
    }

    let mismatch: ApiError = TriggerUpdateError::IdMismatch {
        path_id: "trg_1".to_string(),
        body_id: "trg_2".to_string(),
    }
    .into();
    assert_eq!(mismatch, ApiError::BadRequest("id mismatch".to_string()));
}

#[tokio::test]
async fn exact_payloads_are_preserved() {
    let not_found = ApiError::NotFound("missing trigger".to_string()).into_response();
    assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    let (body, _, _) = extract_response_body(not_found).await;
    let json = parse_problem_body(&body);
    assert_eq!(json["type"], "https://httpstatus.es/404");
    assert_eq!(json["title"], "Not Found");
    assert_eq!(json["status"], 404);
    assert_eq!(json["detail"], "missing trigger");

    let internal = ApiError::Internal("leaky detail".to_string()).into_response();
    assert_eq!(internal.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let (body, _, _) = extract_response_body(internal).await;
    let json = parse_problem_body(&body);
    assert_eq!(json["type"], "https://httpstatus.es/500");
    assert_eq!(json["title"], "Internal Server Error");
    assert_eq!(json["status"], 500);
    assert_eq!(json["detail"], "leaky detail");
}

#[tokio::test]
async fn error_handler_logs_500_with_request_context() {
    let (writer, _guard) = setup_log_capture();

    let internal_error = ApiError::Internal("database connection refused".to_string());
    internal_error.into_response();

    let logs = writer.get_logs();
    let has_error_level = logs.iter().any(|log| log.contains("ERROR"));
    assert!(
        has_error_level,
        "Expected ERROR level log for 500 error, got: {:?}",
        logs
    );

    let has_error_msg = logs.iter().any(|log| log.contains("database connection refused"));
    assert!(
        has_error_msg,
        "Expected log to contain error message, got: {:?}",
        logs
    );
}

#[tokio::test]
async fn error_handler_logs_404_at_warn_level() {
    let (writer, _guard) = setup_log_capture();

    let not_found_error = ApiError::NotFound("resource not found".to_string());
    not_found_error.into_response();

    let logs = writer.get_logs();
    let has_warn_level = logs.iter().any(|log| log.contains("WARN") || log.contains("WARNING"));
    assert!(
        has_warn_level,
        "Expected WARN level log for 404 error, got: {:?}",
        logs
    );
}

#[tokio::test]
async fn error_handler_logs_500_at_error_level_with_context() {
    let (writer, _guard) = setup_log_capture();

    let internal_error = ApiError::Internal("stack trace: at Module.func".to_string());
    internal_error.into_response();

    let logs = writer.get_logs();

    let has_error_level = logs.iter().any(|log| log.contains("ERROR"));
    assert!(
        has_error_level,
        "Expected ERROR level log for 500 error, got: {:?}",
        logs
    );

    let has_stack_trace = logs.iter().any(|log| log.contains("stack trace"));
    assert!(
        has_stack_trace,
        "Expected log to contain stack trace, got: {:?}",
        logs
    );
}

#[tokio::test]
async fn error_logs_contain_method_path_for_500_errors() {
    let (writer, _guard) = setup_log_capture();

    let internal_error = ApiError::Internal("handler failed".to_string());
    internal_error.into_response();

    let logs = writer.get_logs();
    let all_logs = logs.join("\n");

    assert!(
        all_logs.contains("method") || all_logs.contains("GET") || all_logs.contains("POST"),
        "Expected log to contain HTTP method, got: {:?}",
        logs
    );

    assert!(
        all_logs.contains("path") || all_logs.contains("/"),
        "Expected log to contain request path, got: {:?}",
        logs
    );
}
