//! Tests for the in-memory broker.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::panic)]
#![allow(clippy::float_cmp)]

use super::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use twerk_core::id::{JobId, NodeId, TaskId};
use twerk_core::node::NodeStatus;
use twerk_core::task::{Task, TaskLogPart};
use twerk_core::uuid::new_uuid;

fn make_heartbeat_handler(
    received: Arc<RwLock<Vec<twerk_core::node::Node>>>,
) -> super::super::HeartbeatHandler {
    Arc::new(move |node: twerk_core::node::Node| {
        let received = received.clone();
        Box::pin(async move {
            received.write().await.push(node);
            Ok(())
        })
    })
}

fn make_task_log_part_handler(
    received: Arc<RwLock<Vec<TaskLogPart>>>,
) -> super::super::TaskLogPartHandler {
    Arc::new(move |part: TaskLogPart| {
        let received = received.clone();
        Box::pin(async move {
            received.write().await.push(part);
            Ok(())
        })
    })
}

#[tokio::test]
async fn test_publish_heartbeat_stores_and_notifies() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let handler = make_heartbeat_handler(received.clone());

    broker.subscribe_for_heartbeats(handler).await.unwrap();

    let node = twerk_core::node::Node {
        id: Some(NodeId::new("00000000-0000-0000-0000-000000000001").unwrap()),
        name: Some("worker-1".to_string()),
        status: Some(NodeStatus::UP),
        ..Default::default()
    };

    broker.publish_heartbeat(node.clone()).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(
        guard[0].id,
        Some(NodeId::new("00000000-0000-0000-0000-000000000001").unwrap())
    );
    assert_eq!(guard[0].name, Some("worker-1".to_string()));
}

#[tokio::test]
async fn test_subscribe_for_heartbeats_sends_existing() {
    let broker = InMemoryBroker::new();

    let node1 = twerk_core::node::Node {
        id: Some(NodeId::new("00000000-0000-0000-0000-000000000001").unwrap()),
        name: Some("worker-1".to_string()),
        status: Some(NodeStatus::UP),
        ..Default::default()
    };
    let node2 = twerk_core::node::Node {
        id: Some(NodeId::new("00000000-0000-0000-0000-000000000002").unwrap()),
        name: Some("worker-2".to_string()),
        status: Some(NodeStatus::UP),
        ..Default::default()
    };

    broker.publish_heartbeat(node1.clone()).await.unwrap();
    broker.publish_heartbeat(node2.clone()).await.unwrap();

    let received = Arc::new(RwLock::new(Vec::new()));
    let handler = make_heartbeat_handler(received.clone());

    broker.subscribe_for_heartbeats(handler).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 2);
    let ids: Vec<_> = guard.iter().map(|n| n.id.clone()).collect();
    assert!(ids.contains(&Some(
        NodeId::new("00000000-0000-0000-0000-000000000001").unwrap()
    )));
    assert!(ids.contains(&Some(
        NodeId::new("00000000-0000-0000-0000-000000000002").unwrap()
    )));
}

#[tokio::test]
async fn test_publish_task_log_part_stores_and_notifies() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let handler = make_task_log_part_handler(received.clone());

    broker.subscribe_for_task_log_part(handler).await.unwrap();

    let part = TaskLogPart {
        id: Some("log-part-1".to_string()),
        task_id: Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        number: 1,
        contents: Some("Log line 1".to_string()),
        ..Default::default()
    };

    broker.publish_task_log_part(&part).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].id, Some("log-part-1".to_string()));
    assert_eq!(
        guard[0].task_id,
        Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap())
    );
    assert_eq!(guard[0].number, 1);
    assert_eq!(guard[0].contents, Some("Log line 1".to_string()));
}

#[tokio::test]
async fn test_subscribe_for_task_log_part_sends_existing() {
    let broker = InMemoryBroker::new();

    let part1 = TaskLogPart {
        id: Some("log-part-1".to_string()),
        task_id: Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        number: 1,
        contents: Some("Log line 1".to_string()),
        ..Default::default()
    };
    let part2 = TaskLogPart {
        id: Some("log-part-2".to_string()),
        task_id: Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        number: 2,
        contents: Some("Log line 2".to_string()),
        ..Default::default()
    };

    broker.publish_task_log_part(&part1).await.unwrap();
    broker.publish_task_log_part(&part2).await.unwrap();

    let received = Arc::new(RwLock::new(Vec::new()));
    let handler = make_task_log_part_handler(received.clone());

    broker.subscribe_for_task_log_part(handler).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 2);
    assert_eq!(guard[0].id, Some("log-part-1".to_string()));
    assert_eq!(guard[1].id, Some("log-part-2".to_string()));
}

#[tokio::test]
async fn test_heartbeat_without_id_does_not_store() {
    let broker = InMemoryBroker::new();

    let node_no_id = twerk_core::node::Node {
        id: None,
        name: Some("anonymous".to_string()),
        status: Some(NodeStatus::UP),
        ..Default::default()
    };

    broker.publish_heartbeat(node_no_id).await.unwrap();

    let received = Arc::new(RwLock::new(Vec::new()));
    let handler = make_heartbeat_handler(received.clone());

    broker.subscribe_for_heartbeats(handler).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert!(guard.is_empty());
}

#[tokio::test]
async fn test_task_log_part_without_task_id_does_not_store() {
    let broker = InMemoryBroker::new();

    let part_no_task_id = TaskLogPart {
        id: Some("log-part-1".to_string()),
        task_id: None,
        number: 1,
        contents: Some("Log line 1".to_string()),
        ..Default::default()
    };

    broker
        .publish_task_log_part(&part_no_task_id)
        .await
        .unwrap();

    let received = Arc::new(RwLock::new(Vec::new()));
    let handler = make_task_log_part_handler(received.clone());

    broker.subscribe_for_task_log_part(handler).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert!(guard.is_empty());
}

// === Tests ported from Go broker/inmemory_test.go ===

#[tokio::test]
async fn test_publish_and_subscribe_for_task() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let qname = "test-queue".to_string();

    let received_clone = received.clone();
    let handler: super::super::TaskHandler = Arc::new(move |task: Arc<Task>| {
        let received = received_clone.clone();
        Box::pin(async move {
            received.write().await.push(task);
            Ok(())
        })
    });

    broker
        .subscribe_for_tasks(qname.clone(), handler)
        .await
        .unwrap();

    let task = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        name: Some("test-task".to_string()),
        ..Default::default()
    };

    broker.publish_task(qname, &task).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(
        guard[0].id,
        Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap())
    );
}

#[tokio::test]
async fn test_get_queues() {
    let broker = InMemoryBroker::new();
    let qname = format!("test-queue-{}", new_uuid());

    // Publish a task to create the queue
    broker
        .publish_task(qname.clone(), &Task::default())
        .await
        .unwrap();

    let queues = broker.queues().await.unwrap();
    assert_eq!(queues.len(), 1);
    assert_eq!(queues[0].name, qname);
    assert_eq!(queues[0].subscribers, 0);

    // Add multiple subscribers
    for _ in 0..10 {
        let handler: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();
    }

    let queues = broker.queues().await.unwrap();
    assert_eq!(queues.len(), 1);
    assert_eq!(queues[0].subscribers, 10);
}

#[tokio::test]
async fn test_get_queues_unacked() {
    let broker = InMemoryBroker::new();
    let qname = format!("test-queue-{}", new_uuid());

    broker
        .publish_task(qname.clone(), &Task::default())
        .await
        .unwrap();

    let queues = broker.queues().await.unwrap();
    assert_eq!(queues.len(), 1);
    assert_eq!(queues[0].name, qname);
    assert_eq!(queues[0].unacked, 0);
}

#[tokio::test]
async fn test_delete_queue() {
    let broker = InMemoryBroker::new();
    let qname = format!("test-queue-{}", new_uuid());

    // Publish a task to create the queue
    broker
        .publish_task(qname.clone(), &Task::default())
        .await
        .unwrap();

    // Add a subscriber
    let handler: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler)
        .await
        .unwrap();

    let queues = broker.queues().await.unwrap();
    assert_eq!(queues.len(), 1);
    assert_eq!(queues[0].subscribers, 1);

    // Delete the queue
    broker.delete_queue(qname.clone()).await.unwrap();

    let queues = broker.queues().await.unwrap();
    assert!(queues.is_empty());
}

#[tokio::test]
async fn test_publish_and_subscribe_for_job() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));

    let received_clone = received.clone();
    let handler: super::super::JobHandler = Arc::new(move |job: twerk_core::job::Job| {
        let received = received_clone.clone();
        Box::pin(async move {
            received.write().await.push(job);
            Ok(())
        })
    });

    broker.subscribe_for_jobs(handler).await.unwrap();

    let job = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000004").unwrap()),
        name: Some("test-job".to_string()),
        ..Default::default()
    };

    broker.publish_job(&job).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(
        guard[0].id.as_deref(),
        Some("00000000-0000-0000-0000-000000000004")
    );
}

#[tokio::test]
async fn test_multiple_subscribers_for_job() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let count = Arc::new(RwLock::new(0));

    let make_handler = |received: Arc<RwLock<Vec<twerk_core::job::Job>>>,
                        count: Arc<RwLock<i32>>|
     -> super::super::JobHandler {
        Arc::new(move |job: twerk_core::job::Job| {
            let received = received.clone();
            let count = count.clone();
            Box::pin(async move {
                received.write().await.push(job.clone());
                *count.write().await += 1;
                Ok(())
            })
        })
    };

    // Subscribe two handlers
    broker
        .subscribe_for_jobs(make_handler(received.clone(), count.clone()))
        .await
        .unwrap();
    broker
        .subscribe_for_jobs(make_handler(received.clone(), count.clone()))
        .await
        .unwrap();

    // Publish multiple jobs
    for i in 0..10 {
        let job = twerk_core::job::Job {
            id: Some(
                JobId::new(format!("00000000-0000-0000-0000-0000000000{:02}", 10 + i)).unwrap(),
            ),
            ..Default::default()
        };
        broker.publish_job(&job).await.unwrap();
    }

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 20); // 10 jobs * 2 handlers
    let cnt = *count.read().await;
    assert_eq!(cnt, 20);
}

#[tokio::test]
async fn test_subscribe_for_events() {
    let broker = InMemoryBroker::new();
    let received1 = Arc::new(RwLock::new(Vec::new()));
    let received2 = Arc::new(RwLock::new(Vec::new()));

    let received1_clone = received1.clone();
    let handler1: super::super::EventHandler = Arc::new(move |event: serde_json::Value| {
        let received = received1_clone.clone();
        Box::pin(async move {
            received.write().await.push(event);
            Ok(())
        })
    });

    let received2_clone = received2.clone();
    let handler2: super::super::EventHandler = Arc::new(move |event: serde_json::Value| {
        let received = received2_clone.clone();
        Box::pin(async move {
            received.write().await.push(event);
            Ok(())
        })
    });

    // Subscribe to JOB.* pattern
    broker
        .subscribe_for_events("job.*".to_string(), handler1)
        .await
        .unwrap();
    // Subscribe to JOB_COMPLETED pattern
    broker
        .subscribe_for_events("job.completed".to_string(), handler2)
        .await
        .unwrap();

    let job = serde_json::json!({
        "id": "job-1",
        "state": "COMPLETED"
    });

    // Publish to JOB_COMPLETED topic
    broker
        .publish_event("job.completed".to_string(), job.clone())
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Both handlers should receive it (pattern match)
    let guard1 = received1.read().await;
    let guard2 = received2.read().await;
    assert_eq!(guard1.len(), 1);
    assert_eq!(guard2.len(), 1);
}

#[tokio::test]
async fn test_health_check() {
    let broker = InMemoryBroker::new();

    broker.health_check().await.unwrap();

    broker.shutdown().await.unwrap();

    broker.health_check().await.unwrap();
}

#[tokio::test]
async fn test_publish_and_subscribe_for_task_progress() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));

    let received_clone = received.clone();
    let handler: super::super::TaskProgressHandler = Arc::new(move |task: Task| {
        let received = received_clone.clone();
        Box::pin(async move {
            received.write().await.push(task);
            Ok(())
        })
    });

    broker.subscribe_for_task_progress(handler).await.unwrap();

    let task = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        progress: 50.0,
        ..Default::default()
    };

    broker.publish_task_progress(&task).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].progress, 50.0);
}

#[tokio::test]
async fn test_queue_info() {
    let broker = InMemoryBroker::new();
    let qname = "test-queue".to_string();

    // Publish some tasks
    for i in 0..5 {
        let task = Task {
            id: Some(
                TaskId::new(format!("00000000-0000-0000-0000-0000000000{:02}", 20 + i)).unwrap(),
            ),
            ..Default::default()
        };
        broker.publish_task(qname.clone(), &task).await.unwrap();
    }

    let info = broker.queue_info(qname).await.unwrap();
    assert_eq!(info.size, 5);
    assert_eq!(info.name, "test-queue");
}

#[tokio::test]
async fn broker_publish_heartbeat_receives_handler() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));

    let received_clone = received.clone();
    let handler: super::super::HeartbeatHandler = Arc::new(move |node: twerk_core::node::Node| {
        let received = received_clone.clone();
        Box::pin(async move {
            received.write().await.push(node);
            Ok(())
        })
    });

    broker.subscribe_for_heartbeats(handler).await.unwrap();

    let node = twerk_core::node::Node {
        id: Some(NodeId::new("00000000-0000-0000-0000-000000000001").unwrap()),
        name: Some("worker-1".to_string()),
        status: Some(NodeStatus::UP),
        ..Default::default()
    };

    broker.publish_heartbeat(node).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let guard = received.read().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(
        guard[0].id,
        Some(NodeId::new("00000000-0000-0000-0000-000000000001").unwrap())
    );
    assert_eq!(guard[0].name, Some("worker-1".to_string()));
}

#[tokio::test]
async fn broker_shutdown_fails_health_check_after() {
    let broker = InMemoryBroker::new();
    let qname = format!("exclusive-queue-{}", new_uuid());

    let handler: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler)
        .await
        .unwrap();

    let task = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        ..Default::default()
    };
    broker.publish_task(qname, &task).await.unwrap();

    broker.health_check().await.unwrap();

    broker.shutdown().await.unwrap();

    broker.health_check().await.unwrap();
}
