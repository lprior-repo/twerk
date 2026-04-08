//! Broker module for message queue and pub/sub functionality.
//!
//! This module provides broker implementations for delivering tasks
//! and coordinating between workers and the coordinator.

use anyhow::Result;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use twerk_core::job::{Job, JobEvent};
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
    let trimmed = engine_id.trim();
    if trimmed.is_empty() || queue.ends_with(&format!(".{trimmed}")) {
        queue.to_string()
    } else {
        format!("{queue}.{trimmed}")
    }
}

fn coordinator_queue_names() -> [&'static str; 8] {
    [
        queue::QUEUE_COMPLETED,
        queue::QUEUE_FAILED,
        queue::QUEUE_STARTED,
        queue::QUEUE_HEARTBEAT,
        queue::QUEUE_JOBS,
        queue::QUEUE_PROGRESS,
        queue::QUEUE_TASK_LOG_PART,
        queue::QUEUE_REDELIVERIES,
    ]
}

fn base_queue_name(qname: &str) -> &str {
    coordinator_queue_names()
        .into_iter()
        .find(|queue_name| {
            let dotted = format!("{queue_name}.");
            qname.starts_with(&dotted)
        })
        .unwrap_or(qname)
}

/// Extracts `engine_id` from a prefixed queue name.
/// Returns None if the queue is not prefixed (no dot separator at the end).
#[must_use]
pub fn extract_engine_id(queue_name: &str) -> Option<String> {
    coordinator_queue_names()
        .into_iter()
        .find_map(|coordinator_queue| {
            let prefix = format!("{coordinator_queue}.");
            queue_name.strip_prefix(&prefix).and_then(|suffix| {
                let trimmed = suffix.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
        })
}

#[must_use]
pub fn is_coordinator_queue(qname: &str) -> bool {
    matches!(
        base_queue_name(qname),
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

    /// Consolidated test: all coordinator queues are NOT worker queues.
    #[test]
    fn is_worker_queue_returns_false_for_all_coordinator_queues() {
        let coordinator_queues = [
            queue::QUEUE_COMPLETED,
            queue::QUEUE_FAILED,
            queue::QUEUE_STARTED,
            queue::QUEUE_HEARTBEAT,
            queue::QUEUE_JOBS,
            queue::QUEUE_PROGRESS,
            queue::QUEUE_TASK_LOG_PART,
            queue::QUEUE_REDELIVERIES,
        ];

        for queue in coordinator_queues {
            assert!(
                !is_worker_queue(queue),
                "{queue} should NOT be a worker queue"
            );
        }
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
        assert_eq!(extract_engine_id("x-jobs.a.b.c"), Some("a.b.c".to_string()));
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

    // ── Round-trip property tests ──────────────────────────────────────────────

    /// When `engine_id` is empty, round-trip is identity.
    #[test]
    fn prefixed_queue_extract_round_trip_is_identity_when_engine_id_empty() {
        let queue = "x-pending";
        let prefixed = prefixed_queue(queue, "");
        assert_eq!(prefixed, queue);

        let queue = "x-jobs";
        let prefixed = prefixed_queue(queue, "");
        assert_eq!(prefixed, queue);
    }

    /// When `engine_id` is non-empty, extract then re-prefix gives equivalent result.
    #[test]
    #[allow(clippy::panic)]
    fn prefixed_queue_extract_and_reprefix_round_trip() {
        let engine_id = "test-abc";
        let queue = "x-jobs";

        let prefixed = prefixed_queue(queue, engine_id);
        // Extracting and re-prefixing should give same result
        let Some(extracted) = extract_engine_id(&prefixed) else {
            panic!("[BUG] extract_engine_id returned None for prefixed queue - round-trip broken");
        };
        let round_tripped = prefixed_queue(queue, &extracted);
        assert_eq!(
            prefixed, round_tripped,
            "prefixed_queue(queue, extract_engine_id(prefixed_queue(queue, engine_id))) should equal prefixed_queue(queue, engine_id)"
        );
    }

    /// Round-trip for all coordinator queue types.
    #[test]
    #[allow(clippy::expect_used)]
    fn all_coordinator_queues_round_trip_correctly() {
        let engine_id = "engine-xyz";
        let coordinator_queues = [
            queue::QUEUE_COMPLETED,
            queue::QUEUE_FAILED,
            queue::QUEUE_STARTED,
            queue::QUEUE_HEARTBEAT,
            queue::QUEUE_JOBS,
            queue::QUEUE_PROGRESS,
            queue::QUEUE_TASK_LOG_PART,
            queue::QUEUE_REDELIVERIES,
        ];

        for queue in coordinator_queues {
            let prefixed = prefixed_queue(queue, engine_id);
            let extracted = extract_engine_id(&prefixed)
                .expect("Expected extract_engine_id to succeed for coordinator queue");
            let round_tripped = prefixed_queue(queue, &extracted);
            assert_eq!(
                prefixed, round_tripped,
                "Round-trip failed for queue '{queue}'"
            );
        }
    }

    // ── Already-prefixed queue handling ──────────────────────────────────────────

    /// Calling `prefixed_queue` on an already-prefixed queue appends the new suffix.
    /// Result is "x-jobs.test-abc.test-xyz" - the new `engine_id` becomes part of suffix.
    /// This is a KNOWN LIMITATION: callers must ensure they don't call this on
    /// already-prefixed queues. The function doesn't prevent double-suffixing.
    #[test]
    fn prefixed_queue_appends_suffix_when_queue_already_prefixed() {
        let queue = "x-jobs.test-abc";
        let result = prefixed_queue(queue, "test-xyz");
        // Current behavior: appends, creating "x-jobs.test-abc.test-xyz"
        assert_eq!(result, "x-jobs.test-abc.test-xyz");
    }

    /// Extract from an already-prefixed coordinator queue returns the full suffix.
    #[test]
    fn extract_engine_id_from_already_prefixed_queue() {
        // "x-jobs.test-abc.test-xyz" -> extract returns the full suffix
        let queue = "x-jobs.test-abc.test-xyz";
        let extracted = extract_engine_id(queue);
        assert_eq!(extracted, Some("test-abc.test-xyz".to_string()));
    }

    // ── Empty/whitespace engine_id edge cases ─────────────────────────────────

    /// Empty `engine_id` produces unprefixed queue (backward compatible).
    #[test]
    fn empty_engine_id_produces_unprefixed_queue() {
        assert_eq!(prefixed_queue("x-jobs", ""), "x-jobs");
        assert_eq!(prefixed_queue("x-pending", ""), "x-pending");
    }

    /// Whitespace-only `engine_id` is treated as empty.
    #[test]
    fn whitespace_engine_id_produces_prefixed_queue() {
        let result = prefixed_queue("x-jobs", "   ");
        assert_eq!(result, "x-jobs");
    }

    // ── is_task_queue behavior ────────────────────────────────────────────────

    /// `is_task_queue` is an alias for `is_worker_queue` - verifies basic delegation.
    #[test]
    fn is_task_queue_matches_is_worker_queue() {
        // Worker queues
        assert_eq!(is_task_queue("default"), is_worker_queue("default"));
        assert_eq!(is_task_queue("my-queue"), is_worker_queue("my-queue"));
        assert_eq!(is_task_queue("x-pending"), is_worker_queue("x-pending"));

        // Coordinator queues
        assert_eq!(
            is_task_queue(queue::QUEUE_JOBS),
            is_worker_queue(queue::QUEUE_JOBS)
        );
        assert_eq!(
            is_task_queue(queue::QUEUE_COMPLETED),
            is_worker_queue(queue::QUEUE_COMPLETED)
        );
    }
}
