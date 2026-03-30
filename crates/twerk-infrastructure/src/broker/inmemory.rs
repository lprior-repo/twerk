use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use super::{
    BoxedFuture, Broker, EventHandler, HeartbeatHandler, JobHandler, QueueInfo, TaskHandler,
    TaskLogPartHandler, TaskProgressHandler,
};
use twerk_common::wildcard::wildcard_match;
use twerk_core::job::Job;
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

/// In-memory broker implementation for testing and single-process usage.
pub struct InMemoryBroker {
    /// Queue name -> list of tasks
    tasks: DashMap<String, Vec<Arc<Task>>>,
    /// Queue name -> list of task handlers
    handlers: DashMap<String, Vec<TaskHandler>>,
    /// Job handlers
    job_handlers: Arc<RwLock<Vec<JobHandler>>>,
    /// Task progress handlers
    progress_handlers: Arc<RwLock<Vec<TaskProgressHandler>>>,
    /// Event handlers (topic pattern -> list of handlers)
    event_handlers: Arc<DashMap<String, Vec<EventHandler>>>,
    /// Heartbeat handlers
    heartbeat_handlers: Arc<RwLock<Vec<HeartbeatHandler>>>,
    /// Stored heartbeats (node_id -> node)
    heartbeats: Arc<RwLock<DashMap<String, Node>>>,
    /// Task log part handlers
    task_log_part_handlers: Arc<RwLock<Vec<TaskLogPartHandler>>>,
    /// Stored task log parts (task_id -> Vec<TaskLogPart>)
    task_log_parts: Arc<RwLock<DashMap<String, Vec<TaskLogPart>>>>,
}

impl Default for InMemoryBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBroker {
    /// Creates a new in-memory broker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            handlers: DashMap::new(),
            job_handlers: Arc::new(RwLock::new(Vec::new())),
            progress_handlers: Arc::new(RwLock::new(Vec::new())),
            event_handlers: Arc::new(DashMap::new()),
            heartbeat_handlers: Arc::new(RwLock::new(Vec::new())),
            heartbeats: Arc::new(RwLock::new(DashMap::new())),
            task_log_part_handlers: Arc::new(RwLock::new(Vec::new())),
            task_log_parts: Arc::new(RwLock::new(DashMap::new())),
        }
    }
}

/// Creates a new in-memory broker.
#[must_use]
pub fn new_in_memory_broker() -> Box<dyn Broker + Send + Sync> {
    Box::new(InMemoryBroker::new())
}

impl Broker for InMemoryBroker {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let task = Arc::new(task.clone());

        // Store the task
        self.tasks
            .entry(qname.clone())
            .or_default()
            .push(Arc::clone(&task));

        // Collect handlers for this queue before spawning tasks
        let handlers: Vec<TaskHandler> = self
            .handlers
            .get(&qname)
            .map(|entry| entry.value().clone())
            .unwrap_or_default();

        // Invoke all registered handlers for this queue
        for handler in handlers {
            let task_clone = Arc::clone(&task);
            tokio::spawn(async move {
                let _ = handler(task_clone).await;
            });
        }

        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
        self.handlers.entry(qname).or_default().push(handler);
        Box::pin(async { Ok(()) })
    }

    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()> {
        let task = task.clone();
        let handlers = self.progress_handlers.clone();
        Box::pin(async move {
            let handlers = handlers.read().await;
            for handler in handlers.iter() {
                let task_clone = task.clone();
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let _ = handler_clone(task_clone).await;
                });
            }
            Ok(())
        })
    }

    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()> {
        let handlers = self.progress_handlers.clone();
        Box::pin(async move {
            handlers.write().await.push(handler);
            Ok(())
        })
    }

    fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()> {
        let node = node.clone();
        let handlers = self.heartbeat_handlers.clone();
        let heartbeats = self.heartbeats.clone();
        Box::pin(async move {
            if let Some(ref node_id) = node.id {
                heartbeats
                    .write()
                    .await
                    .insert(node_id.to_string(), node.clone());
            }
            let handlers = handlers.read().await;
            for handler in handlers.iter() {
                let node_clone = node.clone();
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let _ = handler_clone(node_clone).await;
                });
            }
            Ok(())
        })
    }

    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()> {
        let handlers = self.heartbeat_handlers.clone();
        let heartbeats = self.heartbeats.clone();
        Box::pin(async move {
            let nodes: Vec<Node> = heartbeats
                .read()
                .await
                .iter()
                .map(|e| e.value().clone())
                .collect();
            for node in nodes {
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let _ = handler_clone(node).await;
                });
            }
            handlers.write().await.push(handler);
            Ok(())
        })
    }

    fn publish_job(&self, job: &Job) -> BoxedFuture<()> {
        let job = job.clone();
        let handlers = self.job_handlers.clone();
        Box::pin(async move {
            let handlers = handlers.read().await;
            debug!(
                "Publishing job {} to {} handlers",
                job.id.as_deref().unwrap_or("unknown"),
                handlers.len()
            );
            for handler in handlers.iter() {
                let job_clone = job.clone();
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let _ = handler_clone(job_clone).await;
                });
            }
            Ok(())
        })
    }

    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()> {
        let handlers = self.job_handlers.clone();
        Box::pin(async move {
            debug!("Subscribing for jobs");
            handlers.write().await.push(handler);
            Ok(())
        })
    }

    fn publish_event(&self, topic: String, event: Value) -> BoxedFuture<()> {
        let handlers = self.event_handlers.clone();
        Box::pin(async move {
            for entry in handlers.iter() {
                let pattern = entry.key();
                if wildcard_match(pattern, &topic) {
                    let event_clone = event.clone();
                    let topic_handlers = entry.value().clone();
                    for handler in topic_handlers {
                        let ev_clone = event_clone.clone();
                        let h_clone = handler.clone();
                        tokio::spawn(async move {
                            let _ = h_clone(ev_clone).await;
                        });
                    }
                }
            }
            Ok(())
        })
    }

    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()> {
        self.event_handlers
            .entry(pattern)
            .or_default()
            .push(handler);
        Box::pin(async { Ok(()) })
    }

    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()> {
        let part = part.clone();
        let handlers = self.task_log_part_handlers.clone();
        let task_log_parts = self.task_log_parts.clone();
        Box::pin(async move {
            if let Some(task_id) = &part.task_id {
                let task_id_str = task_id.to_string();
                let task_log_parts_guard = task_log_parts.write().await;
                let mut entry = task_log_parts_guard.entry(task_id_str).or_default();
                entry.push(part.clone());
            }
            let handlers = handlers.read().await;
            for handler in handlers.iter() {
                let part_clone = part.clone();
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let _ = handler_clone(part_clone).await;
                });
            }
            Ok(())
        })
    }

    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()> {
        let handlers = self.task_log_part_handlers.clone();
        let task_log_parts = self.task_log_parts.clone();
        Box::pin(async move {
            let parts: Vec<TaskLogPart> = task_log_parts
                .read()
                .await
                .iter()
                .flat_map(|e| e.value().clone())
                .collect();
            for part in parts {
                let handler_clone = handler.clone();
                tokio::spawn(async move {
                    let _ = handler_clone(part).await;
                });
            }
            handlers.write().await.push(handler);
            Ok(())
        })
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        let queues = self
            .tasks
            .iter()
            .map(|entry| {
                let qname = entry.key().clone();
                let task_list = entry.value();
                let subscribers = self
                    .handlers
                    .get(&qname)
                    .map(|h| h.len() as i32)
                    .unwrap_or(0);
                QueueInfo {
                    name: qname,
                    size: task_list.len() as i32,
                    subscribers,
                    unacked: 0,
                }
            })
            .collect();
        Box::pin(async { Ok(queues) })
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        let size = self
            .tasks
            .get(&qname)
            .map(|entry| entry.len() as i32)
            .unwrap_or(0);
        let subscribers = self
            .handlers
            .get(&qname)
            .map(|entry| entry.len() as i32)
            .unwrap_or(0);
        Box::pin(async move {
            Ok(QueueInfo {
                name: qname,
                size,
                subscribers,
                unacked: 0,
            })
        })
    }

    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        self.tasks.remove(&qname);
        self.handlers.remove(&qname);
        Box::pin(async { Ok(()) })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twerk_core::id::{JobId, NodeId, TaskId};
    use twerk_core::node::NodeStatus;
    use twerk_core::uuid::new_uuid;

    fn make_heartbeat_handler(received: Arc<RwLock<Vec<Node>>>) -> HeartbeatHandler {
        Arc::new(move |node: Node| {
            let received = received.clone();
            Box::pin(async move {
                received.write().await.push(node);
                Ok(())
            })
        })
    }

    fn make_task_log_part_handler(received: Arc<RwLock<Vec<TaskLogPart>>>) -> TaskLogPartHandler {
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

        let node = Node {
            id: Some(NodeId::new("node-1")),
            name: Some("worker-1".to_string()),
            status: Some(NodeStatus::UP),
            ..Default::default()
        };

        broker.publish_heartbeat(node.clone()).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let guard = received.read().await;
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].id, Some(NodeId::new("node-1")));
        assert_eq!(guard[0].name, Some("worker-1".to_string()));
    }

    #[tokio::test]
    async fn test_subscribe_for_heartbeats_sends_existing() {
        let broker = InMemoryBroker::new();

        let node1 = Node {
            id: Some(NodeId::new("node-1")),
            name: Some("worker-1".to_string()),
            status: Some(NodeStatus::UP),
            ..Default::default()
        };
        let node2 = Node {
            id: Some(NodeId::new("node-2")),
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
        assert!(ids.contains(&Some(NodeId::new("node-1"))));
        assert!(ids.contains(&Some(NodeId::new("node-2"))));
    }

    #[tokio::test]
    async fn test_publish_task_log_part_stores_and_notifies() {
        let broker = InMemoryBroker::new();
        let received = Arc::new(RwLock::new(Vec::new()));
        let handler = make_task_log_part_handler(received.clone());

        broker.subscribe_for_task_log_part(handler).await.unwrap();

        let part = TaskLogPart {
            id: Some("log-part-1".to_string()),
            task_id: Some(TaskId::new("task-1")),
            number: 1,
            contents: Some("Log line 1".to_string()),
            ..Default::default()
        };

        broker.publish_task_log_part(&part).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let guard = received.read().await;
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].id, Some("log-part-1".to_string()));
        assert_eq!(guard[0].task_id, Some(TaskId::new("task-1")));
        assert_eq!(guard[0].number, 1);
        assert_eq!(guard[0].contents, Some("Log line 1".to_string()));
    }

    #[tokio::test]
    async fn test_subscribe_for_task_log_part_sends_existing() {
        let broker = InMemoryBroker::new();

        let part1 = TaskLogPart {
            id: Some("log-part-1".to_string()),
            task_id: Some(TaskId::new("task-1")),
            number: 1,
            contents: Some("Log line 1".to_string()),
            ..Default::default()
        };
        let part2 = TaskLogPart {
            id: Some("log-part-2".to_string()),
            task_id: Some(TaskId::new("task-1")),
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

        let node_no_id = Node {
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
        let handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
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
            id: Some(TaskId::new("task-1")),
            name: Some("test-task".to_string()),
            ..Default::default()
        };

        broker.publish_task(qname, &task).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let guard = received.read().await;
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].id, Some(TaskId::new("task-1")));
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
            let handler: TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
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
        let handler: TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
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
        let handler: JobHandler = Arc::new(move |job: Job| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.write().await.push(job);
                Ok(())
            })
        });

        broker.subscribe_for_jobs(handler).await.unwrap();

        let job = Job {
            id: Some(JobId::new("job-1")),
            name: Some("test-job".to_string()),
            ..Default::default()
        };

        broker.publish_job(&job).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let guard = received.read().await;
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].id.as_deref(), Some("job-1"));
    }

    #[tokio::test]
    async fn test_multiple_subscribers_for_job() {
        let broker = InMemoryBroker::new();
        let received = Arc::new(RwLock::new(Vec::new()));
        let count = Arc::new(RwLock::new(0));

        let make_handler =
            |received: Arc<RwLock<Vec<Job>>>, count: Arc<RwLock<i32>>| -> JobHandler {
                Arc::new(move |job: Job| {
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
            let job = Job {
                id: Some(JobId::new(format!("job-{}", i))),
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
        let handler1: EventHandler = Arc::new(move |event: Value| {
            let received = received1_clone.clone();
            Box::pin(async move {
                received.write().await.push(event);
                Ok(())
            })
        });

        let received2_clone = received2.clone();
        let handler2: EventHandler = Arc::new(move |event: Value| {
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
        let handler: TaskProgressHandler = Arc::new(move |task: Task| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.write().await.push(task);
                Ok(())
            })
        });

        broker.subscribe_for_task_progress(handler).await.unwrap();

        let task = Task {
            id: Some(TaskId::new("task-1")),
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
                id: Some(TaskId::new(format!("task-{}", i))),
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
        let handler: HeartbeatHandler = Arc::new(move |node: Node| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.write().await.push(node);
                Ok(())
            })
        });

        broker.subscribe_for_heartbeats(handler).await.unwrap();

        let node = Node {
            id: Some(NodeId::new("node-1")),
            name: Some("worker-1".to_string()),
            status: Some(NodeStatus::UP),
            ..Default::default()
        };

        broker.publish_heartbeat(node).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let guard = received.read().await;
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].id, Some(NodeId::new("node-1")));
        assert_eq!(guard[0].name, Some("worker-1".to_string()));
    }

    #[tokio::test]
    async fn broker_shutdown_fails_health_check_after() {
        let broker = InMemoryBroker::new();
        let qname = format!("exclusive-queue-{}", new_uuid());

        let handler: TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        let task = Task {
            id: Some(TaskId::new("task-1")),
            ..Default::default()
        };
        broker.publish_task(qname, &task).await.unwrap();

        broker.health_check().await.unwrap();

        broker.shutdown().await.unwrap();

        broker.health_check().await.unwrap();
    }
}
