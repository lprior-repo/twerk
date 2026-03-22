//! In-memory broker implementation.
//!
//! A simple broker implementation using in-memory channels for local
//! development, testing, etc.

use crate::broker::{
    is_coordinator_queue, queue, Broker, BoxedFuture, BoxedHandlerFuture, EventHandler,
    HeartbeatHandler, JobHandler, QueueInfo, TaskHandler, TaskLogPartHandler, TaskProgressHandler,
};
use tork::task::TaskLogPart;
use crate::wildcard::match_pattern;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, RwLock};

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
    tx: mpsc::Sender<Arc<dyn Message + Send + Sync>>,
    subs: Mutex<Vec<QSub>>,
}

impl Queue {
    fn new(name: String) -> (Self, mpsc::Receiver<Arc<dyn Message + Send + Sync>>) {
        let (tx, rx) = mpsc::channel(DEFAULT_QUEUE_SIZE);
        (Self { name, tx, subs: Mutex::new(Vec::new()) }, rx)
    }

    /// Add a subscription and return the terminate/terminated channels
    fn subscribe(&self, handler: Arc<dyn Fn(Arc<dyn std::any::Any + Send + Sync>) -> BoxedHandlerFuture + Send + Sync + 'static>) -> (mpsc::Sender<()>, mpsc::Receiver<()>)
    {
        let (terminate_tx, mut terminate_rx) = mpsc::channel::<()>(1);
        let (terminated_tx, terminated_rx) = mpsc::channel::<()>(1);

        let tx = self.tx.clone();
        let name = self.name.clone();

        tokio::spawn(async move {
            let mut rx = tx.subscribe();
            loop {
                tokio::select! {
                    _ = terminate_rx.recv() => {
                        tracing::debug!("queue {} subscription terminated", name);
                        let _ = terminated_tx.send(()).await;
                        return;
                    }
                    msg = rx.recv() => {
                        match msg {
                            Some(m) => {
                                handler(m.as_any()).await;
                            }
                            None => {
                                tracing::debug!("queue {} channel closed", name);
                                let _ = terminated_tx.send(()).await;
                                return;
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
        for sub in self.subs.lock().unwrap().iter() {
            let _ = sub.terminate.try_send(());
        }
        // Wait for coordinator queue subscriptions to terminate
        if is_coordinator_queue(&self.name) {
            for sub in self.subs.lock().unwrap().iter() {
                let _ = sub.terminated.try_recv();
            }
        }
    }
}

/// Internal topic structure for pub/sub
struct Topic {
    name: String,
    tx: mpsc::Sender<Arc<dyn Message + Send + Sync>>,
    /// Channel to signal termination
    terminate: mpsc::Sender<()>,
    /// Channel that's closed when terminated
    terminated: mpsc::Receiver<()>,
}

impl Topic {
    fn new(name: String) -> (Self, mpsc::Receiver<Arc<dyn Message + Send + Sync>>) {
        let (tx, rx) = mpsc::channel(DEFAULT_QUEUE_SIZE);
        let (terminate_tx, terminate_rx) = mpsc::channel::<()>(1);
        let (terminated_tx, terminated_rx) = mpsc::channel::<()>(1);

        let name_clone = name.clone();
        tokio::spawn(async move {
            let mut rx = rx;
            let mut terminate_rx = terminate_rx;
            loop {
                tokio::select! {
                    _ = terminate_rx.recv() => {
                        tracing::debug!("topic {} terminated", name_clone);
                        let _ = terminated_tx.send(()).await;
                        return;
                    }
                    msg = rx.recv() => {
                        match msg {
                            Some(m) => {
                                let _ = tx.send(m).await;
                            }
                            None => {
                                tracing::debug!("topic {} channel closed", name_clone);
                                let _ = terminated_tx.send(()).await;
                                return;
                            }
                        }
                    }
                }
            }
        });

        (Self { name, tx, terminate: terminate_tx, terminated: terminated_rx }, rx)
    }

    /// Close the topic and wait for termination
    fn close(&mut self) {
        let _ = self.terminate.try_send(());
        // Dropping tx will close all subscriptions so subscribers receive None
        drop(&self.tx);
        let _ = self.terminated.try_recv();
    }
}

/// Trait for message envelope
trait Message: Send + Sync {
    fn as_any(&self) -> Arc<dyn std::any::Any + Send + Sync>;
}

impl<T: Send + Sync + 'static> Message for T {
    fn as_any(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        Arc::new(self.clone())
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
    async fn publish_to_queue(&self, qname: &str, msg: Arc<dyn Message + Send + Sync>) -> Result<(), anyhow::Error> {
        let mut queues = self.queues.write().await;
        let tx = if let Some(q) = queues.get(qname) {
            q.tx.clone()
        } else {
            let (q, _rx) = Queue::new(qname.to_string());
            let tx = q.tx.clone();
            queues.insert(qname.to_string(), q);
            tx
        };
        tx.send(msg).await.map_err(|_| anyhow::anyhow!("queue closed"))?;
        Ok(())
    }

    /// Internal subscribe to a queue
    async fn subscribe_to_queue(
        &self,
        qname: &str,
        handler: Arc<dyn Fn(Arc<dyn std::any::Any + Send + Sync>) -> BoxedHandlerFuture + Send + Sync + 'static>,
    ) -> Result<(), anyhow::Error> {
        let mut queues = self.queues.write().await;
        let queue = if let Some(q) = queues.get(qname) {
            q
        } else {
            let (q, _rx) = Queue::new(qname.to_string());
            queues.insert(qname.to_string(), q);
            queues.get(qname).unwrap()
        };

        let (terminate_tx, terminated_rx) = queue.subscribe(handler);
        queue.subs.lock().unwrap().push(QSub { terminate: terminate_tx, terminated: terminated_rx });

        Ok(())
    }
}

impl Broker for InMemoryBroker {
    fn publish_task(&self, qname: String, task: &tork::task::Task) -> BoxedFuture<()> {
        let task = task.deep_clone();
        let qname = qname.clone();
        Box::pin(async move {
            self.publish_to_queue(&qname, Arc::new(task)).await?;
            Ok(())
        })
    }

    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
        let qname = qname.clone();
        Box::pin(async move {
            self.subscribe_to_queue(&qname, Arc::new(move |msg| {
                let handler = handler.clone();
                Box::pin(async move {
                    if let Some(task) = msg.downcast_ref::<tork::task::Task>() {
                        handler(task.deep_clone()).await;
                    }
                })
            }))
            .await?;
            Ok(())
        })
    }

    fn publish_task_progress(&self, task: &tork::task::Task) -> BoxedFuture<()> {
        let task = task.deep_clone();
        Box::pin(async move {
            self.publish_to_queue(queue::QUEUE_PROGRESS, Arc::new(task)).await?;
            Ok(())
        })
    }

    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            self.subscribe_to_queue(queue::QUEUE_PROGRESS, Arc::new(move |msg| {
                let handler = handler.clone();
                Box::pin(async move {
                    if let Some(task) = msg.downcast_ref::<tork::task::Task>() {
                        handler(task.deep_clone()).await;
                    }
                })
            }))
            .await?;
            Ok(())
        })
    }

    fn publish_heartbeat(&self, node: tork::node::Node) -> BoxedFuture<()> {
        let node = node.deep_clone();
        Box::pin(async move {
            self.publish_to_queue(queue::QUEUE_HEARTBEAT, Arc::new(node)).await?;
            Ok(())
        })
    }

    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            self.subscribe_to_queue(queue::QUEUE_HEARTBEAT, Arc::new(move |msg| {
                let handler = handler.clone();
                Box::pin(async move {
                    if let Some(node) = msg.downcast_ref::<tork::node::Node>() {
                        handler(node.deep_clone()).await;
                    }
                })
            }))
            .await?;
            Ok(())
        })
    }

    fn publish_job(&self, job: &tork::job::Job) -> BoxedFuture<()> {
        let job = job.deep_clone();
        Box::pin(async move {
            self.publish_to_queue(queue::QUEUE_JOBS, Arc::new(job)).await?;
            Ok(())
        })
    }

    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            self.subscribe_to_queue(queue::QUEUE_JOBS, Arc::new(move |msg| {
                let handler = handler.clone();
                Box::pin(async move {
                    if let Some(job) = msg.downcast_ref::<tork::job::Job>() {
                        handler(job.deep_clone()).await;
                    }
                })
            }))
            .await?;
            Ok(())
        })
    }

    fn publish_event(&self, topic: String, event: serde_json::Value) -> BoxedFuture<()> {
        let topic_name = topic.clone();
        Box::pin(async move {
            let topics = self.topics.read().await;
            // Publish to all matching topics
            for (name, topic) in topics.iter() {
                if match_pattern(name, &topic_name) {
                    let _ = topic.tx.send(Arc::new(event.clone())).await;
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
                    let (t, _rx) = Topic::new(pattern.clone());
                    let tx = t.tx.clone();
                    topics_guard.insert(pattern, t);
                    tx
                }
            };

            tokio::spawn(async move {
                let mut rx = tx.subscribe();
                while let Some(msg) = rx.recv().await {
                    handler(msg.as_any().downcast::<serde_json::Value>().unwrap_or_else(|_| Arc::new(serde_json::Value::Null))).await;
                }
            });

            Ok(())
        })
    }

    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()> {
        let part = part.clone();
        Box::pin(async move {
            self.publish_to_queue(queue::QUEUE_LOGS, Arc::new(part)).await?;
            Ok(())
        })
    }

    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            self.subscribe_to_queue(queue::QUEUE_LOGS, Arc::new(move |msg| {
                let handler = handler.clone();
                Box::pin(async move {
                    if let Some(part) = msg.downcast_ref::<TaskLogPart>() {
                        handler(part.clone()).await;
                    }
                })
            }))
            .await?;
            Ok(())
        })
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        Box::pin(async move {
            let queues = self.queues.read().await;
            let result: Vec<QueueInfo> = queues
                .iter()
                .map(|(name, q)| QueueInfo {
                    name: name.clone(),
                    size: q.tx.capacity() as i64,
                    subscribers: 0,
                    unacked: 0,
                })
                .collect();
            Ok(result)
        })
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        Box::pin(async move {
            let queues = self.queues.read().await;
            queues
                .get(&qname)
                .map(|q| QueueInfo {
                    name: qname,
                    size: q.tx.capacity() as i64,
                    subscribers: 0,
                    unacked: 0,
                })
                .ok_or_else(|| anyhow::anyhow!("queue {} not found", qname))
        })
    }

    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        Box::pin(async move {
            let mut queues = self.queues.write().await;
            queues.remove(&qname);
            Ok(())
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async move {
            if self.terminated.load(std::sync::atomic::Ordering::SeqCst) {
                Err(anyhow::anyhow!("broker is terminated"))
            } else {
                Ok(())
            }
        })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        let queues = self.queues.clone();
        let topics = self.topics.clone();
        Box::pin(async move {
            if !self.terminated.compare_exchange(false, true, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() {
                return Ok(());
            }

            // Close all queues with proper termination signaling
            {
                let mut queues_guard = queues.write().await;
                for (_name, queue) in queues_guard.iter_mut() {
                    tracing::debug!("shutting down queue {}", queue.name);
                    queue.close();
                }
                queues_guard.clear();
            }

            // Close all topics with proper termination signaling
            {
                let mut topics_guard = topics.write().await;
                for (_name, topic) in topics_guard.iter_mut() {
                    tracing::debug!("shutting down topic {}", topic.name);
                    topic.close();
                }
                topics_guard.clear();
            }

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

        broker.subscribe_for_tasks(qname.clone(), handler).await.unwrap();

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
            ..Default::default()
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

        broker.health_check().await.expect_err("should fail after shutdown");
    }
}
