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

/// Prefixes a queue name with `engine_id` if non-empty.
/// Returns queue unchanged when `engine_id` is empty (backward compatible).
#[must_use]
pub fn prefixed_queue(queue: &str, engine_id: &str) -> String {
    if engine_id.is_empty() {
        queue.to_string()
    } else {
        format!("{queue}.{engine_id}")
    }
}

/// Extracts `engine_id` from a prefixed queue name.
/// Returns None if the queue is not prefixed (no dot separator at the end).
#[must_use]
pub fn extract_engine_id(queue_name: &str) -> Option<String> {
    if let Some(idx) = queue_name.rfind('.') {
        let suffix = &queue_name[idx + 1..];
        // If suffix looks like an engine_id (not another queue segment), return it
        if !suffix.is_empty() && !suffix.starts_with("x-") {
            return Some(suffix.to_string());
        }
    }
    None
}

#[must_use]
pub fn is_coordinator_queue(qname: &str) -> bool {
    // Check exact matches first
    if matches!(
        qname,
        queue::QUEUE_COMPLETED
            | queue::QUEUE_FAILED
            | queue::QUEUE_STARTED
            | queue::QUEUE_HEARTBEAT
            | queue::QUEUE_JOBS
            | queue::QUEUE_PROGRESS
            | queue::QUEUE_TASK_LOG_PART
            | queue::QUEUE_REDELIVERIES
    ) {
        return true;
    }
    // Check if it's a prefixed coordinator queue (e.g., "x-jobs.engine-abc")
    if let Some(engine_id) = extract_engine_id(qname) {
        // Strip the ".{engine_id}" suffix to get the base queue name
        let base_queue_len = qname.len() - engine_id.len() - 1;
        let base_queue = &qname[..base_queue_len];
        matches!(
            base_queue,
            queue::QUEUE_COMPLETED
                | queue::QUEUE_FAILED
                | queue::QUEUE_STARTED
                | queue::QUEUE_HEARTBEAT
                | queue::QUEUE_JOBS
                | queue::QUEUE_PROGRESS
                | queue::QUEUE_TASK_LOG_PART
                | queue::QUEUE_REDELIVERIES
        )
    } else {
        false
    }
}

#[must_use]
pub fn is_worker_queue(qname: &str) -> bool {
    !is_coordinator_queue(qname) && !qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX)
}

#[must_use]
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
    use super::{
        extract_engine_id, is_coordinator_queue, is_task_queue, is_worker_queue, prefixed_queue,
    };

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

    // ── Queue prefix behavior ─────────────────────────────────────────────────

    #[test]
    fn prefixed_queue_returns_original_when_engine_id_empty() {
        assert_eq!(prefixed_queue("x-pending", ""), "x-pending");
        assert_eq!(prefixed_queue("x-jobs", ""), "x-jobs");
        assert_eq!(prefixed_queue("x-completed", ""), "x-completed");
        assert_eq!(prefixed_queue("x-failed", ""), "x-failed");
        assert_eq!(prefixed_queue("x-started", ""), "x-started");
        assert_eq!(prefixed_queue("x-heartbeat", ""), "x-heartbeat");
        assert_eq!(prefixed_queue("x-progress", ""), "x-progress");
        assert_eq!(prefixed_queue("x-task_log_part", ""), "x-task_log_part");
        assert_eq!(prefixed_queue("x-redeliveries", ""), "x-redeliveries");
    }

    #[test]
    fn prefixed_queue_adds_prefix_when_engine_id_non_empty() {
        assert_eq!(
            prefixed_queue("x-pending", "test-abc"),
            "x-pending.test-abc"
        );
        assert_eq!(prefixed_queue("x-jobs", "engine-xyz"), "x-jobs.engine-xyz");
        assert_eq!(
            prefixed_queue("x-completed", "worker-1"),
            "x-completed.worker-1"
        );
        assert_eq!(
            prefixed_queue("x-failed", "engine-abc"),
            "x-failed.engine-abc"
        );
    }

    #[test]
    fn prefixed_queue_handles_engine_id_with_dots() {
        // Engine ID itself can contain dots
        assert_eq!(
            prefixed_queue("x-pending", "engine.v1"),
            "x-pending.engine.v1"
        );
        assert_eq!(prefixed_queue("x-jobs", "a.b.c"), "x-jobs.a.b.c");
    }

    #[test]
    fn extract_engine_id_returns_none_for_unprefixed_queue() {
        assert_eq!(extract_engine_id("x-jobs"), None);
        assert_eq!(extract_engine_id("x-pending"), None);
        assert_eq!(extract_engine_id("x-completed"), None);
        assert_eq!(extract_engine_id("x-failed"), None);
        assert_eq!(extract_engine_id("default"), None);
        assert_eq!(extract_engine_id("my-queue"), None);
    }

    #[test]
    fn extract_engine_id_extracts_from_prefixed_queue() {
        assert_eq!(
            extract_engine_id("x-jobs.test-abc"),
            Some("test-abc".to_string())
        );
        assert_eq!(
            extract_engine_id("x-pending.engine-xyz"),
            Some("engine-xyz".to_string())
        );
        assert_eq!(
            extract_engine_id("x-completed.worker-1"),
            Some("worker-1".to_string())
        );
        assert_eq!(
            extract_engine_id("x-failed.engine-abc"),
            Some("engine-abc".to_string())
        );
    }

    #[test]
    fn extract_engine_id_handles_engine_id_with_dots() {
        // Note: The implementation uses rfind('.') which extracts the LAST dot-separated segment.
        // This means "x-pending.engine.v1" extracts only "v1", not "engine.v1".
        // This is a limitation of the current design when engine_ids contain dots.
        assert_eq!(
            extract_engine_id("x-pending.engine.v1"),
            Some("v1".to_string())
        );
        assert_eq!(extract_engine_id("x-jobs.a.b.c"), Some("c".to_string()));
    }

    #[test]
    fn is_coordinator_queue_recognizes_prefixed_coordinator_queues() {
        assert!(is_coordinator_queue("x-jobs.engine-abc"));
        assert!(is_coordinator_queue("x-completed.engine-abc"));
        assert!(is_coordinator_queue("x-failed.engine-abc"));
        assert!(is_coordinator_queue("x-started.engine-abc"));
        assert!(is_coordinator_queue("x-heartbeat.engine-abc"));
        assert!(is_coordinator_queue("x-progress.engine-abc"));
        assert!(is_coordinator_queue("x-task_log_part.engine-abc"));
        assert!(is_coordinator_queue("x-redeliveries.engine-abc"));
    }

    #[test]
    fn is_worker_queue_rejects_prefixed_coordinator_queues() {
        assert!(!is_worker_queue("x-jobs.engine-abc"));
        assert!(!is_worker_queue("x-completed.engine-abc"));
        assert!(!is_worker_queue("x-failed.engine-abc"));
        assert!(!is_worker_queue("x-started.engine-abc"));
        assert!(!is_worker_queue("x-heartbeat.engine-abc"));
        assert!(!is_worker_queue("x-progress.engine-abc"));
        assert!(!is_worker_queue("x-task_log_part.engine-abc"));
        assert!(!is_worker_queue("x-redeliveries.engine-abc"));
    }

    #[test]
    fn is_worker_queue_accepts_worker_queues() {
        assert!(is_worker_queue("default"));
        assert!(is_worker_queue("my-queue"));
        assert!(is_worker_queue("x-pending"));
        assert!(is_worker_queue("x-pending.engine-xyz")); // x-pending is a worker queue even with prefix
    }
}
