use std::sync::Arc;
use tokio::sync::RwLock;
use dashmap::DashMap;
use tracing::debug;
use serde_json::Value;

use twerk_core::task::{Task, TaskLogPart};
use twerk_core::node::Node;
use twerk_core::job::Job;
use twerk_common::wildcard::wildcard_match;
use super::{Broker, BoxedFuture, TaskHandler, TaskProgressHandler, HeartbeatHandler, JobHandler, EventHandler, TaskLogPartHandler, QueueInfo};

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
        self.handlers
            .entry(qname)
            .or_default()
            .push(handler);
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

    fn publish_heartbeat(&self, _node: Node) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_heartbeats(&self, _handler: HeartbeatHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_job(&self, job: &Job) -> BoxedFuture<()> {
        let job = job.clone();
        let handlers = self.job_handlers.clone();
        Box::pin(async move {
            let handlers = handlers.read().await;
            debug!("Publishing job {} to {} handlers", job.id.as_deref().unwrap_or("unknown"), handlers.len());
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

    fn publish_task_log_part(&self, _part: &TaskLogPart) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_task_log_part(&self, _handler: TaskLogPartHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
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
