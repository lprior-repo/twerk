use axum::response::Response;
use serde_json::{json, Value};
use std::sync::Arc;
use twerk_core::id::{JobId, TaskId};
use twerk_core::job::{Job, JobState};
use twerk_core::task::{Task, TaskState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;
use twerk_web::api::trigger_api::InMemoryTriggerDatastore;
use twerk_web::api::{create_router, AppState, Config};

pub const JOB_ID: &str = "00000000-0000-0000-0000-000000000010";
pub const RUNNING_JOB_ID: &str = "00000000-0000-0000-0000-000000000001";
pub const DIRECT_TASK_ID: &str = "00000000-0000-0000-0000-000000000002";

pub async fn setup_state() -> AppState {
    crate::support::TestHarness::new().await.into_state()
}

pub async fn setup_state_with_triggers() -> (AppState, Arc<InMemoryTriggerDatastore>) {
    let harness =
        crate::support::TestHarness::with_trigger_ids(&["trg_test_1", "trg_test_2"]).await;
    (harness.clone().into_state(), harness.trigger_store())
}

pub async fn setup_state_with_jobs() -> (AppState, Arc<InMemoryDatastore>) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());
    let job = Job {
        id: Some(JobId::new(JOB_ID).unwrap()),
        name: Some("Test Job".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            id: Some("task-1".into()),
            name: Some("Task 1".to_string()),
            state: TaskState::Pending,
            ..Default::default()
        }]),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();
    (state, ds)
}

pub async fn setup_state_with_tasks() -> (AppState, Arc<InMemoryDatastore>, JobId) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());
    let job_id = JobId::new(RUNNING_JOB_ID).unwrap();
    let job = Job {
        id: Some(job_id.clone()),
        name: Some("Job With Tasks".to_string()),
        state: JobState::Running,
        tasks: Some(vec![Task {
            id: Some("task-1".into()),
            job_id: Some(job_id.clone()),
            name: Some("Task 1".to_string()),
            state: TaskState::Running,
            ..Default::default()
        }]),
        ..Default::default()
    };
    ds.create_job(&job).await.unwrap();
    (state, ds, job_id)
}

pub async fn setup_state_with_direct_task() -> (AppState, Arc<InMemoryDatastore>, TaskId) {
    let ds = Arc::new(InMemoryDatastore::new());
    let broker = Arc::new(InMemoryBroker::new());
    let state = AppState::new(broker, ds.clone(), Config::default());
    let task_id = TaskId::new(DIRECT_TASK_ID).unwrap();
    let task = crate::support::direct_task(
        RUNNING_JOB_ID,
        DIRECT_TASK_ID,
        "Direct Task",
        TaskState::Running,
    );
    ds.create_task(&task).await.unwrap();
    (state, ds, task_id)
}

pub async fn body_to_json(response: Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

pub fn app(state: AppState) -> axum::Router {
    create_router(state)
}

pub fn scheduled_job_input(name: &str) -> Value {
    json!({
        "name": name,
        "cron": "0 0 * * * *",
        "tasks": [{
            "name": "task-1",
            "image": "alpine",
            "run": "echo hello"
        }]
    })
}

pub fn trigger_input(name: &str) -> Value {
    json!({
        "name": name,
        "enabled": true,
        "event": "test.event",
        "action": "test_action"
    })
}

use http_body_util::BodyExt;
