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

use axum::http::{header, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::id::JobId;
use twerk_core::job::Job;
use twerk_infrastructure::broker::{inmemory::InMemoryBroker, Broker};
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::{create_router, AppState, Config};

fn to_job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("test job id should be valid")
}

struct SignalingBroker {
    inner: InMemoryBroker,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl SignalingBroker {
    fn new(tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self {
            inner: InMemoryBroker::new(),
            tx,
        }
    }
}

impl twerk_infrastructure::broker::Broker for SignalingBroker {
    fn publish_task(
        &self,
        qname: String,
        task: &twerk_core::task::Task,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_task(qname, task)
    }
    fn subscribe_for_tasks(
        &self,
        qname: String,
        handler: twerk_infrastructure::broker::TaskHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_tasks(qname, handler)
    }
    fn publish_task_progress(
        &self,
        task: &twerk_core::task::Task,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_task_progress(task)
    }
    fn subscribe_for_task_progress(
        &self,
        handler: twerk_infrastructure::broker::TaskProgressHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_task_progress(handler)
    }
    fn publish_heartbeat(
        &self,
        node: twerk_core::node::Node,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_heartbeat(node)
    }
    fn subscribe_for_heartbeats(
        &self,
        handler: twerk_infrastructure::broker::HeartbeatHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_heartbeats(handler)
    }
    fn publish_job(
        &self,
        job: &twerk_core::job::Job,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_job(job)
    }
    fn subscribe_for_jobs(
        &self,
        handler: twerk_infrastructure::broker::JobHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_jobs(handler)
    }
    fn publish_event(
        &self,
        topic: String,
        event: Value,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_event(topic, event)
    }
    fn subscribe_for_events(
        &self,
        pattern: String,
        handler: twerk_infrastructure::broker::EventHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        let tx = self.tx.clone();
        let fut = self.inner.subscribe_for_events(pattern, handler);
        Box::pin(async move {
            fut.await?;
            let _ = tx.send(()).await;
            Ok(())
        })
    }
    fn subscribe(
        &self,
        pattern: String,
    ) -> twerk_infrastructure::broker::BoxedFuture<
        tokio::sync::broadcast::Receiver<twerk_core::job::JobEvent>,
    > {
        self.inner.subscribe(pattern)
    }
    fn publish_task_log_part(
        &self,
        part: &twerk_core::task::TaskLogPart,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_task_log_part(part)
    }
    fn subscribe_for_task_log_part(
        &self,
        handler: twerk_infrastructure::broker::TaskLogPartHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_task_log_part(handler)
    }
    fn queues(
        &self,
    ) -> twerk_infrastructure::broker::BoxedFuture<Vec<twerk_infrastructure::broker::QueueInfo>>
    {
        self.inner.queues()
    }
    fn queue_info(
        &self,
        qname: String,
    ) -> twerk_infrastructure::broker::BoxedFuture<twerk_infrastructure::broker::QueueInfo> {
        self.inner.queue_info(qname)
    }
    fn delete_queue(&self, qname: String) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.delete_queue(qname)
    }
    fn health_check(&self) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.health_check()
    }
    fn shutdown(&self) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.shutdown()
    }
}

#[tokio::test]
async fn health_status_is_up_when_engine_is_ready() {
    let state = setup_state().await;
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("app should not panic");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "UP");
}

#[tokio::test]
async fn health_status_returns_503_when_broker_health_check_fails() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(FailingHealthBroker {
        inner: InMemoryBroker::new(),
    });
    let state = AppState::new(broker, ds, Config::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .expect("request builder should not fail"),
        )
        .await
        .expect("app should not panic");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = body_to_json(response).await;
    assert_eq!(body["status"], "DOWN");
}

struct FailingHealthBroker {
    inner: InMemoryBroker,
}

impl twerk_infrastructure::broker::Broker for FailingHealthBroker {
    fn publish_task(
        &self,
        qname: String,
        task: &twerk_core::task::Task,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_task(qname, task)
    }
    fn subscribe_for_tasks(
        &self,
        qname: String,
        handler: twerk_infrastructure::broker::TaskHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_tasks(qname, handler)
    }
    fn publish_task_progress(
        &self,
        task: &twerk_core::task::Task,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_task_progress(task)
    }
    fn subscribe_for_task_progress(
        &self,
        handler: twerk_infrastructure::broker::TaskProgressHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_task_progress(handler)
    }
    fn publish_heartbeat(
        &self,
        node: twerk_core::node::Node,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_heartbeat(node)
    }
    fn subscribe_for_heartbeats(
        &self,
        handler: twerk_infrastructure::broker::HeartbeatHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_heartbeats(handler)
    }
    fn publish_job(
        &self,
        job: &twerk_core::job::Job,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_job(job)
    }
    fn subscribe_for_jobs(
        &self,
        handler: twerk_infrastructure::broker::JobHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_jobs(handler)
    }
    fn publish_event(
        &self,
        topic: String,
        event: Value,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_event(topic, event)
    }
    fn subscribe_for_events(
        &self,
        pattern: String,
        handler: twerk_infrastructure::broker::EventHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_events(pattern, handler)
    }
    fn subscribe(
        &self,
        pattern: String,
    ) -> twerk_infrastructure::broker::BoxedFuture<
        tokio::sync::broadcast::Receiver<twerk_core::job::JobEvent>,
    > {
        self.inner.subscribe(pattern)
    }
    fn publish_task_log_part(
        &self,
        part: &twerk_core::task::TaskLogPart,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.publish_task_log_part(part)
    }
    fn subscribe_for_task_log_part(
        &self,
        handler: twerk_infrastructure::broker::TaskLogPartHandler,
    ) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.subscribe_for_task_log_part(handler)
    }
    fn queues(
        &self,
    ) -> twerk_infrastructure::broker::BoxedFuture<Vec<twerk_infrastructure::broker::QueueInfo>>
    {
        self.inner.queues()
    }
    fn queue_info(
        &self,
        qname: String,
    ) -> twerk_infrastructure::broker::BoxedFuture<twerk_infrastructure::broker::QueueInfo> {
        self.inner.queue_info(qname)
    }
    fn delete_queue(&self, qname: String) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.delete_queue(qname)
    }
    fn health_check(&self) -> twerk_infrastructure::broker::BoxedFuture<()> {
        Box::pin(async { Err(anyhow::anyhow!("broker unhealthy")) })
    }
    fn shutdown(&self) -> twerk_infrastructure::broker::BoxedFuture<()> {
        self.inner.shutdown()
    }
}

#[tokio::test]
async fn job_created_successfully_when_valid_json_posted() {
    let state = setup_state().await;
    let app = create_router(state);

    let job_input = json!({
        "name": "test-job",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).expect("json serialization should not fail"),
                ))
                .expect("request builder should not fail"),
        )
        .await
        .expect("app should not panic");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "test-job");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn job_created_successfully_when_valid_yaml_posted() {
    let state = setup_state().await;
    let app = create_router(state);

    let yaml_input = "
name: test-job-yaml
tasks:
  - name: task-1
    image: alpine
    run: echo hello
";

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/x-yaml")
                .body(axum::body::Body::from(yaml_input))
                .expect("request builder should not fail"),
        )
        .await
        .expect("app should not panic");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "test-job-yaml");
}

#[tokio::test]
async fn job_wait_returns_completed_when_job_finishes() {
    let ds = Arc::new(InMemoryDatastore::new());
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel(1);
    let broker = Arc::new(SignalingBroker::new(sub_tx));
    let state = AppState::new(broker.clone(), ds, Config::default());
    let app = create_router(state);

    let job_id = "550e8400-e29b-41d4-a716-446655440701";
    let job_input = json!({
        "id": job_id,
        "name": "test-job-wait",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let broker_clone = broker.clone();
    let job_id_clone = job_id.to_string();
    tokio::spawn(async move {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), sub_rx.recv()).await;
        let finished_job = json!({
            "id": job_id_clone,
            "state": "COMPLETED",
            "name": "test-job-wait"
        });
        broker_clone
            .publish_event("job.completed".to_string(), finished_job)
            .await
            .unwrap();
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs?wait=true")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["state"], "COMPLETED");
}

#[tokio::test]
async fn job_wait_blocking_explicit_string_returns_completed() {
    let ds = Arc::new(InMemoryDatastore::new());
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel(1);
    let broker = Arc::new(SignalingBroker::new(sub_tx));
    let state = AppState::new(broker.clone(), ds, Config::default());
    let app = create_router(state);

    let job_id = "550e8400-e29b-41d4-a716-446655440702";
    let job_input = json!({
        "id": job_id,
        "name": "test-job-wait-blocking",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let broker_clone = broker.clone();
    let job_id_clone = job_id.to_string();
    tokio::spawn(async move {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), sub_rx.recv()).await;
        let finished_job = json!({
            "id": job_id_clone,
            "state": "COMPLETED",
            "name": "test-job-wait-blocking"
        });
        broker_clone
            .publish_event("job.completed".to_string(), finished_job)
            .await
            .unwrap();
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs?wait=blocking")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["state"], "COMPLETED");
}

#[tokio::test]
async fn job_wait_truthy_string_values_return_completed() {
    for (index, wait_value) in ["1", "yes", "YES", "Yes", "TRUE", "True"]
        .into_iter()
        .enumerate()
    {
        let ds = Arc::new(InMemoryDatastore::new());
        let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel(1);
        let broker = Arc::new(SignalingBroker {
            inner: InMemoryBroker::new(),
            tx: sub_tx,
        });
        let state = AppState::new(broker.clone(), ds, Config::default());
        let app = create_router(state);

        let job_id = format!("550e8400-e29b-41d4-a716-44665544{:04}", 710 + index);
        let job_input = json!({
            "id": job_id,
            "name": format!("test-job-{}", wait_value),
            "tasks": [
                {
                    "name": "task-1",
                    "image": "alpine",
                    "run": "echo hello"
                }
            ]
        });

        let broker_clone = broker.clone();
        let job_id_clone = job_id.clone();
        tokio::spawn(async move {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), sub_rx.recv()).await;
            let finished_job = json!({
                "id": job_id_clone,
                "state": "COMPLETED",
                "name": format!("test-job-{}", wait_value)
            });
            broker_clone
                .publish_event("job.completed".to_string(), finished_job)
                .await
                .unwrap();
        });

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri(format!("/jobs?wait={}", wait_value))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&job_input).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "wait={} should be blocking",
            wait_value
        );
        let body = body_to_json(response).await;
        assert_eq!(
            body["state"], "COMPLETED",
            "wait={} should wait for completion",
            wait_value
        );
    }
}

#[tokio::test]
async fn job_wait_detached_returns_pending_immediately() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds, Config::default());
    let app = create_router(state);

    let job_input = json!({
            "id": "550e8400-e29b-41d4-a716-446655440720",
        "name": "test-job-detached",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let start = std::time::Instant::now();
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs?wait=detached")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "detached mode should return immediately"
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(
        body["state"], "PENDING",
        "detached mode should return PENDING state"
    );
}

#[tokio::test]
async fn job_wait_false_returns_pending_immediately() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds, Config::default());
    let app = create_router(state);

    let job_input = json!({
            "id": "550e8400-e29b-41d4-a716-446655440721",
        "name": "test-job-wait-false",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let start = std::time::Instant::now();
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs?wait=false")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "wait=false should return immediately"
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(
        body["state"], "PENDING",
        "wait=false should return PENDING state"
    );
}

#[tokio::test]
async fn job_wait_invalid_value_returns_pending_immediately() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds, Config::default());
    let app = create_router(state);

    let job_input = json!({
            "id": "550e8400-e29b-41d4-a716-446655440722",
        "name": "test-job-wait-invalid",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let start = std::time::Instant::now();
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs?wait=garbage")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "invalid wait value should return immediately"
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(
        body["state"], "PENDING",
        "invalid wait value should return PENDING state"
    );
}

#[tokio::test]
async fn job_wait_default_returns_pending_immediately() {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds, Config::default());
    let app = create_router(state);

    let job_input = json!({
            "id": "550e8400-e29b-41d4-a716-446655440723",
        "name": "test-job-wait-default",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let start = std::time::Instant::now();
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "default (no wait) should return immediately"
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(
        body["state"], "PENDING",
        "default should return PENDING state"
    );
}

#[tokio::test]
async fn job_wait_blocking_returns_failed_when_job_fails() {
    let ds = Arc::new(InMemoryDatastore::new());
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel(1);
    let broker = Arc::new(SignalingBroker::new(sub_tx));
    let state = AppState::new(broker.clone(), ds, Config::default());
    let app = create_router(state);

    let job_id = "550e8400-e29b-41d4-a716-446655440724";
    let job_input = json!({
        "id": job_id,
        "name": "test-job-wait-failed",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let broker_clone = broker.clone();
    let job_id_clone = job_id.to_string();
    tokio::spawn(async move {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), sub_rx.recv()).await;
        let failed_job = json!({
            "id": job_id_clone,
            "state": "FAILED",
            "name": "test-job-wait-failed",
            "error": "Task execution failed"
        });
        broker_clone
            .publish_event("job.failed".to_string(), failed_job)
            .await
            .unwrap();
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs?wait=true")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(
        body["state"], "FAILED",
        "blocking wait should return FAILED state when job fails"
    );
    assert!(
        body["error"].is_string(),
        "failed job should have error message"
    );
}

#[tokio::test]
async fn job_wait_blocking_returns_cancelled_when_job_cancelled() {
    let ds = Arc::new(InMemoryDatastore::new());
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel(1);
    let broker = Arc::new(SignalingBroker::new(sub_tx));
    let state = AppState::new(broker.clone(), ds, Config::default());
    let app = create_router(state);

    let job_id = "550e8400-e29b-41d4-a716-446655440725";
    let job_input = json!({
        "id": job_id,
        "name": "test-job-wait-cancelled",
        "tasks": [
            {
                "name": "task-1",
                "image": "alpine",
                "run": "echo hello"
            }
        ]
    });

    let broker_clone = broker.clone();
    let job_id_clone = job_id.to_string();
    tokio::spawn(async move {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), sub_rx.recv()).await;
        let cancelled_job = json!({
            "id": job_id_clone,
            "state": "CANCELLED",
            "name": "test-job-wait-cancelled"
        });
        broker_clone
            .publish_event("job.cancelled".to_string(), cancelled_job)
            .await
            .unwrap();
    });

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs?wait=true")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(
        body["state"], "CANCELLED",
        "blocking wait should return CANCELLED state when job cancelled"
    );
}

#[tokio::test]
async fn job_secrets_redacted_when_fetched_from_api() {
    let state = setup_state().await;
    let ds = state.ds.clone();
    let app = create_router(state);
    let job_id = "550e8400-e29b-41d4-a716-446655440601";

    let job = Job {
        id: Some(to_job_id(job_id)),
        name: Some("secret-job".to_string()),
        secrets: Some([("my_secret".to_string(), "password123".to_string())].into()),
        inputs: Some([("api_key".to_string(), "password123".to_string())].into()),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{job_id}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["secrets"]["my_secret"], "[REDACTED]");
    assert_eq!(body["inputs"]["api_key"], "[REDACTED]");
}

#[tokio::test]
async fn error_response_formatted_as_json_when_job_missing() {
    let state = setup_state().await;
    let app = create_router(state);
    let missing_job_id = "550e8400-e29b-41d4-a716-446655449999";

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/jobs/{missing_job_id}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_to_json(response).await;
    assert!(body["message"].is_string());
}

async fn setup_state() -> AppState {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    AppState::new(broker, ds, Config::default())
}

async fn body_to_json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}
