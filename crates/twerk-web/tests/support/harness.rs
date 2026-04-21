use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use bytes::Bytes;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use twerk_core::node::Node;
use twerk_core::task::Task;
use twerk_infrastructure::broker::{inmemory::InMemoryBroker, Broker};
use twerk_infrastructure::datastore::{inmemory::InMemoryDatastore, Datastore};
use twerk_web::api::trigger_api::{InMemoryTriggerDatastore, Trigger, TriggerAppState};
use twerk_web::api::{create_router, AppState, Config};

use super::{queued_task, trigger};

#[derive(Clone)]
pub struct TestHarness {
    app: axum::Router,
    state: AppState,
    trigger_ds: Arc<InMemoryTriggerDatastore>,
}

pub struct TestResponse {
    status: StatusCode,
    content_type: String,
    body: Bytes,
    text: String,
    json: Result<Value, String>,
}

impl TestHarness {
    pub async fn new() -> Self {
        Self::build(None, &[]).await
    }

    pub async fn with_queue(queue_name: &str) -> Self {
        Self::build(Some(queue_name), &[]).await
    }

    pub async fn with_trigger_ids(ids: &[&str]) -> Self {
        Self::build(None, ids).await
    }

    async fn build(queue_name: Option<&str>, trigger_ids: &[&str]) -> Self {
        let ds = Arc::new(InMemoryDatastore::new());
        let broker = Arc::new(InMemoryBroker::new());
        let trigger_ds = Arc::new(InMemoryTriggerDatastore::new());

        if let Some(name) = queue_name {
            broker
                .publish_task(name.to_string(), &queued_task("queued-task"))
                .await
                .unwrap();
        }

        trigger_ids.iter().for_each(|id| {
            trigger_ds.upsert(trigger(id)).unwrap();
        });

        let state = AppState {
            trigger_state: TriggerAppState {
                trigger_ds: trigger_ds.clone(),
            },
            ..AppState::new(broker, ds, Config::default())
        };
        let app = create_router(state.clone());

        Self {
            app,
            state,
            trigger_ds,
        }
    }

    pub async fn call(&self, request: Request<Body>) -> TestResponse {
        call(&self.app, request).await
    }

    pub fn into_state(self) -> AppState {
        self.state
    }

    pub fn trigger_store(&self) -> Arc<InMemoryTriggerDatastore> {
        self.trigger_ds.clone()
    }

    pub async fn get(&self, uri: &str) -> TestResponse {
        self.call(empty_request(uri)).await
    }

    pub async fn delete(&self, uri: &str) -> TestResponse {
        self.call(request(Method::DELETE, uri)).await
    }

    pub async fn post_json(&self, uri: &str, payload: &Value) -> TestResponse {
        self.call(json_request(Method::POST, uri, payload)).await
    }

    pub async fn post_yaml(
        &self,
        uri: &str,
        body: impl Into<Body>,
        content_type: &str,
    ) -> TestResponse {
        self.call(yaml_request(Method::POST, uri, body, content_type))
            .await
    }

    pub async fn put_empty(&self, uri: &str) -> TestResponse {
        self.call(request(Method::PUT, uri)).await
    }

    pub async fn seed_job(&self, job: &twerk_core::job::Job) {
        self.state.ds.create_job(job).await.unwrap();
    }

    pub async fn seed_task(&self, task: &Task) {
        self.state.ds.create_task(task).await.unwrap();
    }

    pub async fn seed_node(&self, node: &Node) {
        self.state.ds.create_node(node).await.unwrap();
    }

    pub fn upsert_trigger(&self, trigger: Trigger) {
        self.trigger_ds.upsert(trigger).unwrap();
    }
}

impl TestResponse {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }

    pub fn json(&self) -> &Value {
        match &self.json {
            Ok(value) => value,
            Err(error) => panic!(
                "response body was not valid JSON: {error}; body={}",
                self.text
            ),
        }
    }

    pub fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(self.status, expected, "response body: {}", self.text);
        self
    }

    pub fn assert_json_content_type(&self) -> &Self {
        assert_eq!(self.content_type, "application/json");
        self
    }
}

pub fn empty_request(uri: &str) -> Request<Body> {
    request(Method::GET, uri)
}

pub fn request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

pub fn request_with_content_type(
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
        .unwrap()
}

pub fn json_request(method: Method, uri: &str, payload: &Value) -> Request<Body> {
    request_with_content_type(
        method,
        uri,
        "application/json",
        serde_json::to_vec(payload).unwrap(),
    )
}

pub fn yaml_request(
    method: Method,
    uri: &str,
    body: impl Into<Body>,
    content_type: &str,
) -> Request<Body> {
    request_with_content_type(method, uri, content_type, body)
}

pub async fn call(app: &axum::Router, request: Request<Body>) -> TestResponse {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&body).into_owned();
    let json = if body.is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_slice(&body).map_err(|error| error.to_string())
    };

    TestResponse {
        status,
        content_type,
        body,
        text,
        json,
    }
}
