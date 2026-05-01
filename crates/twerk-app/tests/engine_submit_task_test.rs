#![allow(clippy::unwrap_used)]

use anyhow::Result;
use std::sync::Arc;
use twerk_app::engine::{BrokerProxy, Config, DatastoreProxy, Engine, Mode, State, SubmitTaskError};
use twerk_core::id::TaskId;
use twerk_core::task::Task;
use twerk_infrastructure::broker::{inmemory::InMemoryBroker, Broker};
use twerk_infrastructure::datastore::{inmemory::InMemoryDatastore, Datastore};

fn engine_with_mode(mode: Mode) -> Engine {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    Engine::new(Config {
        mode,
        ..Default::default()
    })
}

fn to_task_id(value: impl Into<String>) -> TaskId {
    TaskId::new(value).expect("test task id should be valid")
}

#[tokio::test]
async fn engine_submit_task_returns_error_when_engine_not_running() -> Result<()> {
    let engine = engine_with_mode(Mode::Standalone);
    let task = Task {
        name: Some("test task".to_string()),
        image: Some("alpine".to_string()),
        run: Some("echo hello".to_string()),
        ..Default::default()
    };

    let result = engine.submit_task(task).await;
    assert!(matches!(result, Err(SubmitTaskError::NotRunning)));
    Ok(())
}

#[tokio::test]
async fn engine_submit_task_returns_valid_task_handle() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    let task = Task {
        name: Some("test task".to_string()),
        image: Some("alpine".to_string()),
        run: Some("echo hello".to_string()),
        ..Default::default()
    };

    let result = engine.submit_task(task).await;
    assert!(matches!(result, Ok(_)));

    let handle = result.unwrap();
    assert!(handle.task_id.to_string().len() > 0, "task_id should not be empty");

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn engine_submit_task_appears_in_pending_queue() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    let queue_name = "test-queue";
    let task = Task {
        name: Some("queued task".to_string()),
        image: Some("alpine".to_string()),
        run: Some("echo hello".to_string()),
        queue: Some(queue_name.to_string()),
        ..Default::default()
    };

    let handle = engine.submit_task(task).await?;

    let broker = engine.broker_proxy();
    let queues = broker.queues().await?;

    let queue_info = broker.queue_info(queue_name.to_string()).await?;
    assert!(queue_info.message_count >= 1, "queue should have at least 1 message");

    engine.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn engine_submit_task_rejects_duplicate_task_id() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    let mut engine = engine_with_mode(Mode::Standalone);
    engine.start().await?;

    let task_id = to_task_id("duplicate-task-123");
    let task1 = Task {
        id: Some(task_id.clone()),
        name: Some("task 1".to_string()),
        image: Some("alpine".to_string()),
        run: Some("echo first".to_string()),
        ..Default::default()
    };

    let result1 = engine.submit_task(task1).await;
    assert!(result1.is_ok(), "first submission should succeed");

    let task2 = Task {
        id: Some(task_id.clone()),
        name: Some("task 2".to_string()),
        image: Some("alpine".to_string()),
        run: Some("echo second".to_string()),
        ..Default::default()
    };

    let result2 = engine.submit_task(task2).await;
    assert!(
        matches!(result2, Err(SubmitTaskError::DuplicateTaskId(id)) if id == task_id),
        "duplicate task_id should be rejected"
    );

    engine.terminate().await?;
    Ok(())
}