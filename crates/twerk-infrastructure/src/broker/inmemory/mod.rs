//! In-memory broker implementation for testing and single-process usage.

mod ack;
mod consume;
mod publish;
mod subscription;
#[cfg(test)]
mod tests;

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{
    BoxedFuture, Broker, EventHandler, HeartbeatHandler, JobHandler, QueueInfo, TaskHandler,
    TaskLogPartHandler, TaskProgressHandler,
};
use twerk_core::job::JobEvent;
use twerk_core::node::Node;
use twerk_core::task::TaskLogPart;

/// In-memory broker implementation for testing and single-process usage.
///
/// # Concurrent Access Pattern
///
/// This broker uses two different synchronization strategies:
///
/// - **`Arc<RwLock<DashMap>`** for `heartbeats` and `task_log_parts`: The
///   `RwLock` ensures consistent iteration by holding a read lock during
///   iteration. Without it, `DashMap`'s per-shard locking could cause missed
///   entries when iterating over a cloned handle.
///
/// - **`Arc<RwLock<Vec<...>>>`** for handler registries (`job_handlers`,
///   `progress_handlers`, `heartbeat_handlers`, `task_log_part_handlers`):
///   Vectors require explicit synchronization because they don't have
///   `DashMap`'s internal locking.
///
/// - **`DashMap`** directly for `tasks`, `handlers`, and `event_handlers`:
///   These use `DashMap`'s internal synchronization.
///
/// - **`DashMap`** for `typed_event_channels`: Maps topic patterns to
///   `tokio::sync::broadcast` senders for typed `JobEvent` streams.
pub struct InMemoryBroker {
    /// Queue name -> list of tasks
    pub(crate) tasks: DashMap<String, Vec<Arc<twerk_core::task::Task>>>,
    /// Queue name -> list of task handlers
    pub(crate) handlers: DashMap<String, Vec<TaskHandler>>,
    /// Job handlers
    pub(crate) job_handlers: Arc<RwLock<Vec<JobHandler>>>,
    /// Task progress handlers
    pub(crate) progress_handlers: Arc<RwLock<Vec<TaskProgressHandler>>>,
    /// Event handlers (topic pattern -> list of handlers)
    pub(crate) event_handlers: Arc<DashMap<String, Vec<EventHandler>>>,
    /// Typed event channels (topic pattern -> broadcast sender for `JobEvent`)
    pub(crate) typed_event_channels: Arc<DashMap<String, tokio::sync::broadcast::Sender<JobEvent>>>,
    /// Heartbeat handlers
    pub(crate) heartbeat_handlers: Arc<RwLock<Vec<HeartbeatHandler>>>,
    /// Stored heartbeats (`node_id` -> node) — `RwLock` ensures consistent iteration
    pub(crate) heartbeats: Arc<RwLock<DashMap<String, Node>>>,
    /// Task log part handlers
    pub(crate) task_log_part_handlers: Arc<RwLock<Vec<TaskLogPartHandler>>>,
    /// Stored task log parts (`task_id` -> Vec<TaskLogPart>) — `RwLock` ensures consistent iteration
    pub(crate) task_log_parts: Arc<RwLock<DashMap<String, Vec<TaskLogPart>>>>,
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
            typed_event_channels: Arc::new(DashMap::new()),
            heartbeat_handlers: Arc::new(RwLock::new(Vec::new())),
            heartbeats: Arc::new(RwLock::new(DashMap::new())),
            task_log_part_handlers: Arc::new(RwLock::new(Vec::new())),
            task_log_parts: Arc::new(RwLock::new(DashMap::new())),
        }
    }
}

impl Broker for InMemoryBroker {
    fn publish_task(&self, qname: String, task: &twerk_core::task::Task) -> BoxedFuture<()> {
        publish::task(self, &qname, task)
    }

    fn publish_tasks(&self, qname: String, tasks: &[twerk_core::task::Task]) -> BoxedFuture<()> {
        publish::tasks(self, &qname, tasks)
    }

    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
        subscription::for_tasks(self, qname, handler)
    }

    fn publish_task_progress(&self, task: &twerk_core::task::Task) -> BoxedFuture<()> {
        publish::task_progress(self, task)
    }

    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()> {
        subscription::for_task_progress(self, handler)
    }

    fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()> {
        publish::heartbeat(self, node)
    }

    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()> {
        subscription::for_heartbeats(self, handler)
    }

    fn publish_job(&self, job: &twerk_core::job::Job) -> BoxedFuture<()> {
        publish::job(self, job)
    }

    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()> {
        subscription::for_jobs(self, handler)
    }

    fn publish_event(&self, topic: String, event: serde_json::Value) -> BoxedFuture<()> {
        publish::event(self, topic, event)
    }

    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()> {
        subscription::for_events(self, pattern, handler)
    }

    fn subscribe(
        &self,
        pattern: String,
    ) -> BoxedFuture<tokio::sync::broadcast::Receiver<JobEvent>> {
        subscription::typed_events(self, &pattern)
    }

    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()> {
        publish::task_log_part(self, part)
    }

    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()> {
        subscription::for_task_log_part(self, handler)
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        consume::queues(self)
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        consume::queue_info(self, qname)
    }

    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        consume::delete_queue(self, qname)
    }

    fn health_check(&self) -> BoxedFuture<()> {
        ack::health_check()
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        ack::shutdown()
    }
}
