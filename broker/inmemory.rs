//! In-memory broker implementation.
//!
//! A simple broker implementation using in-memory channels for local
//! development, testing, etc.

use crate::broker::{
    is_coordinator_queue, queue, BoxedFuture, BoxedHandlerFuture, Broker, EventHandler,
    HeartbeatHandler, JobHandler, QueueInfo, TaskHandler, TaskLogPartHandler, TaskProgressHandler,
};
use crate::wildcard::match_pattern;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast::error::RecvError as BroadcastRecvError;
use tokio::sync::{broadcast, mpsc, RwLock};
use tork::task::TaskLogPart;

/// Default queue size for in-memory channels
const DEFAULT_QUEUE_SIZE: usize = 1000;

/// The in-memory broker implementation.
#[derive(Clone)]
pub struct InMemoryBroker {
    queues: Arc<RwLock<HashMap<String, Queue>>>,
    topics: Arc<RwLock<HashMap<String, Topic>>>,
    terminated: Arc<std::sync::atomic::AtomicBool>,
}

/// Queue subscription with terminate/terminated channels
struct QSub {
    /// Channel to signal termination
    terminate: mpsc::Sender<()>,
    /// Channel that's closed when terminated
    terminated: mpsc::Receiver<()>,
}

/// Internal queue structure
struct Queue {
    name: String,
    tx: broadcast::Sender<Arc<dyn Message + Send + Sync>>,
    /// Keep a receiver alive to ensure send() doesn't fail
    _rx: broadcast::Receiver<Arc<dyn Message + Send + Sync>>,
    subs: Mutex<Vec<QSub>>,
    /// Number of messages currently being processed by subscribers (unacked)
    unacked: Arc<AtomicUsize>,
    /// Number of messages in the queue (pending consumption)
    size: Arc<AtomicUsize>,
}

impl Queue {
    fn new(name: String) -> Self {
        let (tx, rx) = broadcast::channel(DEFAULT_QUEUE_SIZE);
        Self {
            name,
            tx,
            _rx: rx,
            subs: Mutex::new(Vec::new()),
            unacked: Arc::new(AtomicUsize::new(0)),
            size: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Add a subscription and return the terminate/terminated channels
    fn subscribe(
        &self,
        handler: Arc<
            dyn Fn(Arc<dyn std::any::Any + Send + Sync>) -> BoxedHandlerFuture
                + Send
                + Sync
                + 'static,
        >,
    ) -> (mpsc::Sender<()>, mpsc::Receiver<()>) {
        let (terminate_tx, mut terminate_rx) = mpsc::channel::<()>(1);
        let (terminated_tx, terminated_rx) = mpsc::channel::<()>(1);

        // Create the receiver BEFORE spawning the task
        let mut rx = self.tx.subscribe();
        let name = self.name.clone();
        let unacked = self.unacked.clone();
        let size = self.size.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = terminate_rx.recv() => {
                        tracing::debug!("queue {} subscription terminated", name);
                        let _ = terminated_tx.send(()).await;
                        return;
                    }
                    msg = rx.recv() => {
                        match msg {
                            Ok(m) => {
                                unacked.fetch_add(1, Ordering::SeqCst);
                                handler(m.clone().as_any()).await;
                                unacked.fetch_sub(1, Ordering::SeqCst);
                                size.fetch_sub(1, Ordering::SeqCst);
                            }
                            Err(BroadcastRecvError::Closed) => {
                                tracing::debug!("queue {} channel closed", name);
                                let _ = terminated_tx.send(()).await;
                                return;
                            }
                            Err(BroadcastRecvError::Lagged(_)) => {
                                // Subscriber lagged, continue
                            }
                        }
                    }
                }
            }
        });

        (terminate_tx, terminated_rx)
    }

    /// Close the queue and wait for coordinator subscriptions to terminate
    fn close(&self) {
        let mut subs = self.subs.lock().map_err(|_| "mutex poisoned").ok();
        if let Some(ref mut subs) = subs {
            for sub in subs.iter_mut() {
                let _ = sub.terminate.try_send(());
            }
            // Wait for coordinator queue subscriptions to terminate
            if is_coordinator_queue(&self.name) {
                for sub in subs.iter_mut() {
                    let _ = sub.terminated.try_recv();
                }
            }
        }
    }
}

/// Internal topic structure for pub/sub
///
/// Uses broadcast channels to deliver events to all subscribers.
/// When the Topic is dropped (e.g., on broker shutdown), the broadcast
/// sender is dropped, causing all subscriber receivers to get `Closed`
/// and their spawned tasks to exit cleanly.
struct Topic {
    #[allow(dead_code)]
    name: String,
    tx: broadcast::Sender<Arc<dyn Message + Send + Sync>>,
    /// Keep a receiver alive to ensure send() doesn't fail
    _rx: broadcast::Receiver<Arc<dyn Message + Send + Sync>>,
}

impl Topic {
    fn new(name: String) -> Self {
        let (tx, rx) = broadcast::channel(DEFAULT_QUEUE_SIZE);
        Self { name, tx, _rx: rx }
    }

    /// Close the topic. Subscribers are cleaned up when the Topic is dropped
    /// (which drops tx, causing all broadcast receivers to get Closed).
    #[allow(dead_code)]
    fn close(&self) {
        // Intentionally a no-op. The actual cleanup happens when the Topic
        // is dropped from the HashMap during shutdown, which drops tx and
        // causes all subscriber receivers to receive RecvError::Closed.
    }
}

/// Trait for message envelope
trait Message: Send + Sync {
    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync>;
}

impl<T: Clone + Send + Sync + 'static> Message for T {
    fn as_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        Arc::new((*self).clone())
    }
}

/// Create a new in-memory broker
#[must_use]
pub fn new_in_memory_broker() -> InMemoryBroker {
    InMemoryBroker {
        queues: Arc::new(RwLock::new(HashMap::new())),
        topics: Arc::new(RwLock::new(HashMap::new())),
        terminated: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
}

impl InMemoryBroker {
    /// Internal publish to a queue
    async fn publish_to_queue(
        &self,
        qname: &str,
        msg: Arc<dyn Message + Send + Sync>,
    ) -> Result<(), anyhow::Error> {
        let mut queues = self.queues.write().await;
        let (tx, size) = if let Some(q) = queues.get(qname) {
            (q.tx.clone(), q.size.clone())
        } else {
            let q = Queue::new(qname.to_string());
            let tx = q.tx.clone();
            let size = q.size.clone();
            queues.insert(qname.to_string(), q);
            (tx, size)
        };
        tx.send(msg).map_err(|_| anyhow::anyhow!("queue closed"))?;
        size.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Internal subscribe to a queue
    async fn subscribe_to_queue(
        &self,
        qname: &str,
        handler: Arc<
            dyn Fn(Arc<dyn std::any::Any + Send + Sync>) -> BoxedHandlerFuture
                + Send
                + Sync
                + 'static,
        >,
    ) -> Result<(), anyhow::Error> {
        let mut queues = self.queues.write().await;
        let queue = queues
            .entry(qname.to_string())
            .or_insert_with(|| Queue::new(qname.to_string()));

        let (terminate_tx, terminated_rx) = queue.subscribe(handler);
        if let Ok(mut subs) = queue.subs.lock() {
            subs.push(QSub {
                terminate: terminate_tx,
                terminated: terminated_rx,
            });
        }

        Ok(())
    }
}

impl Broker for InMemoryBroker {
    fn publish_task(&self, qname: String, task: &tork::task::Task) -> BoxedFuture<()> {
        let broker = self.clone();
        let task = task.deep_clone();
        let qname = qname.clone();
        Box::pin(async move {
            broker.publish_to_queue(&qname, Arc::new(task)).await?;
            Ok(())
        })
    }

    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
        let broker = self.clone();
        let qname = qname.clone();
        Box::pin(async move {
            broker
                .subscribe_to_queue(
                    &qname,
                    Arc::new(move |msg| {
                        let handler = handler.clone();
                        Box::pin(async move {
                            if let Some(task) = msg.downcast_ref::<tork::task::Task>() {
                                handler(Arc::new(task.deep_clone())).await;
                            }
                        })
                    }),
                )
                .await?;
            Ok(())
        })
    }

    fn publish_task_progress(&self, task: &tork::task::Task) -> BoxedFuture<()> {
        let broker = self.clone();
        let task = task.deep_clone();
        Box::pin(async move {
            broker
                .publish_to_queue(queue::QUEUE_PROGRESS, Arc::new(task))
                .await?;
            Ok(())
        })
    }

    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()> {
        let broker = self.clone();
        Box::pin(async move {
            broker
                .subscribe_to_queue(
                    queue::QUEUE_PROGRESS,
                    Arc::new(move |msg| {
                        let handler = handler.clone();
                        Box::pin(async move {
                            if let Some(task) = msg.downcast_ref::<tork::task::Task>() {
                                handler(task.deep_clone()).await;
                            }
                        })
                    }),
                )
                .await?;
            Ok(())
        })
    }

    fn publish_heartbeat(&self, node: tork::node::Node) -> BoxedFuture<()> {
        let broker = self.clone();
        let node = node.deep_clone();
        Box::pin(async move {
            broker
                .publish_to_queue(queue::QUEUE_HEARTBEAT, Arc::new(node))
                .await?;
            Ok(())
        })
    }

    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()> {
        let broker = self.clone();
        Box::pin(async move {
            broker
                .subscribe_to_queue(
                    queue::QUEUE_HEARTBEAT,
                    Arc::new(move |msg| {
                        let handler = handler.clone();
                        Box::pin(async move {
                            if let Some(node) = msg.downcast_ref::<tork::node::Node>() {
                                handler(node.deep_clone()).await;
                            }
                        })
                    }),
                )
                .await?;
            Ok(())
        })
    }

    fn publish_job(&self, job: &tork::job::Job) -> BoxedFuture<()> {
        let broker = self.clone();
        let job = job.deep_clone();
        Box::pin(async move {
            broker
                .publish_to_queue(queue::QUEUE_JOBS, Arc::new(job))
                .await?;
            Ok(())
        })
    }

    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()> {
        let broker = self.clone();
        Box::pin(async move {
            broker
                .subscribe_to_queue(
                    queue::QUEUE_JOBS,
                    Arc::new(move |msg| {
                        let handler = handler.clone();
                        Box::pin(async move {
                            if let Some(job) = msg.downcast_ref::<tork::job::Job>() {
                                handler(job.deep_clone()).await;
                            }
                        })
                    }),
                )
                .await?;
            Ok(())
        })
    }

    fn publish_event(&self, topic: String, event: serde_json::Value) -> BoxedFuture<()> {
        let topics = self.topics.clone();
        let topic_name = topic.clone();
        Box::pin(async move {
            let topics = topics.read().await;
            // Publish to all matching topics
            for (name, topic) in topics.iter() {
                if match_pattern(name, &topic_name) {
                    let _ = topic.tx.send(Arc::new(event.clone()));
                }
            }
            Ok(())
        })
    }

    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()> {
        let topics = self.topics.clone();
        Box::pin(async move {
            let tx = {
                let mut topics_guard = topics.write().await;
                if let Some(t) = topics_guard.get(&pattern) {
                    t.tx.clone()
                } else {
                    let t = Topic::new(pattern.clone());
                    let tx = t.tx.clone();
                    topics_guard.insert(pattern, t);
                    tx
                }
            };

            // Create the broadcast receiver BEFORE spawning the task.
            // This ensures the receiver is registered at the current tail
            // position, so any subsequent sends will be received.
            let mut rx = tx.subscribe();

            tokio::spawn(async move {
                while let Ok(msg) = rx.recv().await {
                    let value = msg
                        .clone()
                        .as_any()
                        .downcast::<serde_json::Value>()
                        .unwrap_or_else(|_| Arc::new(serde_json::Value::Null));
                    handler((*value).clone()).await;
                }
            });

            Ok(())
        })
    }

    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()> {
        let broker = self.clone();
        let part = part.clone();
        Box::pin(async move {
            broker
                .publish_to_queue(queue::QUEUE_LOGS, Arc::new(part))
                .await?;
            Ok(())
        })
    }

    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()> {
        let broker = self.clone();
        Box::pin(async move {
            broker
                .subscribe_to_queue(
                    queue::QUEUE_LOGS,
                    Arc::new(move |msg| {
                        let handler = handler.clone();
                        Box::pin(async move {
                            if let Some(part) = msg.downcast_ref::<TaskLogPart>() {
                                handler(part.clone()).await;
                            }
                        })
                    }),
                )
                .await?;
            Ok(())
        })
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        let queues = self.queues.clone();
        Box::pin(async move {
            let queues = queues.read().await;
            let result: Vec<QueueInfo> = queues
                .iter()
                .map(|(name, q)| {
                    let sub_count = q.subs.lock().map_or(0, |s| s.len()) as i64;
                    let unacked = q.unacked.load(Ordering::SeqCst) as i64;
                    let size = q.size.load(Ordering::SeqCst) as i64;
                    QueueInfo {
                        name: name.clone(),
                        size,
                        subscribers: sub_count,
                        unacked,
                    }
                })
                .collect();
            Ok(result)
        })
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        let queues = self.queues.clone();
        Box::pin(async move {
            let queues = queues.read().await;
            let qname_for_error = qname.clone();
            queues
                .get(&qname)
                .map(|q| {
                    let sub_count = q.subs.lock().map_or(0, |s| s.len()) as i64;
                    let unacked = q.unacked.load(Ordering::SeqCst) as i64;
                    let size = q.size.load(Ordering::SeqCst) as i64;
                    QueueInfo {
                        name: qname,
                        size,
                        subscribers: sub_count,
                        unacked,
                    }
                })
                .ok_or_else(|| anyhow::anyhow!("queue {} not found", qname_for_error))
        })
    }

    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        let queues = self.queues.clone();
        Box::pin(async move {
            let mut queues = queues.write().await;
            queues.remove(&qname);
            Ok(())
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        let terminated = self.terminated.clone();
        Box::pin(async move {
            if terminated.load(std::sync::atomic::Ordering::SeqCst) {
                Err(anyhow::anyhow!("broker is terminated"))
            } else {
                Ok(())
            }
        })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        let queues = self.queues.clone();
        let topics = self.topics.clone();
        let terminated = self.terminated.clone();
        Box::pin(async move {
            if !terminated
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                return Ok(());
            }

            // Close all queues with proper termination signaling
            {
                let mut queues_guard = queues.write().await;
                for (_name, queue) in queues_guard.iter() {
                    tracing::debug!("shutting down queue {}", queue.name);
                    queue.close();
                }
                queues_guard.clear();
            }

            // Close all topics - dropping them closes broadcast senders,
            // causing all subscriber receivers to get Closed
            {
                let topics_guard = topics.read().await;
                for (name, _topic) in topics_guard.iter() {
                    tracing::debug!("shutting down topic {}", name);
                }
            }
            // Drop all topics to close broadcast senders
            topics.write().await.clear();

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uuid::new_uuid;

    #[tokio::test]
    async fn test_publish_and_subscribe_task() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(std::sync::Mutex::new(false));
        let processed_clone = processed.clone();

        let qname = format!("test-queue-{}", new_uuid());
        let handler: TaskHandler = Arc::new(move |_task| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                *processed.lock().unwrap() = true;
            })
        });

        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        let task = tork::task::Task {
            id: Some(new_uuid()),
            ..Default::default()
        };

        broker.publish_task(qname, &task).await.unwrap();

        // Wait for processing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(*processed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_get_queues() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        let task = tork::task::Task::default();
        broker.publish_task(qname.clone(), &task).await.unwrap();

        let queues = broker.queues().await.unwrap();
        assert_eq!(1, queues.len());
        assert_eq!(qname, queues[0].name);
        assert_eq!(0, queues[0].subscribers);

        // Subscribe 10 concurrent subscribers (mirrors Go's TestInMemoryGetQueues)
        let mut handles = Vec::new();
        for _ in 0..10 {
            let broker_clone = broker.clone();
            let qname_clone = qname.clone();
            handles.push(tokio::spawn(async move {
                let handler: TaskHandler = Arc::new(move |_task| Box::pin(async move {}));
                broker_clone.subscribe_for_tasks(qname_clone, handler).await
            }));
        }
        for handle in handles {
            handle
                .await
                .expect("subscribe task should not panic")
                .expect("subscribe should succeed");
        }

        let queues = broker.queues().await.unwrap();
        assert_eq!(1, queues.len());
        assert_eq!(10, queues[0].subscribers);
    }

    #[tokio::test]
    async fn test_delete_queue() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        let task = tork::task::Task::default();
        broker.publish_task(qname.clone(), &task).await.unwrap();

        let queues = broker.queues().await.unwrap();
        assert_eq!(1, queues.len());

        broker.delete_queue(qname.clone()).await.unwrap();

        let queues = broker.queues().await.unwrap();
        assert!(queues.is_empty());
    }

    #[tokio::test]
    async fn test_publish_and_subscribe_heartbeat() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(std::sync::Mutex::new(false));
        let processed_clone = processed.clone();

        let handler: HeartbeatHandler = Arc::new(move |_node| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                *processed.lock().unwrap() = true;
            })
        });

        broker.subscribe_for_heartbeats(handler).await.unwrap();

        let node = tork::node::Node {
            id: Some(new_uuid()),
            name: None,
            started_at: time::OffsetDateTime::UNIX_EPOCH,
            cpu_percent: 0.0,
            last_heartbeat_at: time::OffsetDateTime::UNIX_EPOCH,
            queue: None,
            status: tork::node::NodeStatus::from("TEST"),
            hostname: None,
            port: 0,
            task_count: 0,
            version: String::new(),
        };

        broker.publish_heartbeat(node).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(*processed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_publish_and_subscribe_job() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(std::sync::Mutex::new(false));
        let processed_clone = processed.clone();

        let handler: JobHandler = Arc::new(move |_job| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                *processed.lock().unwrap() = true;
            })
        });

        broker.subscribe_for_jobs(handler).await.unwrap();

        let job = tork::job::Job::default();

        broker.publish_job(&job).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(*processed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_publish_and_subscribe_task_log_part() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(std::sync::Mutex::new(false));
        let processed_clone = processed.clone();

        let handler: TaskLogPartHandler = Arc::new(move |_part| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                *processed.lock().unwrap() = true;
            })
        });

        broker.subscribe_for_task_log_part(handler).await.unwrap();

        let part = TaskLogPart {
            id: Some(new_uuid()),
            number: 1,
            task_id: Some(new_uuid()),
            contents: Some("test log".to_string()),
            created_at: None,
        };

        broker.publish_task_log_part(&part).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(*processed.lock().unwrap());
    }

    #[tokio::test]
    async fn test_health_check() {
        let broker = new_in_memory_broker();
        broker.health_check().await.unwrap();

        broker.shutdown().await.unwrap();

        broker
            .health_check()
            .await
            .expect_err("should fail after shutdown");
    }

    /// Mirrors Go's TestInMemoryGetQueuesUnacked:
    /// Verifies that the unacked counter tracks messages currently being processed.
    #[tokio::test]
    async fn test_get_queues_unacked() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        // Publish a task (no subscribers yet)
        let task = tork::task::Task::default();
        broker.publish_task(qname.clone(), &task).await.unwrap();

        let queues = broker.queues().await.unwrap();
        assert_eq!(1, queues.len());
        assert_eq!(0, queues[0].subscribers);
        assert_eq!(0, queues[0].unacked);

        // Subscribe with a handler that blocks until signaled
        let started = Arc::new(tokio::sync::Notify::new());
        let started_clone = started.clone();
        let release = Arc::new(tokio::sync::Notify::new());
        let release_clone = release.clone();

        let handler: TaskHandler = Arc::new(move |_task| {
            let started = started_clone.clone();
            let release = release_clone.clone();
            Box::pin(async move {
                started.notify_one();
                release.notified().await;
            })
        });

        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        // Publish a task that the handler will pick up and block on
        broker.publish_task(qname.clone(), &task).await.unwrap();

        // Wait for handler to start processing
        started.notified().await;

        // While handler is running, unacked should be 1
        let queues = broker.queues().await.unwrap();
        assert_eq!(1, queues[0].unacked);
        assert_eq!(1, queues[0].subscribers);

        // Release the handler
        release.notify_one();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // After handler completes, unacked should be 0
        let queues = broker.queues().await.unwrap();
        assert_eq!(0, queues[0].unacked);
    }

    /// Mirrors Go's TestMultipleSubsSubsribeForJob:
    /// Verifies that multiple job subscribers all receive published jobs.
    #[tokio::test]
    async fn test_multiple_subs_subscribe_for_job() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(std::sync::Mutex::new(0_usize));
        let processed_clone = processed.clone();

        let handler: JobHandler = Arc::new(move |_job| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                let mut guard = processed.lock().expect("mutex not poisoned");
                *guard += 1;
            })
        });

        broker.subscribe_for_jobs(handler).await.unwrap();

        let processed_clone2 = processed.clone();
        let handler2: JobHandler = Arc::new(move |_job| {
            let processed = processed_clone2.clone();
            Box::pin(async move {
                let mut guard = processed.lock().expect("mutex not poisoned");
                *guard += 1;
            })
        });

        broker.subscribe_for_jobs(handler2).await.unwrap();

        // Publish 10 jobs
        for _ in 0..10 {
            let job = tork::job::Job::default();
            broker
                .publish_job(&job)
                .await
                .expect("publish should succeed");
        }

        // Wait for all processing (2 subscribers × 10 jobs = 20)
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let count = *processed.lock().expect("mutex not poisoned");
        assert_eq!(20, count);
    }

    /// Mirrors Go's TestInMemoryShutdown:
    /// Verifies that shutdown returns cleanly even when a subscriber is blocked,
    /// and that publish after shutdown does not hang.
    #[tokio::test]
    async fn test_shutdown() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(tokio::sync::Notify::new());
        let processed_clone = processed.clone();

        let qname1 = format!("{}test-{}", queue::QUEUE_EXCLUSIVE_PREFIX, new_uuid());
        let qname2 = format!("{}test-{}", queue::QUEUE_EXCLUSIVE_PREFIX, new_uuid());

        // Subscribe with a handler that blocks after signaling
        let handler: TaskHandler = Arc::new(move |_task| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                processed.notify_one();
                // Block forever (simulating long-running task processing)
                std::future::pending::<()>().await;
            })
        });

        broker
            .subscribe_for_tasks(qname1.clone(), handler)
            .await
            .unwrap();

        let task = tork::task::Task::default();
        // Publish 10 tasks to each queue
        for _ in 0..10 {
            broker.publish_task(qname1.clone(), &task).await.unwrap();
            broker.publish_task(qname2.clone(), &task).await.unwrap();
        }

        // Wait for at least one task to start processing
        processed.notified().await;

        // Shutdown should return cleanly, not block on the sleeping handler
        broker.shutdown().await.unwrap();

        // Publishing after shutdown should not hang
        broker.publish_task(qname1.clone(), &task).await.unwrap();
    }

    /// Mirrors Go's TestInMemorSubsribeForEvent:
    /// Verifies wildcard topic matching — "job.*" matches both
    /// "job.completed" and "job.failed", while "job.completed" only
    /// matches itself.
    #[tokio::test]
    async fn test_subscribe_for_event() {
        let broker = new_in_memory_broker();
        let processed1 = Arc::new(std::sync::Mutex::new(0_usize));
        let processed2 = Arc::new(std::sync::Mutex::new(0_usize));

        let processed1_clone = processed1.clone();
        let handler1: EventHandler = Arc::new(move |_event| {
            let processed = processed1_clone.clone();
            Box::pin(async move {
                let mut guard = processed.lock().expect("mutex not poisoned");
                *guard += 1;
            })
        });

        let processed2_clone = processed2.clone();
        let handler2: EventHandler = Arc::new(move |_event| {
            let processed = processed2_clone.clone();
            Box::pin(async move {
                let mut guard = processed.lock().expect("mutex not poisoned");
                *guard += 1;
            })
        });

        let topic_job = "job.*".to_string();
        let topic_job_completed = "job.completed".to_string();

        // Subscribe to "job.*" — should receive ALL job events
        broker
            .subscribe_for_events(topic_job.clone(), handler1)
            .await
            .unwrap();

        // Subscribe to "job.completed" — should only receive completed events
        broker
            .subscribe_for_events(topic_job_completed.clone(), handler2)
            .await
            .unwrap();

        // Publish 10 completed + 10 failed events
        for _ in 0..10 {
            let event = serde_json::json!({"id": new_uuid()});
            broker
                .publish_event(topic_job_completed.clone(), event.clone())
                .await
                .unwrap();
            broker
                .publish_event("job.failed".to_string(), event.clone())
                .await
                .unwrap();
        }

        // Wait for all events to be processed
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // "job.*" subscriber should have received 20 events
        assert_eq!(20, *processed1.lock().expect("mutex not poisoned"));
        // "job.completed" subscriber should have received 10 events
        assert_eq!(10, *processed2.lock().expect("mutex not poisoned"));
    }

    /// Mirrors Go's TestInMemoryPublishAndSubsribeTaskProgress:
    /// Verifies end-to-end publish/subscribe for task progress messages.
    #[tokio::test]
    async fn test_publish_and_subscribe_task_progress() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(std::sync::Mutex::new(false));
        let processed_clone = processed.clone();

        let handler: TaskProgressHandler = Arc::new(move |_task| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                *processed.lock().expect("mutex not poisoned") = true;
            })
        });

        broker.subscribe_for_task_progress(handler).await.unwrap();

        let task = tork::task::Task {
            id: Some(new_uuid()),
            ..Default::default()
        };

        broker.publish_task_progress(&task).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert!(*processed.lock().expect("mutex not poisoned"));
    }

    /// Mirrors Go's TestInMemoryPublishAndSubsribeForTask with mounts:
    /// Verifies that a task with mounts is published and received correctly,
    /// and that mount target data survives the serialization round-trip.
    #[tokio::test]
    async fn test_publish_and_subscribe_task_with_mounts() {
        let broker = new_in_memory_broker();
        let received_task = Arc::new(std::sync::Mutex::new(None));
        let received_clone = received_task.clone();

        let qname = format!("test-queue-{}", new_uuid());
        let handler: TaskHandler = Arc::new(move |task| {
            let received = received_clone.clone();
            Box::pin(async move {
                let mut guard = received.lock().expect("mutex not poisoned");
                *guard = Some(task.deep_clone());
            })
        });

        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        let task = tork::task::Task {
            id: Some(new_uuid()),
            mounts: Some(vec![tork::mount::Mount {
                mount_type: tork::mount::MOUNT_TYPE_VOLUME.to_string(),
                target: Some("/somevolume".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        broker.publish_task(qname, &task).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let guard = received_task.lock().expect("mutex not poisoned");
        let received = guard.as_ref().expect("should have received a task");
        let mounts = received.mounts.as_ref().expect("should have mounts");
        assert_eq!(
            "/somevolume",
            mounts[0].target.as_ref().expect("mount should have target")
        );
    }

    /// Mirrors Go's TestInMemoryDeleteQueue with subscribers:
    /// Verifies that deleting a queue with subscribers removes it completely.
    #[tokio::test]
    async fn test_delete_queue_with_subscribers() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        // Subscribe first (creates queue)
        let handler: TaskHandler = Arc::new(move |_task| Box::pin(async move {}));
        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        let queues = broker.queues().await.unwrap();
        assert_eq!(1, queues.len());
        assert_eq!(1, queues[0].subscribers);

        // Delete the queue
        broker.delete_queue(qname.clone()).await.unwrap();

        let queues = broker.queues().await.unwrap();
        assert!(queues.is_empty());
    }

    /// Verifies that publishing to a deleted queue creates a fresh queue.
    #[tokio::test]
    async fn test_publish_after_delete_queue() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        let task = tork::task::Task::default();
        broker.publish_task(qname.clone(), &task).await.unwrap();
        broker.delete_queue(qname.clone()).await.unwrap();

        let queues = broker.queues().await.unwrap();
        assert!(queues.is_empty());

        // Re-publish should create a new queue
        broker.publish_task(qname.clone(), &task).await.unwrap();
        let queues = broker.queues().await.unwrap();
        assert_eq!(1, queues.len());
    }

    /// Verifies that shutdown is idempotent (calling it twice doesn't error).
    #[tokio::test]
    async fn test_shutdown_idempotent() {
        let broker = new_in_memory_broker();
        broker.shutdown().await.unwrap();
        broker.shutdown().await.unwrap();
    }

    /// Verifies that shutdown with health_check returns an error.
    #[tokio::test]
    async fn test_health_check_after_shutdown() {
        let broker = new_in_memory_broker();
        broker.shutdown().await.unwrap();
        let result = broker.health_check().await;
        assert!(result.is_err(), "health check should fail after shutdown");
    }

    /// Tests queue_info for a specific queue.
    #[tokio::test]
    async fn test_queue_info() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        let task = tork::task::Task::default();
        broker.publish_task(qname.clone(), &task).await.unwrap();

        let info = broker.queue_info(qname.clone()).await.unwrap();
        assert_eq!(qname, info.name);
        assert_eq!(1, info.size);
        assert_eq!(0, info.subscribers);
        assert_eq!(0, info.unacked);
    }

    /// Tests queue_info returns error for non-existent queue.
    #[tokio::test]
    async fn test_queue_info_not_found() {
        let broker = new_in_memory_broker();
        let result = broker.queue_info("nonexistent-queue".to_string()).await;
        assert!(
            result.is_err(),
            "queue_info should error for non-existent queue"
        );
    }

    /// Tests that queues() returns all queues including empty ones.
    #[tokio::test]
    async fn test_queues_includes_empty() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        // Create queue by subscribing (no messages)
        let handler: TaskHandler = Arc::new(move |_task| Box::pin(async move {}));
        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        let queues = broker.queues().await.unwrap();
        assert!(queues.iter().any(|q| q.name == qname && q.size == 0));
    }

    /// Tests multiple concurrent publishers to the same queue.
    #[tokio::test]
    async fn test_concurrent_publish() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());
        let received = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let received_clone = received.clone();

        let handler: TaskHandler = Arc::new(move |_task| {
            let received = received_clone.clone();
            Box::pin(async move {
                received.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })
        });

        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        // Publish 100 tasks concurrently
        let mut handles = Vec::new();
        for _ in 0..100 {
            let broker_clone = broker.clone();
            let qname_clone = qname.clone();
            handles.push(tokio::spawn(async move {
                let task = tork::task::Task::default();
                broker_clone
                    .publish_task(qname_clone, &task)
                    .await
                    .expect("publish should succeed");
            }));
        }
        for handle in handles {
            handle.await.expect("task should not panic");
        }

        // Wait for processing
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert_eq!(100, received.load(std::sync::atomic::Ordering::SeqCst));
    }

    /// Tests that queue size decrements after message is consumed.
    #[tokio::test]
    async fn test_queue_size_decrement_on_consume() {
        let broker = new_in_memory_broker();
        let qname = format!("test-queue-{}", new_uuid());

        // Subscribe first so we receive the message
        let done = Arc::new(tokio::sync::Notify::new());
        let done_clone = done.clone();

        let handler: TaskHandler = Arc::new(move |_task| {
            let done = done_clone.clone();
            Box::pin(async move {
                done.notify_one();
            })
        });

        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        // Now publish - the subscriber will receive it
        let task = tork::task::Task::default();
        broker.publish_task(qname.clone(), &task).await.unwrap();

        // Wait for message to be consumed
        done.notified().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let info = broker.queue_info(qname.clone()).await.unwrap();
        assert_eq!(0, info.size);
    }

    /// Tests that delete_queue on non-existent queue succeeds (idempotent).
    #[tokio::test]
    async fn test_delete_nonexistent_queue() {
        let broker = new_in_memory_broker();
        let result = broker.delete_queue("nonexistent-queue".to_string()).await;
        assert!(
            result.is_ok(),
            "delete should succeed on non-existent queue"
        );
    }

    /// Tests that multiple queues with different names are tracked separately.
    #[tokio::test]
    async fn test_multiple_queues() {
        let broker = new_in_memory_broker();
        let qname1 = format!("test-queue-1-{}", new_uuid());
        let qname2 = format!("test-queue-2-{}", new_uuid());

        let task = tork::task::Task::default();
        broker.publish_task(qname1.clone(), &task).await.unwrap();
        broker.publish_task(qname1.clone(), &task).await.unwrap();
        broker.publish_task(qname2.clone(), &task).await.unwrap();

        let queues = broker.queues().await.unwrap();
        assert_eq!(2, queues.len());

        let info1 = broker.queue_info(qname1.clone()).await.unwrap();
        let info2 = broker.queue_info(qname2.clone()).await.unwrap();

        assert_eq!(2, info1.size);
        assert_eq!(1, info2.size);
    }

    /// Tests event subscription with exact topic match (no wildcard).
    #[tokio::test]
    async fn test_subscribe_for_event_exact_topic() {
        let broker = new_in_memory_broker();
        let processed = Arc::new(std::sync::Mutex::new(0_usize));
        let processed_clone = processed.clone();

        let handler: EventHandler = Arc::new(move |_event| {
            let processed = processed_clone.clone();
            Box::pin(async move {
                let mut guard = processed.lock().expect("mutex not poisoned");
                *guard += 1;
            })
        });

        let topic = "task.started".to_string();
        broker
            .subscribe_for_events(topic.clone(), handler)
            .await
            .unwrap();

        // Publish to exact topic
        for _ in 0..5 {
            let event = serde_json::json!({"id": new_uuid()});
            broker.publish_event(topic.clone(), event).await.unwrap();
        }

        // Publish to different topic - should NOT be received
        broker
            .publish_event("task.stopped".to_string(), serde_json::json!({}))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert_eq!(5, *processed.lock().expect("mutex not poisoned"));
    }

    /// Tests that tasks with priority are published and received correctly.
    #[tokio::test]
    async fn test_publish_task_with_priority() {
        let broker = new_in_memory_broker();
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let qname = format!("test-queue-{}", new_uuid());
        let handler: TaskHandler = Arc::new(move |task| {
            let received = received_clone.clone();
            Box::pin(async move {
                let mut guard = received.lock().expect("mutex not poisoned");
                guard.push(task.priority);
            })
        });

        broker
            .subscribe_for_tasks(qname.clone(), handler)
            .await
            .unwrap();

        // Publish tasks with different priorities
        for p in [5_i64, 3, 9, 1, 7] {
            let task = tork::task::Task {
                id: Some(new_uuid()),
                priority: p,
                ..Default::default()
            };
            broker.publish_task(qname.clone(), &task).await.unwrap();
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let mut priorities = received.lock().expect("mutex not poisoned");
        priorities.sort();
        assert_eq!(vec![1, 3, 5, 7, 9], *priorities);
    }
}
