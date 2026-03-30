//! Broker module for message queue and pub/sub functionality.
//!
//! This module provides broker implementations for delivering tasks
//! and coordinating between workers and the coordinator.

use anyhow::Result;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use twerk_core::job::Job;
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

pub type BoxedFuture<T> = Pin<Box<dyn Future<Output = Result<T>> + Send>>;
pub type BoxedHandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

pub type TaskHandler = Arc<dyn Fn(Arc<Task>) -> BoxedHandlerFuture + Send + Sync>;
pub type TaskProgressHandler = Arc<dyn Fn(Task) -> BoxedHandlerFuture + Send + Sync>;
pub type HeartbeatHandler = Arc<dyn Fn(Node) -> BoxedHandlerFuture + Send + Sync>;
pub type JobHandler = Arc<dyn Fn(Job) -> BoxedHandlerFuture + Send + Sync>;
pub type EventHandler = Arc<dyn Fn(Value) -> BoxedHandlerFuture + Send + Sync>;
pub type TaskLogPartHandler = Arc<dyn Fn(TaskLogPart) -> BoxedHandlerFuture + Send + Sync>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub size: i32,
    pub subscribers: i32,
    pub unacked: i32,
}

pub mod queue {
    pub const QUEUE_PENDING: &str = "x-pending";
    pub const QUEUE_COMPLETED: &str = "x-completed";
    pub const QUEUE_FAILED: &str = "x-failed";
    pub const QUEUE_STARTED: &str = "x-started";
    pub const QUEUE_HEARTBEAT: &str = "x-heartbeat";
    pub const QUEUE_JOBS: &str = "x-jobs";
    pub const QUEUE_PROGRESS: &str = "x-progress";
    pub const QUEUE_TASK_LOG_PART: &str = "x-task_log_part";
    pub const QUEUE_LOGS: &str = "x-task_log_part";
    pub const QUEUE_EXCLUSIVE_PREFIX: &str = "x-exclusive.";
    pub const QUEUE_REDELIVERIES: &str = "x-redeliveries";
}

pub fn is_coordinator_queue(qname: &str) -> bool {
    matches!(
        qname,
        queue::QUEUE_COMPLETED
            | queue::QUEUE_FAILED
            | queue::QUEUE_STARTED
            | queue::QUEUE_HEARTBEAT
            | queue::QUEUE_JOBS
            | queue::QUEUE_PROGRESS
            | queue::QUEUE_TASK_LOG_PART
            | queue::QUEUE_REDELIVERIES
    )
}

pub fn is_worker_queue(qname: &str) -> bool {
    !is_coordinator_queue(qname) && !qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX)
}

pub fn is_task_queue(qname: &str) -> bool {
    is_worker_queue(qname)
}

#[derive(Debug, Clone, Default)]
pub struct RabbitMQOptions {
    pub management_url: Option<String>,
    pub durable_queues: bool,
    pub queue_type: String,
    pub consumer_timeout: Option<std::time::Duration>,
}

pub trait Broker: Send + Sync {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()>;
    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()>;
    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()>;
    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()>;
    fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()>;
    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()>;
    fn publish_job(&self, job: &Job) -> BoxedFuture<()>;
    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()>;
    fn publish_event(&self, topic: String, event: Value) -> BoxedFuture<()>;
    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()>;
    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()>;
    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()>;
    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>>;
    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo>;
    fn delete_queue(&self, qname: String) -> BoxedFuture<()>;
    fn health_check(&self) -> BoxedFuture<()>;
    fn shutdown(&self) -> BoxedFuture<()>;
}

pub mod inmemory;
pub mod rabbitmq;

#[cfg(test)]
mod tests {
    use super::queue;
    use super::{is_coordinator_queue, is_task_queue, is_worker_queue};

    #[test]
    fn is_worker_queue_returns_true_for_default_queue() {
        assert!(is_worker_queue("default"));
    }

    #[test]
    fn is_worker_queue_returns_true_for_custom_queue() {
        assert!(is_worker_queue("my-queue"));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_jobs() {
        assert!(!is_worker_queue(queue::QUEUE_JOBS));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_completed() {
        assert!(!is_worker_queue(queue::QUEUE_COMPLETED));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_failed() {
        assert!(!is_worker_queue(queue::QUEUE_FAILED));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_started() {
        assert!(!is_worker_queue(queue::QUEUE_STARTED));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_heartbeat() {
        assert!(!is_worker_queue(queue::QUEUE_HEARTBEAT));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_progress() {
        assert!(!is_worker_queue(queue::QUEUE_PROGRESS));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_task_log_part() {
        assert!(!is_worker_queue(queue::QUEUE_TASK_LOG_PART));
    }

    #[test]
    fn is_worker_queue_returns_false_for_queue_redeliveries() {
        assert!(!is_worker_queue(queue::QUEUE_REDELIVERIES));
    }

    #[test]
    fn is_worker_queue_returns_false_for_exclusive_prefix_queue() {
        assert!(!is_worker_queue(&format!(
            "{}test",
            queue::QUEUE_EXCLUSIVE_PREFIX
        )));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_jobs() {
        assert!(is_coordinator_queue(queue::QUEUE_JOBS));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_completed() {
        assert!(is_coordinator_queue(queue::QUEUE_COMPLETED));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_failed() {
        assert!(is_coordinator_queue(queue::QUEUE_FAILED));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_started() {
        assert!(is_coordinator_queue(queue::QUEUE_STARTED));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_heartbeat() {
        assert!(is_coordinator_queue(queue::QUEUE_HEARTBEAT));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_progress() {
        assert!(is_coordinator_queue(queue::QUEUE_PROGRESS));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_task_log_part() {
        assert!(is_coordinator_queue(queue::QUEUE_TASK_LOG_PART));
    }

    #[test]
    fn is_coordinator_queue_returns_true_for_queue_redeliveries() {
        assert!(is_coordinator_queue(queue::QUEUE_REDELIVERIES));
    }

    #[test]
    fn is_coordinator_queue_returns_false_for_default_queue() {
        assert!(!is_coordinator_queue("default"));
    }

    #[test]
    fn is_coordinator_queue_returns_false_for_exclusive_prefix_queue() {
        assert!(!is_coordinator_queue(&format!(
            "{}test",
            queue::QUEUE_EXCLUSIVE_PREFIX
        )));
    }

    #[test]
    fn is_task_queue_delegates_to_is_worker_queue() {
        assert!(is_task_queue("default"));
        assert!(!is_task_queue(queue::QUEUE_JOBS));
    }
}
