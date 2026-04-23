//! Core [`Broker`] trait definition.

use serde_json::Value;
use std::sync::Arc;

use twerk_core::job::{Job, JobEvent};
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

use super::queue::QueueInfo;
use super::types::{
    BoxedFuture, EventHandler, HeartbeatHandler, JobHandler, TaskHandler, TaskLogPartHandler,
    TaskProgressHandler,
};

pub trait Broker: Send + Sync {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()>;
    fn publish_tasks(&self, qname: String, tasks: &[Task]) -> BoxedFuture<()> {
        let qname = Arc::new(qname);
        let futures: Vec<_> = tasks
            .iter()
            .map(|t| {
                let q = Arc::clone(&qname);
                self.publish_task((*q).clone(), t)
            })
            .collect();
        Box::pin(async move {
            futures_util::future::try_join_all(futures).await?;
            Ok(())
        })
    }
    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()>;
    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()>;
    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()>;
    fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()>;
    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()>;
    fn publish_job(&self, job: &Job) -> BoxedFuture<()>;
    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()>;
    fn publish_event(&self, topic: String, event: Value) -> BoxedFuture<()>;
    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()>;
    /// Subscribe to typed job events matching a topic pattern.
    ///
    /// Returns a `broadcast::Receiver` that yields `JobEvent` values.
    /// This is the typed replacement for the `subscribe_for_events` callback
    /// pattern, allowing consumers to filter events with match expressions
    /// instead of deserializing raw JSON values.
    fn subscribe(&self, pattern: String)
        -> BoxedFuture<tokio::sync::broadcast::Receiver<JobEvent>>;
    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()>;
    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()>;
    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>>;
    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo>;
    fn delete_queue(&self, qname: String) -> BoxedFuture<()>;
    fn health_check(&self) -> BoxedFuture<()>;
    fn shutdown(&self) -> BoxedFuture<()>;
}
