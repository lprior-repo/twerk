use axum::http::{header, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use twerk_core::job::Job;
use twerk_infrastructure::broker::{inmemory::InMemoryBroker, Broker};
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_web::api::{create_router, AppState, Config};

#[tokio::test]
async fn health_status_is_up_when_engine_is_ready() {
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
                .unwrap(),
        )
        .await
        .unwrap();

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
                    serde_json::to_vec(&job_input).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "test-job");
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn job_created_successfully_when_valid_yaml_posted() {
    let state = setup_state().await;
    let app = create_router(state);

    let yaml_input = r#"
name: test-job-yaml
tasks:
  - name: task-1
    image: alpine
    run: echo hello
"#;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/jobs")
                .header(header::CONTENT_TYPE, "application/x-yaml")
                .body(axum::body::Body::from(yaml_input))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_to_json(response).await;
    assert_eq!(body["name"], "test-job-yaml");
}

#[tokio::test]
async fn job_wait_returns_completed_when_job_finishes() {
    let ds = Arc::new(InMemoryDatastore::new());
    let (sub_tx, mut sub_rx) = tokio::sync::mpsc::channel(1);

    // Custom broker that signals when a subscription happens
    struct SignalingBroker {
        inner: InMemoryBroker,
        tx: tokio::sync::mpsc::Sender<()>,
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
        ) -> twerk_infrastructure::broker::BoxedFuture<twerk_infrastructure::broker::QueueInfo>
        {
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

    let broker = Arc::new(SignalingBroker {
        inner: InMemoryBroker::new(),
        tx: sub_tx,
    });
    let state = AppState::new(broker.clone(), ds, Config::default());
    let app = create_router(state);

    let job_id = "job-wait-1";
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

    // We simulate the job finishing in another task
    let broker_clone = broker.clone();
    let job_id_clone = job_id.to_string();
    tokio::spawn(async move {
        // Wait for notification that the handler has subscribed
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
async fn job_secrets_redacted_when_fetched_from_api() {
    let state = setup_state().await;
    let ds = state.ds.clone();
    let app = create_router(state);

    let job = Job {
        id: Some("job-1".into()),
        name: Some("secret-job".to_string()),
        secrets: Some([("my_secret".to_string(), "password123".to_string())].into()),
        inputs: Some([("api_key".to_string(), "password123".to_string())].into()),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/job-1")
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

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/jobs/non-existent")
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
