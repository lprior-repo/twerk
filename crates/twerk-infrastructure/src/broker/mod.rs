//! Broker module for message queue and pub/sub functionality.
//!
//! This module provides broker implementations for delivering tasks
//! and coordinating between workers and the coordinator.

#[allow(clippy::module_inception)]
pub mod broker;
pub mod config;
pub mod inmemory;
pub mod queue;
pub mod rabbitmq;
pub mod types;
pub mod utils;

// Re-exports to preserve public API paths
pub use broker::Broker;
pub use config::RabbitMQOptions;
pub use queue::QueueInfo;
pub(crate) use types::BoxedHandlerFuture;
pub use types::{
    BoxedFuture, EventHandler, HeartbeatHandler, JobHandler, TaskHandler, TaskLogPartHandler,
    TaskProgressHandler,
};
pub use utils::{
    extract_engine_id, is_coordinator_queue, is_task_queue, is_worker_queue, prefixed_queue,
};

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
