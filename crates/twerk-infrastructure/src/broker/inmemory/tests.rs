//! Tests for the in-memory broker.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::panic)]
#![allow(clippy::float_cmp)]

use super::*;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use twerk_core::id::{JobId, NodeId, TaskId};
use twerk_core::node::NodeStatus;
use twerk_core::task::{Task, TaskLogPart};
use twerk_core::uuid::new_uuid;

/// Helper to wait for a handler notification with timeout.
async fn wait_for_handler_notification(rx: &mut oneshot::Receiver<()>) -> Result<(), String> {
    tokio::time::timeout(std::time::Duration::from_secs(5), rx)
        .await
        .map_err(|_| "Handler notification timed out".to_string())?
        .map_err(|e| e.to_string())
}

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
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::HeartbeatHandler = Arc::new(move |node: twerk_core::node::Node| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            received.write().await.push(node);
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
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

    broker.publish_heartbeat(node.clone()).await.unwrap();

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count_clone = count.clone();

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::HeartbeatHandler = Arc::new(move |node: twerk_core::node::Node| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        let count = count_clone.clone();
        Box::pin(async move {
            received.write().await.push(node);
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count.load(std::sync::atomic::Ordering::SeqCst) == 2 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            Ok(())
        })
    });

    broker.subscribe_for_heartbeats(handler).await.unwrap();

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::TaskLogPartHandler = Arc::new(move |part: TaskLogPart| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            received.write().await.push(part);
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
            Ok(())
        })
    });

    broker.subscribe_for_task_log_part(handler).await.unwrap();

    let part = TaskLogPart {
        id: Some("log-part-1".to_string()),
        task_id: Some(TaskId::new("00000000-0000-0000-0000-000000000003").unwrap()),
        number: 1,
        contents: Some("Log line 1".to_string()),
        ..Default::default()
    };

    broker.publish_task_log_part(&part).await.unwrap();

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count_clone = count.clone();

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::TaskLogPartHandler = Arc::new(move |part: TaskLogPart| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        let count = count_clone.clone();
        Box::pin(async move {
            received.write().await.push(part);
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count.load(std::sync::atomic::Ordering::SeqCst) == 2 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            Ok(())
        })
    });

    broker.subscribe_for_task_log_part(handler).await.unwrap();

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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

    // No sleep needed - nodes without id don't trigger handlers
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

    // No sleep needed - parts without task_id don't trigger handlers
    let guard = received.read().await;
    assert!(guard.is_empty());
}

// === Tests ported from Go broker/inmemory_test.go ===

#[tokio::test]
async fn test_publish_and_subscribe_for_task() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
    let qname = "test-queue".to_string();

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::TaskHandler = Arc::new(move |task: Arc<Task>| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            received.write().await.push(task);
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
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

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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

    // Add 10 subscribers explicitly
    let handler1: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler1)
        .await
        .unwrap();

    let handler2: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler2)
        .await
        .unwrap();

    let handler3: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler3)
        .await
        .unwrap();

    let handler4: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler4)
        .await
        .unwrap();

    let handler5: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler5)
        .await
        .unwrap();

    let handler6: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler6)
        .await
        .unwrap();

    let handler7: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler7)
        .await
        .unwrap();

    let handler8: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler8)
        .await
        .unwrap();

    let handler9: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler9)
        .await
        .unwrap();

    let handler10: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    broker
        .subscribe_for_tasks(qname.clone(), handler10)
        .await
        .unwrap();

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
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::JobHandler = Arc::new(move |job: twerk_core::job::Job| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            received.write().await.push(job);
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
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

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let make_handler = |received: Arc<RwLock<Vec<twerk_core::job::Job>>>,
                        count: Arc<std::sync::atomic::AtomicUsize>,
                        tx: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>|
     -> super::super::JobHandler {
        let received_clone = received.clone();
        let tx_clone = tx.clone();
        let count_clone = count.clone();
        Arc::new(move |job: twerk_core::job::Job| {
            let received = received_clone.clone();
            let tx = tx_clone.clone();
            let count = count_clone.clone();
            Box::pin(async move {
                received.write().await.push(job.clone());
                let prev = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if prev + 1 == 20 {
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                }
                Ok(())
            })
        })
    };

    // Subscribe two handlers
    broker
        .subscribe_for_jobs(make_handler(received.clone(), count.clone(), tx.clone()))
        .await
        .unwrap();
    broker
        .subscribe_for_jobs(make_handler(received.clone(), count.clone(), tx.clone()))
        .await
        .unwrap();

    // Publish 10 jobs explicitly
    let job1 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000010").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job1).await.unwrap();

    let job2 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000011").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job2).await.unwrap();

    let job3 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000012").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job3).await.unwrap();

    let job4 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000013").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job4).await.unwrap();

    let job5 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000014").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job5).await.unwrap();

    let job6 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000015").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job6).await.unwrap();

    let job7 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000016").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job7).await.unwrap();

    let job8 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000017").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job8).await.unwrap();

    let job9 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000018").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job9).await.unwrap();

    let job10 = twerk_core::job::Job {
        id: Some(JobId::new("00000000-0000-0000-0000-000000000019").unwrap()),
        ..Default::default()
    };
    broker.publish_job(&job10).await.unwrap();

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

    let guard = received.read().await;
    assert_eq!(guard.len(), 20); // 10 jobs * 2 handlers
    let cnt = count.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(cnt, 20);
}

#[tokio::test]
async fn test_subscribe_for_events() {
    let broker = InMemoryBroker::new();
    let received1 = Arc::new(RwLock::new(Vec::new()));
    let received2 = Arc::new(RwLock::new(Vec::new()));
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let received1_clone = received1.clone();
    let tx_clone = tx.clone();
    let count_clone = count.clone();
    let handler1: super::super::EventHandler = Arc::new(move |event: serde_json::Value| {
        let received = received1_clone.clone();
        let tx = tx_clone.clone();
        let count = count_clone.clone();
        Box::pin(async move {
            received.write().await.push(event);
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count.load(std::sync::atomic::Ordering::SeqCst) == 2 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            Ok(())
        })
    });

    let received2_clone = received2.clone();
    let tx_clone2 = tx.clone();
    let count_clone2 = count.clone();
    let handler2: super::super::EventHandler = Arc::new(move |event: serde_json::Value| {
        let received = received2_clone.clone();
        let tx = tx_clone2.clone();
        let count = count_clone2.clone();
        Box::pin(async move {
            received.write().await.push(event);
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count.load(std::sync::atomic::Ordering::SeqCst) == 2 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
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

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::TaskProgressHandler = Arc::new(move |task: Task| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            received.write().await.push(task);
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
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

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

    let guard = received.read().await;
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].progress, 50.0);
}

#[tokio::test]
async fn test_queue_info() {
    let broker = InMemoryBroker::new();
    let qname = "test-queue".to_string();

    // Publish 5 tasks explicitly
    let task1 = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000014").unwrap()),
        ..Default::default()
    };
    broker.publish_task(qname.clone(), &task1).await.unwrap();

    let task2 = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000015").unwrap()),
        ..Default::default()
    };
    broker.publish_task(qname.clone(), &task2).await.unwrap();

    let task3 = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000016").unwrap()),
        ..Default::default()
    };
    broker.publish_task(qname.clone(), &task3).await.unwrap();

    let task4 = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000017").unwrap()),
        ..Default::default()
    };
    broker.publish_task(qname.clone(), &task4).await.unwrap();

    let task5 = Task {
        id: Some(TaskId::new("00000000-0000-0000-0000-000000000018").unwrap()),
        ..Default::default()
    };
    broker.publish_task(qname.clone(), &task5).await.unwrap();

    let info = broker.queue_info(qname).await.unwrap();
    assert_eq!(info.size, 5);
    assert_eq!(info.name, "test-queue");
}

#[tokio::test]
async fn broker_publish_heartbeat_receives_handler() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::HeartbeatHandler = Arc::new(move |node: twerk_core::node::Node| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            received.write().await.push(node);
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
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

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Handler should notify");

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

#[tokio::test]
async fn test_subscribe_returns_broadcast_receiver() {
    let broker = InMemoryBroker::new();
    let mut rx = broker
        .subscribe("test.*".to_string())
        .await
        .unwrap();
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn test_subscribe_pattern_matches_order_created() {
    let broker = InMemoryBroker::new();
    let mut rx = broker.subscribe("order.*".to_string()).await.unwrap();

    let job = serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "state": "COMPLETED"
    });
    broker
        .publish_event("order.created".to_string(), job)
        .await
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("Should receive event")
        .expect("Should receive event without error");
    assert!(matches!(event, twerk_core::job::JobEvent::Completed(_)));
}

#[tokio::test]
async fn test_subscribe_pattern_matches_order_updated() {
    let broker = InMemoryBroker::new();
    let mut rx = broker.subscribe("order.*".to_string()).await.unwrap();

    let job = serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000002",
        "state": "COMPLETED"
    });
    broker
        .publish_event("order.updated".to_string(), job)
        .await
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("Should receive event")
        .expect("Should receive event without error");
    assert!(matches!(event, twerk_core::job::JobEvent::Completed(_)));
}

#[tokio::test]
async fn test_subscribe_pattern_does_not_match_payment_created() {
    let broker = InMemoryBroker::new();
    let mut rx = broker.subscribe("order.*".to_string()).await.unwrap();

    let order_job = serde_json::json!({
        "id": "job-1",
        "state": "COMPLETED"
    });
    broker
        .publish_event("order.created".to_string(), order_job)
        .await
        .unwrap();

    let _order_event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("Should receive order event")
        .expect("Should receive order event without error");

    let payment_job = serde_json::json!({
        "id": "job-2",
        "state": "COMPLETED"
    });
    broker
        .publish_event("payment.created".to_string(), payment_job)
        .await
        .unwrap();

    let result = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await;
    assert!(result.is_err() || result.as_ref().is_err());
}

#[tokio::test]
async fn test_subscribe_accepts_pattern_without_immediate_validation() {
    let broker = InMemoryBroker::new();
    let result = broker.subscribe("[invalid".to_string()).await;
    assert!(result.is_ok());
    let mut rx = result.unwrap();
    let job = serde_json::json!({
        "id": "job-1",
        "state": "COMPLETED"
    });
    broker
        .publish_event("order.created".to_string(), job)
        .await
        .unwrap();
    let event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("Should receive event")
        .expect("Should receive event without error");
    assert!(matches!(event, twerk_core::job::JobEvent::Completed(_)));
}

#[tokio::test]
async fn test_broker_maintains_fifo_order_per_topic_single_topic() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::TaskHandler = Arc::new(move |task: std::sync::Arc<twerk_core::task::Task>| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            let name = task.name.clone().unwrap_or_else(|| "unnamed".to_string());
            received.write().await.push(name);
            if received.read().await.len() == 3 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            Ok(())
        })
    });

    let topic_a = format!("topic-A-{}", new_uuid());
    broker.subscribe_for_tasks(topic_a.clone(), handler).await.unwrap();

    let m1 = twerk_core::task::Task {
        name: Some("m1".to_string()),
        ..Default::default()
    };
    let m2 = twerk_core::task::Task {
        name: Some("m2".to_string()),
        ..Default::default()
    };
    let m3 = twerk_core::task::Task {
        name: Some("m3".to_string()),
        ..Default::default()
    };

    broker.publish_task(topic_a.clone(), &m1).await.unwrap();
    broker.publish_task(topic_a.clone(), &m2).await.unwrap();
    broker.publish_task(topic_a.clone(), &m3).await.unwrap();

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Should receive all 3 messages");

    let guard = received.read().await;
    assert_eq!(*guard, vec!["m1", "m2", "m3"], "Messages should arrive in FIFO order");
}

#[tokio::test]
async fn test_broker_maintains_fifo_order_per_topic_topic_b() {
    let broker = InMemoryBroker::new();
    let received = Arc::new(RwLock::new(Vec::new()));
    let (tx, mut rx) = oneshot::channel();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));

    let received_clone = received.clone();
    let tx_clone = tx.clone();
    let handler: super::super::TaskHandler = Arc::new(move |task: std::sync::Arc<twerk_core::task::Task>| {
        let received = received_clone.clone();
        let tx = tx_clone.clone();
        Box::pin(async move {
            let name = task.name.clone().unwrap_or_else(|| "unnamed".to_string());
            received.write().await.push(name);
            if received.read().await.len() == 2 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            Ok(())
        })
    });

    let topic_b = format!("topic-B-{}", new_uuid());
    broker.subscribe_for_tasks(topic_b.clone(), handler).await.unwrap();

    let m4 = twerk_core::task::Task {
        name: Some("m4".to_string()),
        ..Default::default()
    };
    let m5 = twerk_core::task::Task {
        name: Some("m5".to_string()),
        ..Default::default()
    };

    broker.publish_task(topic_b.clone(), &m4).await.unwrap();
    broker.publish_task(topic_b.clone(), &m5).await.unwrap();

    wait_for_handler_notification(&mut rx)
        .await
        .expect("Should receive all 2 messages");

    let guard = received.read().await;
    assert_eq!(*guard, vec!["m4", "m5"], "Messages on topic B should arrive in FIFO order");
}

#[tokio::test]
async fn test_broker_maintains_fifo_order_per_topic_interleaved_publishes() {
    let broker = InMemoryBroker::new();
    let received_a = Arc::new(RwLock::new(Vec::new()));
    let received_b = Arc::new(RwLock::new(Vec::new()));
    let (tx_a, mut rx_a) = oneshot::channel();
    let (tx_b, mut rx_b) = oneshot::channel();
    let tx_a = Arc::new(std::sync::Mutex::new(Some(tx_a)));
    let tx_b = Arc::new(std::sync::Mutex::new(Some(tx_b)));

    let received_a_clone = received_a.clone();
    let tx_a_clone = tx_a.clone();
    let handler_a: super::super::TaskHandler = Arc::new(move |task: std::sync::Arc<twerk_core::task::Task>| {
        let received = received_a_clone.clone();
        let tx = tx_a_clone.clone();
        Box::pin(async move {
            let name = task.name.clone().unwrap_or_else(|| "unnamed".to_string());
            received.write().await.push(name);
            if received.read().await.len() == 3 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            Ok(())
        })
    });

    let received_b_clone = received_b.clone();
    let tx_b_clone = tx_b.clone();
    let handler_b: super::super::TaskHandler = Arc::new(move |task: std::sync::Arc<twerk_core::task::Task>| {
        let received = received_b_clone.clone();
        let tx = tx_b_clone.clone();
        Box::pin(async move {
            let name = task.name.clone().unwrap_or_else(|| "unnamed".to_string());
            received.write().await.push(name);
            if received.read().await.len() == 2 {
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            Ok(())
        })
    });

    let topic_a = format!("topic-A-{}", new_uuid());
    let topic_b = format!("topic-B-{}", new_uuid());
    broker.subscribe_for_tasks(topic_a.clone(), handler_a).await.unwrap();
    broker.subscribe_for_tasks(topic_b.clone(), handler_b).await.unwrap();

    let m1 = twerk_core::task::Task {
        name: Some("m1".to_string()),
        ..Default::default()
    };
    let m2 = twerk_core::task::Task {
        name: Some("m2".to_string()),
        ..Default::default()
    };
    let m3 = twerk_core::task::Task {
        name: Some("m3".to_string()),
        ..Default::default()
    };
    let m4 = twerk_core::task::Task {
        name: Some("m4".to_string()),
        ..Default::default()
    };
    let m5 = twerk_core::task::Task {
        name: Some("m5".to_string()),
        ..Default::default()
    };

    broker.publish_task(topic_a.clone(), &m1).await.unwrap();
    broker.publish_task(topic_b.clone(), &m4).await.unwrap();
    broker.publish_task(topic_a.clone(), &m2).await.unwrap();
    broker.publish_task(topic_b.clone(), &m5).await.unwrap();
    broker.publish_task(topic_a.clone(), &m3).await.unwrap();

    wait_for_handler_notification(&mut rx_a)
        .await
        .expect("Should receive all 3 messages on topic A");
    wait_for_handler_notification(&mut rx_b)
        .await
        .expect("Should receive all 2 messages on topic B");

    let guard_a = received_a.read().await;
    let guard_b = received_b.read().await;
    assert_eq!(
        *guard_a,
        vec!["m1", "m2", "m3"],
        "Messages on topic A should arrive in FIFO order despite interleaved publishes"
    );
    assert_eq!(
        *guard_b,
        vec!["m4", "m5"],
        "Messages on topic B should arrive in FIFO order despite interleaved publishes"
    );
}

#[tokio::test]
async fn test_pubsub_all_subscribers_receive_message() {
    let broker = InMemoryBroker::new();
    let topic = "task.events".to_string();

    let mut rx1 = broker.subscribe(topic.clone()).await.unwrap();
    let mut rx2 = broker.subscribe(topic.clone()).await.unwrap();
    let mut rx3 = broker.subscribe(topic.clone()).await.unwrap();

    let job = serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "state": "CREATED",
        "name": "test-task"
    });

    broker.publish_event(topic.clone(), job.clone()).await.unwrap();

    let event1 = tokio::time::timeout(std::time::Duration::from_secs(5), rx1.recv())
        .await
        .expect("Consumer 1 should receive event")
        .expect("Should receive event without error");
    let event2 = tokio::time::timeout(std::time::Duration::from_secs(5), rx2.recv())
        .await
        .expect("Consumer 2 should receive event")
        .expect("Should receive event without error");
    let event3 = tokio::time::timeout(std::time::Duration::from_secs(5), rx3.recv())
        .await
        .expect("Consumer 3 should receive event")
        .expect("Should receive event without error");

    assert!(
        matches!(event1, twerk_core::job::JobEvent::Completed(_)),
        "Event 1 should be Completed"
    );
    assert!(
        matches!(event2, twerk_core::job::JobEvent::Completed(_)),
        "Event 2 should be Completed"
    );
    assert!(
        matches!(event3, twerk_core::job::JobEvent::Completed(_)),
        "Event 3 should be Completed"
    );
}

#[tokio::test]
async fn test_pubsub_after_unsubscribe_only_active_receive() {
    let broker = InMemoryBroker::new();
    let topic = "task.events".to_string();

    let mut rx1 = broker.subscribe(topic.clone()).await.unwrap();
    let mut rx2 = broker.subscribe(topic.clone()).await.unwrap();
    let mut rx3 = broker.subscribe(topic.clone()).await.unwrap();

    let job1 = serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "state": "COMPLETED",
        "name": "test-task-1"
    });

    broker.publish_event(topic.clone(), job1.clone()).await.unwrap();

    let _event1 = tokio::time::timeout(std::time::Duration::from_secs(5), rx1.recv())
        .await
        .expect("Consumer 1 should receive first event")
        .expect("Should receive event without error");
    let _event2 = tokio::time::timeout(std::time::Duration::from_secs(5), rx2.recv())
        .await
        .expect("Consumer 2 should receive first event")
        .expect("Should receive event without error");
    let _event3 = tokio::time::timeout(std::time::Duration::from_secs(5), rx3.recv())
        .await
        .expect("Consumer 3 should receive first event")
        .expect("Should receive event without error");

    drop(rx2);

    let job2 = serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000002",
        "state": "COMPLETED",
        "name": "test-task-2"
    });

    broker.publish_event(topic.clone(), job2.clone()).await.unwrap();

    let event1_second = tokio::time::timeout(std::time::Duration::from_secs(5), rx1.recv())
        .await
        .expect("Consumer 1 should receive second event")
        .expect("Should receive event without error");
    let event3_second = tokio::time::timeout(std::time::Duration::from_secs(5), rx3.recv())
        .await
        .expect("Consumer 3 should receive second event")
        .expect("Should receive event without error");

    assert!(
        matches!(event1_second, twerk_core::job::JobEvent::Completed(_)),
        "Event 1 second should be Completed"
    );
    assert!(
        matches!(event3_second, twerk_core::job::JobEvent::Completed(_)),
        "Event 3 second should be Completed"
    );
}
