//! Integration tests for the coordinator API.
//!
//! These tests mirror the Go tests in `tork/internal/coordinator/api/api_test.go`.
//! Each test creates a real API server with an in-memory broker and PostgreSQL
//! datastore, then makes HTTP requests to verify behavior.

use std::sync::Arc;

use axum::http::{header, Method, StatusCode};
use axum::{body::Body, routing::get, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use tokio::sync::watch;
use tower::ServiceExt;

use coordinator::api::{create_router, AppState};
use tork_runtime::broker::inmemory::new_in_memory_broker;
use tork::{Broker, QueueInfo};
use tork::job::{
    Job, JOB_STATE_CANCELLED, JOB_STATE_FAILED, JOB_STATE_PENDING, JOB_STATE_RUNNING,
    JOB_STATE_SCHEDULED,
};
use tork::node::{Node, NODE_STATUS_UP};
use tork::task::{Task, TASK_STATE_PENDING, TASK_STATE_RUNNING, TASK_STATE_SCHEDULED};
use tork::Datastore;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Helper to generate a UUID string matching Go's uuid.NewUUID().
fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

/// Wrapper for PostgreSQL datastore integration tests.
///
/// Creates a real PostgreSQL connection for testing.
struct TestEnv {
    ds: Arc<datastore::postgres::PostgresDatastore>,
    _broker: Arc<dyn Broker>,
    terminate_tx: watch::Sender<bool>,
}

impl TestEnv {
    /// Creates a new test environment connected to the real PostgreSQL database.
    async fn new() -> Self {
        let dsn = "host=localhost user=tork password=tork dbname=tork port=5432 sslmode=disable";
        let options = datastore::postgres::Options {
            disable_cleanup: true,
            ..datastore::postgres::Options::default()
        };
        let ds = datastore::postgres::PostgresDatastore::new(dsn, options)
            .await
            .expect("failed to connect to postgres for integration tests");

        let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

        let (terminate_tx, _terminate_rx) = watch::channel(false);

        Self {
            ds: Arc::new(ds),
            _broker: broker,
            terminate_tx,
        }
    }

    /// Truncates all core tables to prevent state leakage between tests.
    async fn cleanup(&self) {
        let pool = self.ds.pool();
        sqlx::query("TRUNCATE tasks_log_parts, tasks, jobs, nodes, scheduled_jobs CASCADE")
            .execute(pool)
            .await
            .expect("failed to truncate tables for integration test cleanup");
    }
}

/// Create an app state for testing.
fn create_test_state(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> AppState {
    AppState::new(
        broker,
        ds,
        coordinator::api::Config::default(),
    )
}

/// Make a request to the router and return the response bytes and status.
async fn make_request(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<String>,
) -> (StatusCode, Vec<u8>) {
    let mut req = axum::http::Request::builder().method(method).uri(uri);

    if let Some(ref b) = body {
        req = req.header(header::CONTENT_TYPE, "application/json");
    }

    let req = req.body(Body::empty()).unwrap();

    let mut svc = router.clone();
    let mut res = svc.call(req).await.expect("request failed");

    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");

    (status, body.to_vec())
}

/// Make a JSON POST request and return the response.
async fn make_json_request(router: &Router, uri: &str, json_body: &str) -> (StatusCode, Vec<u8>) {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json_body.to_string()))
        .unwrap();

    let mut svc = router.clone();
    let mut res = svc.call(req).await.expect("request failed");

    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");

    (status, body.to_vec())
}

/// Make a JSON PUT request and return the response.
async fn make_json_put_request(
    router: &Router,
    uri: &str,
    json_body: Option<&str>,
) -> (StatusCode, Vec<u8>) {
    let mut req_builder = axum::http::Request::builder().method("PUT").uri(uri);

    if let Some(b) = json_body {
        req_builder = req_builder.header(header::CONTENT_TYPE, "application/json");
        req_builder = req_builder.body(Body::from(b.to_string())).unwrap();
    } else {
        req_builder = req_builder.body(Body::empty()).unwrap();
    }

    let mut svc = router.clone();
    let mut res = svc.call(req_builder).await.expect("request failed");

    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");

    (status, body.to_vec())
}

/// Make a DELETE request and return the response.
async fn make_delete_request(router: &Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let req = axum::http::Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    let mut svc = router.clone();
    let mut res = svc.call(req).await.expect("request failed");

    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .expect("failed to read body");

    (status, body.to_vec())
}

// ---------------------------------------------------------------------------
// Tests: GET /health
// ---------------------------------------------------------------------------

/// Go parity: Test_healthOK
#[tokio::test]
async fn test_health_ok() {
    let env = TestEnv::new().await;
    let state = create_test_state(env.ds.clone(), env._broker.clone());
    let router = create_router(state);

    let (status, body) = make_request(&router, Method::GET, "/health", None).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert!(body_str.contains("\"status\":\"UP\""));

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /queues
// ---------------------------------------------------------------------------

/// Go parity: Test_getQueues
#[tokio::test]
async fn test_get_queues() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Subscribe to a queue to create it
    let qname = "some-queue".to_string();
    let handler: tork::broker::TaskHandler = Arc::new(|_task| Box::pin(async {}));
    broker
        .subscribe_for_tasks(qname.clone(), handler)
        .await
        .expect("subscribe");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) = make_request(&router, Method::GET, "/queues", None).await;

    assert_eq!(status, StatusCode::OK);
    let queues: Vec<QueueInfo> = serde_json::from_slice(&body).expect("parse queues");
    assert_eq!(queues.len(), 1);

    env.cleanup().await;
}

/// Go parity: Test_getQueue
#[tokio::test]
async fn test_get_queue() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Subscribe to a queue to create it
    let qname = "some-queue".to_string();
    let handler: tork::broker::TaskHandler = Arc::new(|_task| Box::pin(async {}));
    broker
        .subscribe_for_tasks(qname.clone(), handler)
        .await
        .expect("subscribe");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) = make_request(&router, Method::GET, "/queues/some-queue", None).await;

    assert_eq!(status, StatusCode::OK);
    let queue: QueueInfo = serde_json::from_slice(&body).expect("parse queue");
    assert_eq!(queue.name, "some-queue");

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /jobs
// ---------------------------------------------------------------------------

/// Go parity: Test_listJobs
#[tokio::test]
async fn test_list_jobs() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Create 101 jobs
    for i in 0..101 {
        let job = Job {
            id: Some(new_uuid()),
            name: Some(format!("job-{}", i)),
            state: JOB_STATE_PENDING.to_string(),
            ..Default::default()
        };
        env.ds.create_job(job).await.expect("create job");
    }

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    // First page - default (page=1, size=10)
    let (status, body) = make_request(&router, Method::GET, "/jobs", None).await;
    assert_eq!(status, StatusCode::OK);

    #[derive(Debug, Deserialize)]
    struct Page {
        size: i64,
        total_pages: i64,
        number: i64,
    }
    let page: Page<Job> = serde_json::from_slice(&body).expect("parse page");
    assert_eq!(page.size, 10);
    assert_eq!(page.total_pages, 11);
    assert_eq!(page.number, 1);

    // Last page (page=11)
    let (status, body) = make_request(&router, Method::GET, "/jobs?page=11", None).await;
    assert_eq!(status, StatusCode::OK);
    let page: Page<Job> = serde_json::from_slice(&body).expect("parse page");
    assert_eq!(page.size, 1);
    assert_eq!(page.total_pages, 11);
    assert_eq!(page.number, 11);

    // Custom size (page=1&size=50)
    let (status, body) = make_request(&router, Method::GET, "/jobs?page=1&size=50", None).await;
    assert_eq!(status, StatusCode::OK);
    let page: Page<Job> = serde_json::from_slice(&body).expect("parse page");
    assert_eq!(page.size, 20); // clamped to max 20
    assert_eq!(page.total_pages, 6);
    assert_eq!(page.number, 1);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /nodes
// ---------------------------------------------------------------------------

/// Go parity: Test_getActiveNodes
#[tokio::test]
async fn test_get_active_nodes() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Create an active node
    let active = Node {
        id: Some("1234".to_string()),
        name: Some("active-node".to_string()),
        last_heartbeat_at: OffsetDateTime::now_utc(),
        status: NODE_STATUS_UP.to_string(),
        hostname: Some("localhost".to_string()),
        cpu_percent: 0.0,
        started_at: OffsetDateTime::now_utc(),
        queue: None,
        port: 8080,
        task_count: 0,
        version: "1.0.0".to_string(),
    };
    env.ds
        .create_node(active)
        .await
        .expect("create active node");

    // Create an inactive node (heartbeat 1 hour ago)
    let inactive = Node {
        id: Some("2345".to_string()),
        name: Some("inactive-node".to_string()),
        last_heartbeat_at: OffsetDateTime::now_utc() - time::Duration::hours(1),
        status: NODE_STATUS_UP.to_string(),
        hostname: Some("localhost".to_string()),
        cpu_percent: 0.0,
        started_at: OffsetDateTime::now_utc(),
        queue: None,
        port: 8080,
        task_count: 0,
        version: "1.0.0".to_string(),
    };
    env.ds
        .create_node(inactive)
        .await
        .expect("create inactive node");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) = make_request(&router, Method::GET, "/nodes", None).await;

    assert_eq!(status, StatusCode::OK);
    let nodes: Vec<Node> = serde_json::from_slice(&body).expect("parse nodes");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id.as_deref(), Some("1234"));

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /tasks/{id}
// ---------------------------------------------------------------------------

/// Go parity: Test_getUnknownTask
#[tokio::test]
async fn test_get_unknown_task() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) = make_request(&router, Method::GET, "/tasks/1", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    env.cleanup().await;
}

/// Go parity: Test_getTask
#[tokio::test]
async fn test_get_task() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Create a job and task
    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        name: Some("test job".to_string()),
        tags: vec!["tag-a".to_string(), "tag-b".to_string()],
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let now = OffsetDateTime::now_utc();
    let task_id = new_uuid();
    let task = Task {
        id: Some(task_id.clone()),
        name: Some("test task".to_string()),
        created_at: Some(now),
        job_id: Some(job_id.clone()),
        state: TASK_STATE_PENDING.clone(),
        ..Default::default()
    };
    env.ds.create_task(task).await.expect("create task");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_request(&router, Method::GET, &format!("/tasks/{}", task_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let returned_task: Task = serde_json::from_slice(&body).expect("parse task");
    assert_eq!(returned_task.id.as_deref(), Some(&task_id));
    assert_eq!(returned_task.name.as_deref(), Some("test task"));

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: POST /jobs
// ---------------------------------------------------------------------------

/// Go parity: Test_createJob
#[tokio::test]
async fn test_create_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker.clone());
    let router = create_router(state);

    let json_body = json!({
        "name": "test job",
        "tasks": [{
            "name": "test task",
            "image": "some:image"
        }]
    });

    let (status, body) = make_json_request(&router, "/jobs", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::OK);
    let result: serde_json::Value = serde_json::from_slice(&body).expect("parse response");
    assert_eq!(result["state"], "PENDING");

    env.cleanup().await;
}

/// Go parity: Test_createJobInvalidProperty
#[tokio::test]
async fn test_create_job_invalid_property() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker.clone());
    let router = create_router(state);

    // Invalid JSON - unknown property "nosuch"
    let json_body = json!({
        "tasks": [{
            "nosuch": "thing",
            "image": "some:image"
        }]
    });

    let (status, _body) = make_json_request(&router, "/jobs", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /jobs/{id}
// ---------------------------------------------------------------------------

/// Go parity: Test_getJob
#[tokio::test]
async fn test_get_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        name: Some("test job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_request(&router, Method::GET, &format!("/jobs/{}", job_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let returned_job: Job = serde_json::from_slice(&body).expect("parse job");
    assert_eq!(returned_job.id.as_deref(), Some(&job_id));
    assert_eq!(returned_job.state, *JOB_STATE_PENDING);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: PUT /jobs/{id}/cancel
// ---------------------------------------------------------------------------

/// Go parity: Test_cancelRunningJob
#[tokio::test]
async fn test_cancel_running_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        state: JOB_STATE_RUNNING.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let now = OffsetDateTime::now_utc();
    let states = vec![
        TASK_STATE_PENDING.clone(),
        TASK_STATE_SCHEDULED.clone(),
        TASK_STATE_RUNNING.clone(),
    ];
    for (i, state) in states.iter().enumerate() {
        let task = Task {
            id: Some(new_uuid()),
            state: state.clone(),
            created_at: Some(now),
            job_id: Some(job_id.clone()),
            ..Default::default()
        };
        env.ds.create_task(task).await.expect("create task");
    }

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_json_put_request(&router, &format!("/jobs/{}/cancel", job_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "{\"status\":\"OK\"}");

    env.cleanup().await;
}

/// Go parity: Test_cancelScheduledJob
#[tokio::test]
async fn test_cancel_scheduled_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        state: JOB_STATE_SCHEDULED.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let now = OffsetDateTime::now_utc();
    let states = vec![
        TASK_STATE_PENDING.clone(),
        TASK_STATE_SCHEDULED.clone(),
        TASK_STATE_RUNNING.clone(),
    ];
    for state in states.iter() {
        let task = Task {
            id: Some(new_uuid()),
            state: state.clone(),
            created_at: Some(now),
            job_id: Some(job_id.clone()),
            ..Default::default()
        };
        env.ds.create_task(task).await.expect("create task");
    }

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_json_put_request(&router, &format!("/jobs/{}/cancel", job_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "{\"status\":\"OK\"}");

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: PUT /jobs/{id}/restart
// ---------------------------------------------------------------------------

/// Go parity: Test_restartJob
#[tokio::test]
async fn test_restart_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        state: JOB_STATE_CANCELLED.to_string(),
        created_at: OffsetDateTime::now_utc(),
        position: 1,
        tasks: vec![tork::task::Task {
            name: Some("some fake task".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_json_put_request(&router, &format!("/jobs/{}/restart", job_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "{\"status\":\"OK\"}");

    env.cleanup().await;
}

/// Go parity: Test_restartRunningJob (expects BAD_REQUEST)
#[tokio::test]
async fn test_restart_running_job_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        state: JOB_STATE_RUNNING.to_string(),
        created_at: OffsetDateTime::now_utc(),
        position: 1,
        tasks: vec![tork::task::Task {
            name: Some("some fake task".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) =
        make_json_put_request(&router, &format!("/jobs/{}/restart", job_id), None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

/// Go parity: Test_restartRunningNoMoreTasksJob (expects BAD_REQUEST)
#[tokio::test]
async fn test_restart_no_more_tasks_job_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        state: JOB_STATE_FAILED.to_string(),
        created_at: OffsetDateTime::now_utc(),
        position: 2, // position > tasks.len()
        tasks: vec![tork::task::Task {
            name: Some("some fake task".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) =
        make_json_put_request(&router, &format!("/jobs/{}/restart", job_id), None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: DELETE /scheduled-jobs/{id}
// ---------------------------------------------------------------------------

/// Go parity: Test_deleteScheduledJob
#[tokio::test]
async fn test_delete_scheduled_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let sj_id = new_uuid();
    let sj = tork::job::ScheduledJob {
        id: Some(sj_id.clone()),
        state: tork::job::SCHEDULED_JOB_STATE_ACTIVE.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds
        .create_scheduled_job(sj)
        .await
        .expect("create scheduled job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) = make_delete_request(&router, &format!("/scheduled-jobs/{}", sj_id)).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "{\"status\":\"OK\"}");

    // Verify the scheduled job is deleted (GetScheduledJobByID should return None)
    let result = env
        .ds
        .get_scheduled_job_by_id(sj_id)
        .await
        .expect("get scheduled job");
    assert!(result.is_none());

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: Disabled endpoint
// ---------------------------------------------------------------------------

/// Go parity: Test_disableEndpoint
#[tokio::test]
async fn test_disable_endpoint() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let (terminate_rx, _terminate_tx) = watch::channel(false);
    let mut config = coordinator::api::Config::default();
    config.enabled.insert("health".to_string(), false);

    let state = AppState::new(broker, env.ds.clone(), config);
    let router = create_router(state);

    let (status, _body) = make_request(&router, Method::GET, "/health", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /tasks/{id}/log
// ---------------------------------------------------------------------------

/// Go parity: Test_getTaskLog
#[tokio::test]
async fn test_get_task_log() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Create a job and task
    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        name: Some("test job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    let task_id = new_uuid();
    let task = Task {
        id: Some(task_id.clone()),
        name: Some("test task".to_string()),
        created_at: Some(OffsetDateTime::now_utc()),
        job_id: Some(job_id.clone()),
        state: TASK_STATE_PENDING.clone(),
        ..Default::default()
    };
    env.ds.create_task(task).await.expect("create task");

    // Add log parts
    let log_part = tork::task::TaskLogPart {
        id: Some(new_uuid()),
        job_id: Some(job_id.clone()),
        task_id: Some(task_id.clone()),
        number: 1,
        contents: "log line 1".to_string(),
        created_at: Some(OffsetDateTime::now_utc()),
        ..Default::default()
    };
    env.ds
        .append_task_log_part(log_part)
        .await
        .expect("append log part");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_request(&router, Method::GET, &format!("/tasks/{}/log", task_id), None).await;

    assert_eq!(status, StatusCode::OK);

    env.cleanup().await;
}

/// Go parity: Test_getTaskLogUnknownTask
#[tokio::test]
async fn test_get_task_log_unknown_task() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) =
        make_request(&router, Method::GET, "/tasks/unknown-id/log", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /jobs/{id}/log
// ---------------------------------------------------------------------------

/// Go parity: Test_getJobLog
#[tokio::test]
async fn test_get_job_log() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let job_id = new_uuid();
    let job = Job {
        id: Some(job_id.clone()),
        name: Some("test job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };
    env.ds.create_job(job).await.expect("create job");

    // Add log parts
    let log_part = tork::task::TaskLogPart {
        id: Some(new_uuid()),
        job_id: Some(job_id.clone()),
        task_id: None,
        number: 1,
        contents: "job log line".to_string(),
        created_at: Some(OffsetDateTime::now_utc()),
        ..Default::default()
    };
    env.ds
        .append_job_log_part(log_part)
        .await
        .expect("append log part");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_request(&router, Method::GET, &format!("/jobs/{}/log", job_id), None).await;

    assert_eq!(status, StatusCode::OK);

    env.cleanup().await;
}

/// Go parity: Test_getJobLogUnknownJob
#[tokio::test]
async fn test_get_job_log_unknown_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) =
        make_request(&router, Method::GET, "/jobs/unknown-id/log", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: POST /scheduled-jobs
// ---------------------------------------------------------------------------

/// Go parity: Test_createScheduledJob
#[tokio::test]
async fn test_create_scheduled_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker.clone());
    let router = create_router(state);

    let json_body = json!({
        "name": "test scheduled job",
        "cron": "0 * * * *",
        "tasks": [{
            "name": "test task",
            "image": "some:image"
        }]
    });

    let (status, body) =
        make_json_request(&router, "/scheduled-jobs", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::OK);
    let result: serde_json::Value = serde_json::from_slice(&body).expect("parse response");
    assert_eq!(result["state"], "ACTIVE");

    env.cleanup().await;
}

/// Go parity: Test_createScheduledJobInvalidCron
#[tokio::test]
async fn test_create_scheduled_job_invalid_cron() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker.clone());
    let router = create_router(state);

    let json_body = json!({
        "name": "test scheduled job",
        "cron": "not-a-cron",
        "tasks": [{
            "name": "test task",
            "image": "some:image"
        }]
    });

    let (status, _body) =
        make_json_request(&router, "/scheduled-jobs", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /scheduled-jobs
// ---------------------------------------------------------------------------

/// Go parity: Test_listScheduledJobs
#[tokio::test]
async fn test_list_scheduled_jobs() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Create scheduled jobs
    for i in 0..5 {
        let sj = tork::job::ScheduledJob {
            id: Some(new_uuid()),
            name: Some(format!("scheduled-job-{}", i)),
            state: tork::job::SCHEDULED_JOB_STATE_ACTIVE.to_string(),
            created_at: OffsetDateTime::now_utc(),
            ..Default::default()
        };
        env.ds.create_scheduled_job(sj).await.expect("create scheduled job");
    }

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) = make_request(&router, Method::GET, "/scheduled-jobs", None).await;

    assert_eq!(status, StatusCode::OK);

    #[derive(Debug, Deserialize)]
    struct Page {
        size: i64,
        total_pages: i64,
        number: i64,
    }
    let page: Page<tork::job::ScheduledJob> =
        serde_json::from_slice(&body).expect("parse page");
    assert_eq!(page.size, 10);
    assert_eq!(page.number, 1);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /scheduled-jobs/{id}
// ---------------------------------------------------------------------------

/// Go parity: Test_getScheduledJob
#[tokio::test]
async fn test_get_scheduled_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let sj_id = new_uuid();
    let sj = tork::job::ScheduledJob {
        id: Some(sj_id.clone()),
        name: Some("test scheduled job".to_string()),
        state: tork::job::SCHEDULED_JOB_STATE_ACTIVE.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds.create_scheduled_job(sj).await.expect("create scheduled job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_request(&router, Method::GET, &format!("/scheduled-jobs/{}", sj_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let returned: tork::job::ScheduledJob =
        serde_json::from_slice(&body).expect("parse scheduled job");
    assert_eq!(returned.id.as_deref(), Some(&sj_id));

    env.cleanup().await;
}

/// Go parity: Test_getUnknownScheduledJob
#[tokio::test]
async fn test_get_unknown_scheduled_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) =
        make_request(&router, Method::GET, "/scheduled-jobs/unknown-id", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: PUT /scheduled-jobs/{id}/pause
// ---------------------------------------------------------------------------

/// Go parity: Test_pauseScheduledJob
#[tokio::test]
async fn test_pause_scheduled_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let sj_id = new_uuid();
    let sj = tork::job::ScheduledJob {
        id: Some(sj_id.clone()),
        name: Some("test scheduled job".to_string()),
        state: tork::job::SCHEDULED_JOB_STATE_ACTIVE.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds.create_scheduled_job(sj).await.expect("create scheduled job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_json_put_request(&router, &format!("/scheduled-jobs/{}/pause", sj_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "{\"status\":\"OK\"}");

    // Verify state changed to PAUSED
    let sj = env
        .ds
        .get_scheduled_job_by_id(sj_id)
        .await
        .expect("get scheduled job");
    assert_eq!(sj.unwrap().state, tork::job::SCHEDULED_JOB_STATE_PAUSED.to_string());

    env.cleanup().await;
}

/// Go parity: Test_pauseNonActiveScheduledJob (expects BAD_REQUEST)
#[tokio::test]
async fn test_pause_non_active_scheduled_job_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let sj_id = new_uuid();
    let sj = tork::job::ScheduledJob {
        id: Some(sj_id.clone()),
        name: Some("test scheduled job".to_string()),
        state: tork::job::SCHEDULED_JOB_STATE_PAUSED.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds.create_scheduled_job(sj).await.expect("create scheduled job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) =
        make_json_put_request(&router, &format!("/scheduled-jobs/{}/pause", sj_id), None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: PUT /scheduled-jobs/{id}/resume
// ---------------------------------------------------------------------------

/// Go parity: Test_resumeScheduledJob
#[tokio::test]
async fn test_resume_scheduled_job() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let sj_id = new_uuid();
    let sj = tork::job::ScheduledJob {
        id: Some(sj_id.clone()),
        name: Some("test scheduled job".to_string()),
        state: tork::job::SCHEDULED_JOB_STATE_PAUSED.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds.create_scheduled_job(sj).await.expect("create scheduled job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) =
        make_json_put_request(&router, &format!("/scheduled-jobs/{}/resume", sj_id), None).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "{\"status\":\"OK\"}");

    // Verify state changed to ACTIVE
    let sj = env
        .ds
        .get_scheduled_job_by_id(sj_id)
        .await
        .expect("get scheduled job");
    assert_eq!(
        sj.unwrap().state,
        tork::job::SCHEDULED_JOB_STATE_ACTIVE.to_string()
    );

    env.cleanup().await;
}

/// Go parity: Test_resumeNonPausedScheduledJob (expects BAD_REQUEST)
#[tokio::test]
async fn test_resume_non_paused_scheduled_job_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let sj_id = new_uuid();
    let sj = tork::job::ScheduledJob {
        id: Some(sj_id.clone()),
        name: Some("test scheduled job".to_string()),
        state: tork::job::SCHEDULED_JOB_STATE_ACTIVE.to_string(),
        created_at: OffsetDateTime::now_utc(),
        ..Default::default()
    };
    env.ds.create_scheduled_job(sj).await.expect("create scheduled job");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) =
        make_json_put_request(&router, &format!("/scheduled-jobs/{}/resume", sj_id), None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: DELETE /queues/{name}
// ---------------------------------------------------------------------------

/// Go parity: Test_deleteTaskQueue
#[tokio::test]
async fn test_delete_task_queue() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Subscribe to a queue to create it (task queues start with "tasks.")
    let qname = "tasks.some-queue".to_string();
    let handler: tork::broker::TaskHandler = Arc::new(|_task| Box::pin(async {}));
    broker
        .subscribe_for_tasks(qname.clone(), handler)
        .await
        .expect("subscribe");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) = make_delete_request(&router, &format!("/queues/{}", qname)).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "");

    env.cleanup().await;
}

/// Go parity: Test_deleteNonTaskQueue (expects BAD_REQUEST)
#[tokio::test]
async fn test_delete_non_task_queue_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Subscribe to a non-task queue (doesn't start with "tasks.")
    let qname = "coordinator.events".to_string();
    let handler: tork::broker::TaskHandler = Arc::new(|_task| Box::pin(async {}));
    broker
        .subscribe_for_tasks(qname.clone(), handler)
        .await
        .expect("subscribe");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, _body) = make_delete_request(&router, &format!("/queues/{}", qname)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: GET /metrics
// ---------------------------------------------------------------------------

/// Go parity: Test_getMetrics
#[tokio::test]
async fn test_get_metrics() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let (status, body) = make_request(&router, Method::GET, "/metrics", None).await;

    assert_eq!(status, StatusCode::OK);
    let metrics: serde_json::Value = serde_json::from_slice(&body).expect("parse metrics");
    // Should have numeric fields
    assert!(metrics.get("jobs_total").is_some());

    env.cleanup().await;
}

// ---------------------------------------------------------------------------
// Tests: POST /users
// ---------------------------------------------------------------------------

/// Go parity: Test_createUser
#[tokio::test]
async fn test_create_user() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let json_body = json!({
        "username": "testuser",
        "password": "testpassword"
    });

    let (status, body) = make_json_request(&router, "/users", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::OK);
    let body_str = String::from_utf8(body).expect("invalid UTF-8");
    assert_eq!(body_str, "");

    env.cleanup().await;
}

/// Go parity: Test_createDuplicateUser (expects BAD_REQUEST)
#[tokio::test]
async fn test_create_duplicate_user_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    // Create first user
    let user = tork::User {
        username: Some("testuser".to_string()),
        password_hash: Some("$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYIq8z diagnost".to_string()),
        created_at: Some(OffsetDateTime::now_utc()),
        ..Default::default()
    };
    env.ds.create_user(user).await.expect("create user");

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let json_body = json!({
        "username": "testuser",
        "password": "anotherpassword"
    });

    let (status, _body) = make_json_request(&router, "/users", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

/// Go parity: Test_createUserMissingUsername (expects BAD_REQUEST)
#[tokio::test]
async fn test_create_user_missing_username_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let json_body = json!({
        "password": "somepassword"
    });

    let (status, _body) = make_json_request(&router, "/users", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}

/// Go parity: Test_createUserMissingPassword (expects BAD_REQUEST)
#[tokio::test]
async fn test_create_user_missing_password_error() {
    let env = TestEnv::new().await;
    let broker: Arc<dyn Broker> = Arc::new(new_in_memory_broker());

    let state = create_test_state(env.ds.clone(), broker);
    let router = create_router(state);

    let json_body = json!({
        "username": "testuser"
    });

    let (status, _body) = make_json_request(&router, "/users", &json_body.to_string()).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);

    env.cleanup().await;
}
