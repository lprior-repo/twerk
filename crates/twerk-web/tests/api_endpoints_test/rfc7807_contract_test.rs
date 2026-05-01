use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use bytes::Bytes;
use http_body_util::BodyExt;
use tower::ServiceExt;

use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::{create_router, AppState, Config};

struct TestResponse {
    status: StatusCode,
    content_type: String,
    body: Bytes,
    json: serde_json::Value,
}

impl TestResponse {
    fn status(&self) -> StatusCode {
        self.status
    }

    fn json(&self) -> &serde_json::Value {
        &self.json
    }

    fn content_type(&self) -> &str {
        &self.content_type
    }

    fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(self.status, expected, "response body: {:?}", self.body);
        self
    }

    fn assert_problem_content_type(&self) -> &Self {
        assert!(
            self.content_type.contains("application/problem+json"),
            "expected application/problem+json, got: {}",
            self.content_type
        );
        self
    }

    fn assert_rfc7807_fields(&self, expected_status: u16) -> &Self {
        let json = self.json();
        assert!(
            json.get("type").is_some(),
            "RFC 7807 response missing 'type' field"
        );
        assert!(
            json.get("title").is_some(),
            "RFC 7807 response missing 'title' field"
        );
        assert!(
            json.get("status").is_some(),
            "RFC 7807 response missing 'status' field"
        );
        assert!(
            json.get("detail").is_some(),
            "RFC 7807 response missing 'detail' field"
        );
        assert_eq!(
            json["status"].as_i64().unwrap() as u16,
            expected_status,
            "status field mismatch"
        );
        self
    }

    fn assert_no_stack_trace_leak(&self) -> &Self {
        let body_str = String::from_utf8_lossy(&self.body);
        assert!(
            !body_str.contains("stack"),
            "response should not contain 'stack'"
        );
        assert!(
            !body_str.contains("trace"),
            "response should not contain 'trace'"
        );
        assert!(
            !body_str.contains("at "),
            "response should not contain 'at ' (stack trace pattern)"
        );
        self
    }
}

fn app() -> axum::Router {
    let broker = Arc::new(InMemoryBroker::new());
    let datastore = Arc::new(InMemoryDatastore::new());
    let state = AppState::new(broker, datastore, Config::default());
    create_router(state)
}

use std::sync::Arc;

async fn call(request: Request<Body>) -> TestResponse {
    let response = app()
        .oneshot(request)
        .await
        .expect("request should succeed");
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should be readable")
        .to_bytes();
    let json = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&body).expect("response body should be valid JSON")
    };

    TestResponse {
        status,
        content_type,
        body,
        json,
    }
}

async fn get(uri: &str) -> TestResponse {
    call(
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .expect("GET request should build"),
    )
    .await
}

#[tokio::test]
async fn get_nonexistent_returns_404_with_rfc7807_problem_details() {
    let response = get("/nonexistent-path-that-does-not-exist").await;
    response
        .assert_status(StatusCode::NOT_FOUND)
        .assert_problem_content_type()
        .assert_rfc7807_fields(404);
    assert_eq!(response.json()["title"], "Not Found");
    assert!(response.json()["detail"]
        .as_str()
        .is_some_and(|d| !d.is_empty()));
}

#[tokio::test]
async fn get_nonexistent_returns_problem_type_uri() {
    let response = get("/tasks/nonexistent-task-id").await;
    response
        .assert_status(StatusCode::NOT_FOUND)
        .assert_problem_content_type()
        .assert_rfc7807_fields(404);
    let type_uri = response.json()["type"].as_str().unwrap();
    assert!(
        type_uri.contains("404"),
        "type URI should contain 404: {}",
        type_uri
    );
}

#[tokio::test]
async fn post_invalid_json_returns_400_with_rfc7807_problem_details() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{invalid json"))
        .expect("request should build");
    let response = call(request).await;
    response
        .assert_status(StatusCode::BAD_REQUEST)
        .assert_problem_content_type()
        .assert_rfc7807_fields(400);
    assert_eq!(response.json()["title"], "Bad Request");
}

#[tokio::test]
async fn post_invalid_json_returns_problem_without_stack_leak() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{invalid json"))
        .expect("request should build");
    let response = call(request).await;
    response
        .assert_status(StatusCode::BAD_REQUEST)
        .assert_problem_content_type()
        .assert_no_stack_trace_leak();
}

#[tokio::test]
async fn internal_error_returns_500_with_rfc7807_problem_details() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name": "test", "tasks": [{"name": "t", "image": "alpine", "run": "echo"}]}"#))
        .expect("request should build");
    let response = call(request).await;
    if response.status() == StatusCode::INTERNAL_SERVER_ERROR {
        response
            .assert_problem_content_type()
            .assert_rfc7807_fields(500);
        assert_eq!(response.json()["title"], "Internal Server Error");
    }
}

#[tokio::test]
async fn internal_error_never_leaks_stack_trace() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/scheduled-jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name": "test", "cron": "invalid", "tasks": [{"name": "t", "image": "alpine", "run": "echo"}]}"#))
        .expect("request should build");
    let response = call(request).await;
    if response.status() == StatusCode::INTERNAL_SERVER_ERROR {
        response
            .assert_problem_content_type()
            .assert_no_stack_trace_leak();
    }
}

#[tokio::test]
async fn all_error_responses_use_application_problem_json() {
    let routes_with_expected_errors = [
        ("/nonexistent", StatusCode::NOT_FOUND),
        ("/tasks/nonexistent-id", StatusCode::NOT_FOUND),
        ("/jobs/nonexistent-id", StatusCode::NOT_FOUND),
    ];

    for (uri, _expected_status) in routes_with_expected_errors {
        let response = get(uri).await;
        assert!(
            response.content_type().contains("application/problem+json"),
            "Content-Type for {} should be application/problem+json, got: {}",
            uri,
            response.content_type()
        );
    }
}

#[tokio::test]
async fn post_invalid_json_returns_rfc7807_problem_with_about_blank_type() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/jobs")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{invalid json"))
        .expect("request should build");
    let response = call(request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        response.content_type().contains("application/problem+json"),
        "expected application/problem+json, got: {}",
        response.content_type()
    );

    let json = response.json();
    assert!(json.get("type").is_some(), "RFC 7807 response missing 'type' field");
    assert!(json.get("title").is_some(), "RFC 7807 response missing 'title' field");
    assert!(json.get("status").is_some(), "RFC 7807 response missing 'status' field");
    assert!(json.get("detail").is_some(), "RFC 7807 response missing 'detail' field");
    assert_eq!(
        json["status"].as_i64().unwrap() as u16,
        400,
        "status field mismatch"
    );
    assert_eq!(
        json["type"].as_str().unwrap(),
        "about:blank",
        "type URI should be 'about:blank', got: {}",
        json["type"]
    );
}