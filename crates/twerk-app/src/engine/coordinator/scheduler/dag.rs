//! DAG (Directed Acyclic Graph) dependency tests for the scheduler.
//!
//! Tests verify that:
//! 1. Tasks with dependencies wait for their dependencies to complete
//! 2. Failure propagates through the dependency chain
//! 3. Circular dependencies are rejected at submit time
//!
//! # DAG Structure for Tests
//!
//! ```text
//!     A
//!     │
//!     ▼
//!     B
//!     │
//!     ▼
//!     C
//! ```
//!
//! A -> B means B depends on A (B cannot run until A completes)

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::mock::{create_test_job, create_test_task, FakeDatastore};
use super::Scheduler;
use std::sync::Arc;
use twerk_core::id::TaskId;
use twerk_core::task::{Task, TaskState};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;

fn create_task_with_id(id: &str) -> Task {
    let mut task = create_test_task();
    task.id = Some(TaskId::new(id).unwrap());
    task
}

fn create_task_with_deps(id: &str, depends_on: Vec<&str>) -> Task {
    let mut task = create_task_with_id(id);
    task.depends_on = Some(depends_on.iter().map(|s| TaskId::new(*s).unwrap()).collect());
    task
}

#[tokio::test]
async fn dag_submit_rejects_circular_dependency_a_depends_on_b_which_depends_on_a() {
    let ds = Arc::new(FakeDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let task_a = create_task_with_deps("00000000-0000-0000-0000-000000000001", vec!["00000000-0000-0000-0000-000000000003"]);
    let task_b = create_task_with_deps("00000000-0000-0000-0000-000000000002", vec!["00000000-0000-0000-0000-000000000001"]);
    let task_c = create_task_with_deps("00000000-0000-0000-0000-000000000003", vec!["00000000-0000-0000-0000-000000000002"]);

    ds.tasks.insert(task_a.id.clone().unwrap(), task_a.clone());
    ds.tasks.insert(task_b.id.clone().unwrap(), task_b.clone());
    ds.tasks.insert(task_c.id.clone().unwrap(), task_c.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    let result = scheduler.submit_dag(vec![task_a, task_b, task_c]).await;

    assert!(
        result.is_err(),
        "circular dependency A->B->C->A should be rejected at submit time"
    );
    let err = result.as_ref().unwrap_err();
    assert!(
        err.to_string().contains("circular"),
        "error should mention circular dependency: {:?}",
        err
    );
}

#[tokio::test]
async fn dag_submit_rejects_direct_cycle_a_depends_on_b_which_depends_on_a() {
    let ds = Arc::new(FakeDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let task_a = create_task_with_deps("00000000-0000-0000-0000-000000000001", vec!["00000000-0000-0000-0000-000000000002"]);
    let task_b = create_task_with_deps("00000000-0000-0000-0000-000000000002", vec!["00000000-0000-0000-0000-000000000001"]);

    ds.tasks.insert(task_a.id.clone().unwrap(), task_a.clone());
    ds.tasks.insert(task_b.id.clone().unwrap(), task_b.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));
    let result = scheduler.submit_dag(vec![task_a, task_b]).await;

    assert!(
        result.is_err(),
        "direct cycle A->B->A should be rejected"
    );
}

#[tokio::test]
async fn dag_b_waits_for_a_when_a_completes_before_b() {
    let ds = Arc::new(FakeDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task_a = create_task_with_deps("00000000-0000-0000-0000-000000000001", vec![]);
    let mut task_b = create_task_with_deps("00000000-0000-0000-0000-000000000002", vec!["00000000-0000-0000-0000-000000000001"]);

    ds.tasks.insert(task_a.id.clone().unwrap(), task_a.clone());
    ds.tasks.insert(task_b.id.clone().unwrap(), task_b.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));

    scheduler.submit_dag(vec![task_a.clone(), task_b.clone()]).await.unwrap();

    let state_a = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000001").unwrap()).unwrap();
    let state_b = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000002").unwrap()).unwrap();

    assert_eq!(
        state_a.state,
        TaskState::Scheduled,
        "A should be scheduled (no dependencies)"
    );
    assert_eq!(
        state_b.state,
        TaskState::Pending,
        "B should be Pending (waiting for A to complete)"
    );
}

#[tokio::test]
async fn dag_c_waits_for_b_when_b_waits_for_a() {
    let ds = Arc::new(FakeDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task_a = create_task_with_deps("00000000-0000-0000-0000-000000000001", vec![]);
    let mut task_b = create_task_with_deps("00000000-0000-0000-0000-000000000002", vec!["00000000-0000-0000-0000-000000000001"]);
    let mut task_c = create_task_with_deps("00000000-0000-0000-0000-000000000003", vec!["00000000-0000-0000-0000-000000000002"]);

    ds.tasks.insert(task_a.id.clone().unwrap(), task_a.clone());
    ds.tasks.insert(task_b.id.clone().unwrap(), task_b.clone());
    ds.tasks.insert(task_c.id.clone().unwrap(), task_c.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));

    scheduler.submit_dag(vec![task_a.clone(), task_b.clone(), task_c.clone()]).await.unwrap();

    let state_a = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000001").unwrap()).unwrap();
    let state_b = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000002").unwrap()).unwrap();
    let state_c = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()).unwrap();

    assert_eq!(state_a.state, TaskState::Scheduled, "A should be Scheduled");
    assert_eq!(state_b.state, TaskState::Pending, "B should be Pending (waiting for A)");
    assert_eq!(state_c.state, TaskState::Pending, "C should be Pending (waiting for B)");
}

#[tokio::test]
async fn dag_when_a_fails_b_and_c_are_cancelled() {
    let ds = Arc::new(FakeDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task_a = create_task_with_deps("00000000-0000-0000-0000-000000000001", vec![]);
    let mut task_b = create_task_with_deps("00000000-0000-0000-0000-000000000002", vec!["00000000-0000-0000-0000-000000000001"]);
    let mut task_c = create_task_with_deps("00000000-0000-0000-0000-000000000003", vec!["00000000-0000-0000-0000-000000000002"]);

    ds.tasks.insert(task_a.id.clone().unwrap(), task_a.clone());
    ds.tasks.insert(task_b.id.clone().unwrap(), task_b.clone());
    ds.tasks.insert(task_c.id.clone().unwrap(), task_c.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));

    scheduler.submit_dag(vec![task_a.clone(), task_b.clone(), task_c.clone()]).await.unwrap();

    scheduler.mark_task_failed(&TaskId::new("00000000-0000-0000-0000-000000000001").unwrap()).await.unwrap();

    let state_a = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000001").unwrap()).unwrap();
    let state_b = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000002").unwrap()).unwrap();
    let state_c = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()).unwrap();

    assert_eq!(state_a.state, TaskState::Failed, "A should be Failed");
    assert_eq!(
        state_b.state,
        TaskState::Cancelled,
        "B should be Cancelled when A fails"
    );
    assert_eq!(
        state_c.state,
        TaskState::Cancelled,
        "C should be Cancelled when B's dependency fails"
    );
}

#[tokio::test]
async fn dag_topological_order_respected_across_multiple_levels() {
    let ds = Arc::new(FakeDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task_a = create_task_with_deps("00000000-0000-0000-0000-000000000001", vec![]);
    let mut task_b = create_task_with_deps("00000000-0000-0000-0000-000000000002", vec!["00000000-0000-0000-0000-000000000001"]);
    let mut task_c = create_task_with_deps("00000000-0000-0000-0000-000000000003", vec!["00000000-0000-0000-0000-000000000002"]);
    let mut task_d = create_task_with_deps("00000000-0000-0000-0000-000000000004", vec!["00000000-0000-0000-0000-000000000003"]);

    ds.tasks.insert(task_a.id.clone().unwrap(), task_a.clone());
    ds.tasks.insert(task_b.id.clone().unwrap(), task_b.clone());
    ds.tasks.insert(task_c.id.clone().unwrap(), task_c.clone());
    ds.tasks.insert(task_d.id.clone().unwrap(), task_d.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));

    scheduler.submit_dag(vec![task_d.clone(), task_c.clone(), task_b.clone(), task_a.clone()]).await.unwrap();

    let state_a = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000001").unwrap()).unwrap();
    let state_b = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000002").unwrap()).unwrap();
    let state_c = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()).unwrap();
    let state_d = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000004").unwrap()).unwrap();

    assert_eq!(state_a.state, TaskState::Scheduled, "A should be Scheduled");
    assert_eq!(state_b.state, TaskState::Pending, "B should be Pending");
    assert_eq!(state_c.state, TaskState::Pending, "C should be Pending");
    assert_eq!(state_d.state, TaskState::Pending, "D should be Pending");

    assert!(
        state_a.scheduled_at.is_some(),
        "A should have scheduled_at set"
    );
    assert!(
        state_b.scheduled_at.is_none(),
        "B should not have scheduled_at yet"
    );
}

#[tokio::test]
async fn dag_multiple_tasks_can_run_when_no_dependencies() {
    let ds = Arc::new(FakeDatastore::new());
    let broker = InMemoryBroker::new();
    let job = create_test_job();
    ds.jobs.insert(job.id.clone().unwrap(), job);

    let mut task_a = create_task_with_deps("00000000-0000-0000-0000-000000000001", vec![]);
    let mut task_b = create_task_with_deps("00000000-0000-0000-0000-000000000002", vec![]);
    let mut task_c = create_task_with_deps("00000000-0000-0000-0000-000000000003", vec![]);

    ds.tasks.insert(task_a.id.clone().unwrap(), task_a.clone());
    ds.tasks.insert(task_b.id.clone().unwrap(), task_b.clone());
    ds.tasks.insert(task_c.id.clone().unwrap(), task_c.clone());

    let scheduler = Scheduler::new(ds.clone(), Arc::new(broker));

    scheduler.submit_dag(vec![task_a.clone(), task_b.clone(), task_c.clone()]).await.unwrap();

    let state_a = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000001").unwrap()).unwrap();
    let state_b = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000002").unwrap()).unwrap();
    let state_c = ds.tasks.get(&TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()).unwrap();

    assert_eq!(state_a.state, TaskState::Scheduled, "A should be Scheduled");
    assert_eq!(state_b.state, TaskState::Scheduled, "B should be Scheduled");
    assert_eq!(state_c.state, TaskState::Scheduled, "C should be Scheduled");
}