use axum::http::StatusCode;
use axum::{
    body::Body,
    http::{header, Method, Request},
};
use bytes::Bytes;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use std::sync::Arc;

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

    fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(self.status, expected, "response body: {:?}", self.body);
        self
    }

    fn assert_json_content_type(&self) -> &Self {
        assert_eq!(self.content_type, "application/json");
        self
    }
}

fn app() -> axum::Router {
    let broker = Arc::new(InMemoryBroker::new());
    let datastore = Arc::new(InMemoryDatastore::new());
    let state = AppState::new(broker, datastore, Config::default());
    create_router(state)
}

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

fn request_with_content_type(
    method: Method,
    uri: &str,
    content_type: &str,
    body: impl Into<Body>,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, content_type)
        .body(body.into())
        .expect("request should build")
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

async fn post_json(uri: &str, payload: &serde_json::Value) -> TestResponse {
    call(request_with_content_type(
        Method::POST,
        uri,
        "application/json",
        serde_json::to_vec(payload).expect("JSON payload should serialize"),
    ))
    .await
}

async fn post_body(uri: &str, body: impl Into<Body>, content_type: &str) -> TestResponse {
    call(request_with_content_type(
        Method::POST,
        uri,
        content_type,
        body,
    ))
    .await
}

#[tokio::test]
async fn list_nodes_returns_nodes_list() {
    let response = get("/nodes").await;
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert!(response.json().is_array());
}

#[tokio::test]
async fn get_metrics_returns_metrics() {
    let response = get("/metrics").await;
    response
        .assert_status(StatusCode::OK)
        .assert_json_content_type();
    assert!(response.json().is_object());
}

#[tokio::test]
async fn create_user_returns_401_without_authentication() {
    let response = post_json(
        "/users",
        &json!({"username": "testuser", "password": "testpassword"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_user_returns_400_without_username() {
    let response = post_json("/users", &json!({"password": "testpassword"})).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_user_returns_400_without_password() {
    let response = post_json("/users", &json!({"username": "testuser"})).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_job_returns_400_with_unsupported_content_type() {
    let response = post_body("/jobs", "plain text body", "text/plain").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_scheduled_job_returns_400_with_unsupported_content_type() {
    let response = post_body("/scheduled-jobs", "plain text body", "text/plain").await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_job_returns_400_with_invalid_json() {
    let response = call(request_with_content_type(
        Method::POST,
        "/jobs",
        "application/json",
        Body::from("{invalid json"),
    ))
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn health_response_includes_version() {
    let response = get("/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.json()["version"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
}
