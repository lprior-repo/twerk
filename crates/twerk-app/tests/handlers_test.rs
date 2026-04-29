#![allow(clippy::unwrap_used)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::redundant_pattern_matching)]

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use twerk_app::engine::coordinator::handlers;
use twerk_app::engine::{BrokerProxy, DatastoreProxy, TOPIC_JOB_FAILED};
use twerk_core::id::{JobId, NodeId};
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

fn to_job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("test job id should be valid")
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

    let job_id = "550e8400-e29b-41d4-a716-446655440201";
    let task_id = "redeliver-test-task";

    let job = Job {
        id: Some(to_job_id(job_id)),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
        state: TaskState::Running,
        redelivered: 1,
        queue: Some("default".to_string()),
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_redelivered(ds, b, task.clone()).await;
    assert!(matches!(result, Ok(_)));

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

    let job_id = "550e8400-e29b-41d4-a716-446655440202";
    let task_id = "started-task";

    let job = Job {
        id: Some(to_job_id(job_id)),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
        state: twerk_core::task::TaskState::Scheduled,
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_started(ds, b, task.clone()).await;
    assert!(matches!(result, Ok(_)));

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

    let job_id = "550e8400-e29b-41d4-a716-446655440203";
    let task_id = "error-task";

    let job = Job {
        id: Some(to_job_id(job_id)),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
        state: TaskState::Running,
        error: Some("something went wrong".to_string()),
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_error(ds, b, task.clone()).await;
    assert!(matches!(result, Ok(_)));

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

    let job_id = "550e8400-e29b-41d4-a716-446655440204";
    let task_id = "error-queue-task";

    let job = Job {
        id: Some(to_job_id(job_id)),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
        state: TaskState::Running,
        error: Some("task failed".to_string()),
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let result = handlers::handle_error(ds, b, task.clone()).await;
    assert!(matches!(result, Ok(_)));

    let queues = broker.clone_inner().queues().await?;
    let failed_queue = queues.iter().find(|q| q.name == QUEUE_FAILED);
    assert!(failed_queue.is_some());
    assert_eq!(failed_queue.unwrap().size, 1);

    Ok(())
}

#[tokio::test]
async fn handle_error_publishes_failed_task_without_routing_job_twice() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "550e8400-e29b-41d4-a716-446655440208";
    let task_id = "error-single-route-task";

    datastore
        .clone_inner()
        .create_job(&Job {
            id: Some(to_job_id(job_id)),
            state: JobState::Running,
            ..Default::default()
        })
        .await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
        state: TaskState::Running,
        error: Some("task failed once".to_string()),
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let mut failed_events = broker
        .clone_inner()
        .subscribe(TOPIC_JOB_FAILED.to_string())
        .await?;

    handlers::handle_error(ds.clone(), b.clone(), task.clone()).await?;

    let job_before_failed_queue = datastore.clone_inner().get_job_by_id(job_id).await?;
    assert_eq!(job_before_failed_queue.state, JobState::Running);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), failed_events.recv())
            .await
            .is_err()
    );

    let queued_failed_task = Task {
        state: TaskState::Failed,
        failed_at: datastore
            .clone_inner()
            .get_task_by_id(task_id)
            .await?
            .failed_at,
        ..task
    };
    handlers::handle_task_failed(ds, b, queued_failed_task).await?;

    let failed_job = datastore.clone_inner().get_job_by_id(job_id).await?;
    assert_eq!(failed_job.state, JobState::Failed);
    tokio::time::timeout(Duration::from_secs(1), failed_events.recv()).await??;
    assert!(
        tokio::time::timeout(Duration::from_millis(50), failed_events.recv())
            .await
            .is_err()
    );

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
    assert!(matches!(result, Ok(_)));

    let updated_node = datastore.clone_inner().get_node_by_id("worker-1").await?;
    assert_eq!(updated_node.status, Some(NodeStatus::UP));

    Ok(())
}

#[tokio::test]
async fn handle_heartbeat_creates_visible_node_when_worker_omits_queue_and_version() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let node = Node {
        id: Some(NodeId::new("worker-missing-fields").unwrap()),
        name: Some("benchmark-worker".to_string()),
        hostname: Some("benchmark-host".to_string()),
        ..Default::default()
    };

    handlers::handle_heartbeat(ds, b, node).await?;

    let stored = datastore
        .clone_inner()
        .get_node_by_id("worker-missing-fields")
        .await?;
    assert_eq!(stored.status, Some(NodeStatus::UP));
    assert_eq!(stored.queue.as_deref(), Some("default"));
    assert!(stored.version.is_some());
    assert!(stored.last_heartbeat_at.is_some());

    Ok(())
}

#[tokio::test]
async fn handle_heartbeat_preserves_explicit_down_status() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    // Given a worker heartbeat explicitly reports DOWN
    let node = Node {
        id: Some(NodeId::new("worker-down").unwrap()),
        name: Some("down-worker".to_string()),
        hostname: Some("benchmark-host".to_string()),
        status: Some(NodeStatus::DOWN),
        ..Default::default()
    };

    // When the coordinator ingests the heartbeat
    handlers::handle_heartbeat(ds, b, node).await?;

    // Then explicit DOWN is preserved rather than forced UP
    let stored = datastore
        .clone_inner()
        .get_node_by_id("worker-down")
        .await?;
    assert_eq!(stored.status, Some(NodeStatus::DOWN));

    Ok(())
}

#[tokio::test]
async fn handle_heartbeat_clamps_future_heartbeat_timestamp() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);
    let before_ingest = time::OffsetDateTime::now_utc();

    // Given a worker heartbeat contains a future timestamp
    let node = Node {
        id: Some(NodeId::new("worker-future").unwrap()),
        name: Some("future-worker".to_string()),
        hostname: Some("benchmark-host".to_string()),
        last_heartbeat_at: Some(before_ingest + time::Duration::hours(1)),
        ..Default::default()
    };

    // When the coordinator ingests the heartbeat
    handlers::handle_heartbeat(ds, b, node).await?;

    // Then the stored timestamp is clamped back to ingestion time
    let stored = datastore
        .clone_inner()
        .get_node_by_id("worker-future")
        .await?;
    assert!(stored
        .last_heartbeat_at
        .is_some_and(|heartbeat_at| heartbeat_at >= before_ingest
            && heartbeat_at <= time::OffsetDateTime::now_utc()));

    Ok(())
}

#[tokio::test]
async fn handle_log_part_stores_log_parts() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "550e8400-e29b-41d4-a716-446655440205";
    let task_id = "log-task";

    let job = Job {
        id: Some(to_job_id(job_id)),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
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
    assert!(matches!(result, Ok(_)));

    let parts = datastore
        .clone_inner()
        .get_task_log_parts(task_id, "", 1, 10)
        .await?;
    assert_eq!(parts.items.len(), 1);
    assert_eq!(parts.items[0].contents, Some("First log line".to_string()));

    Ok(())
}

#[tokio::test]
async fn handle_log_part_assigns_missing_part_id_before_storage() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "550e8400-e29b-41d4-a716-446655440207";
    let task_id = "missing-log-part-id-task";

    let job = Job {
        id: Some(to_job_id(job_id)),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
        state: TaskState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_task(&task).await?;

    let log_part = TaskLogPart {
        id: None,
        task_id: Some(task_id.into()),
        number: 1,
        contents: Some("log line without publisher id".to_string()),
        ..Default::default()
    };

    let result = handlers::handle_log_part(ds.clone(), b.clone(), log_part.clone()).await;
    assert!(matches!(result, Ok(_)));
    let redelivery_result = handlers::handle_log_part(ds, b, log_part).await;
    assert!(matches!(redelivery_result, Ok(_)));

    let parts = datastore
        .clone_inner()
        .get_task_log_parts(task_id, "", 1, 10)
        .await?;
    assert_eq!(parts.items.len(), 1);
    assert!(parts.items[0].id.is_some());
    assert_eq!(
        parts.items[0].contents,
        Some("log line without publisher id".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn handle_log_part_multiple_parts_for_same_task() -> Result<()> {
    let (datastore, broker) = setup().await?;
    let ds = to_ds(&datastore);
    let b = to_broker(&broker);

    let job_id = "550e8400-e29b-41d4-a716-446655440206";
    let task_id = "multi-log-task";

    let job = Job {
        id: Some(to_job_id(job_id)),
        state: JobState::Running,
        ..Default::default()
    };
    datastore.clone_inner().create_job(&job).await?;

    let task = Task {
        id: Some(task_id.into()),
        job_id: Some(to_job_id(job_id)),
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
        assert!(matches!(result, Ok(_)));
    }

    let parts = datastore
        .clone_inner()
        .get_task_log_parts(task_id, "", 1, 10)
        .await?;
    assert_eq!(parts.items.len(), 3);

    Ok(())
}
