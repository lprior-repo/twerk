#![allow(clippy::unwrap_used)]
#![allow(clippy::uninlined_format_args)]

use anyhow::Result;
use std::sync::Arc;
use twerk_app::engine::coordinator::handlers;
use twerk_app::engine::{BrokerProxy, DatastoreProxy};
use twerk_core::id::NodeId;
use twerk_core::job::{Job, JobState};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TaskLogPart, TaskState};
use twerk_infrastructure::broker::queue::QUEUE_FAILED;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

fn to_ds(ds: &DatastoreProxy) -> Arc<dyn Datastore> {
    Arc::new(ds.clone_inner())
}

fn to_broker(b: &BrokerProxy) -> Arc<dyn Broker> {
    Arc::new(b.clone_inner())
}

async fn setup() -> Result<(DatastoreProxy, BrokerProxy)> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();
    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;
    Ok((datastore, broker))
}

#[tokio::test]
async fn handle_redelivered_requeues_task() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "redeliver-test-job";
    let task_id = "redeliver-test-task";

    let job = Job {
        id: Some(job_id.into()),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(job_id.into()),
        state: TaskState::Running,
        redelivered: 1,
        queue: Some("default".to_string()),
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_redelivered(ds, b, task.clone()).await;
    assert!(result.is_ok());

    let queues = broker.clone_inner().queues().await?;
    let default_queue = queues.iter().find(|q| q.name == "default");
    assert!(default_queue.is_some());
    assert_eq!(default_queue.unwrap().size, 1);

    let updated_task = datastore.clone_inner().get_task_by_id(task_id).await?;
    assert_eq!(updated_task.redelivered, 2);

    Ok(())
}

#[tokio::test]
async fn handle_started_updates_task_state() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "started-job";
    let task_id = "started-task";

    let job = Job {
        id: Some(job_id.into()),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(job_id.into()),
        state: twerk_core::task::TaskState::Scheduled,
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_started(ds, b, task.clone()).await;
    assert!(result.is_ok());

    let updated_task = datastore.clone_inner().get_task_by_id(task_id).await?;
    assert_eq!(updated_task.state, TaskState::Running);
    assert!(updated_task.started_at.is_some());

    Ok(())
}

#[tokio::test]
async fn handle_error_updates_task_state() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "error-job";
    let task_id = "error-task";

    let job = Job {
        id: Some(job_id.into()),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(job_id.into()),
        state: TaskState::Running,
        error: Some("something went wrong".to_string()),
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_error(ds, b, task.clone()).await;
    assert!(result.is_ok());

    let updated_task = datastore.clone_inner().get_task_by_id(task_id).await?;
    assert_eq!(updated_task.state, TaskState::Failed);
    assert!(updated_task.failed_at.is_some());
    assert_eq!(updated_task.error, Some("something went wrong".to_string()));

    Ok(())
}

#[tokio::test]
async fn handle_error_publishes_to_failed_queue() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "error-queue-job";
    let task_id = "error-queue-task";

    let job = Job {
        id: Some(job_id.into()),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(job_id.into()),
        state: TaskState::Running,
        error: Some("task failed".to_string()),
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_error(ds, b, task.clone()).await;
    assert!(result.is_ok());

    let queues = broker.clone_inner().queues().await?;
    let failed_queue = queues.iter().find(|q| q.name == QUEUE_FAILED);
    assert!(failed_queue.is_some());
    assert_eq!(failed_queue.unwrap().size, 1);

    Ok(())
}

#[tokio::test]
async fn handle_heartbeat_updates_node_status() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let now = time::OffsetDateTime::now_utc();
    let node = Node {
        id: Some(NodeId::new("worker-1").unwrap()),
        name: Some("test-worker".to_string()),
        status: Some(NodeStatus::UP),
        last_heartbeat_at: Some(now),
        ..Default::default()
    };

    datastore.clone_inner().create_node(&node).await?;

    let result = handlers::handle_heartbeat(ds, b, node.clone()).await;
    assert!(result.is_ok());

    let updated_node = datastore.clone_inner().get_node_by_id("worker-1").await?;
    assert_eq!(updated_node.status, Some(NodeStatus::UP));

    Ok(())
}

#[tokio::test]
async fn handle_log_part_stores_log_parts() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "log-job";
    let task_id = "log-task";

    let job = Job {
        id: Some(job_id.into()),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(job_id.into()),
        state: TaskState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let log_part = TaskLogPart {
        id: Some("log-part-1".to_string()),
        task_id: Some(task_id.into()),
        number: 1,
        contents: Some("First log line".to_string()),
        ..Default::default()
    };

    let result = handlers::handle_log_part(ds, b, log_part.clone()).await;
    assert!(result.is_ok());

    let parts = datastore
        .clone_inner()
        .get_task_log_parts(task_id, "", 1, 10)
        .await?;
    assert_eq!(parts.items.len(), 1);
    assert_eq!(parts.items[0].contents, Some("First log line".to_string()));

    Ok(())
}

#[tokio::test]
async fn handle_log_part_multiple_parts_for_same_task() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "multi-log-job";
    let task_id = "multi-log-task";

    let job = Job {
        id: Some(job_id.into()),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(job_id.into()),
        state: TaskState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    for i in 1..=3 {
        let log_part = TaskLogPart {
            id: Some(format!("log-part-{}", i)),
            task_id: Some(task_id.into()),
            number: i,
            contents: Some(format!("Log line {}", i)),
            ..Default::default()
        };
        let result = handlers::handle_log_part(ds.clone(), b.clone(), log_part.clone()).await;
        assert!(result.is_ok());
    }

    let parts = datastore
        .clone_inner()
        .get_task_log_parts(task_id, "", 1, 10)
        .await?;
    assert_eq!(parts.items.len(), 3);

    Ok(())
}
