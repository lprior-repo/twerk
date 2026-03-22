//! Redelivered task handler.
//!
//! Port of Go `internal/coordinator/handlers/redelivered.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives redelivered tasks (tasks that failed to process)
//! 2. If `task.redelivered >= MAX_REDELIVERIES (5)`:
//!    - Sets error message to "task redelivered too many times"
//!    - Sets `failed_at` timestamp
//!    - Sets state to FAILED
//!    - Publishes to `QUEUE_ERROR` via broker
//! 3. Otherwise:
//!    - Increments `redelivered` counter
//!    - Re-queues to the task's original queue via broker
//!
//! # Architecture
//!
//! - **Calc** (`should_fail_task`, `prepare_failed_task`, `prepare_requeued_task`,
//!   `process_redelivered`): Pure functions that compute the decision and
//!   prepare output tasks without I/O.
//! - **Actions** (`RedeliveredHandler::handle`): Publishes the prepared task
//!   to the appropriate queue via the broker at the shell boundary.

use std::sync::Arc;

use time::OffsetDateTime;
use tork::broker::queue;
use tork::task::{Task, TASK_STATE_FAILED};
use tork::Broker;

use crate::handlers::{HandlerError, MAX_REDELIVERIES};

// ---------------------------------------------------------------------------
// Pure Calculations (Data → Calc)
// ---------------------------------------------------------------------------

/// Checks whether a task has exceeded the maximum allowed redeliveries.
///
/// Go parity: `t.Redelivered >= maxRedeliveries` (maxRedeliveries = 5)
#[must_use]
pub const fn should_fail_task(redelivered: i64) -> bool {
    redelivered >= MAX_REDELIVERIES
}

/// Prepares a failed task for the error queue.
///
/// Creates a new task with:
/// - `error` set to "task redelivered too many times"
/// - `failed_at` set to now
/// - `state` set to FAILED
///
/// Go parity:
/// ```go
/// now := time.Now().UTC()
/// t.Error = "task redelivered too many times"
/// t.FailedAt = &now
/// t.State = tork.TaskStateFailed
/// ```
#[must_use]
pub fn prepare_failed_task(task: &Task) -> Task {
    Task {
        error: Some("task redelivered too many times".to_string()),
        failed_at: Some(OffsetDateTime::now_utc()),
        state: TASK_STATE_FAILED.clone(),
        ..task.clone()
    }
}

/// Prepares a requeued task with incremented redelivered counter.
///
/// Go parity: `t.Redelivered++`
#[must_use]
pub fn prepare_requeued_task(task: &Task) -> Task {
    Task {
        redelivered: task.redelivered + 1,
        ..task.clone()
    }
}

/// The outcome of processing a redelivered task.
///
/// Separates pure decision logic from I/O (broker publishing), enabling
/// thorough testing without mocks.
#[derive(Debug, PartialEq)]
pub enum RedeliveredOutcome {
    /// Task was requeued to its original queue with incremented redelivered count.
    Requeued { task: Task },
    /// Task exceeded max redeliveries and should go to the error queue.
    Failed { task: Task },
}

/// Processes a redelivered task and returns the outcome.
///
/// Go parity with `redeliveredHandler.handle()` decision logic.
#[must_use]
pub fn process_redelivered(task: &Task) -> RedeliveredOutcome {
    if should_fail_task(task.redelivered) {
        RedeliveredOutcome::Failed {
            task: prepare_failed_task(task),
        }
    } else {
        RedeliveredOutcome::Requeued {
            task: prepare_requeued_task(task),
        }
    }
}

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Redelivered task handler for handling task redelivery.
///
/// Go parity with `redeliveredHandler` in `redelivered.go`.
pub struct RedeliveredHandler {
    broker: Arc<dyn Broker>,
}

impl std::fmt::Debug for RedeliveredHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedeliveredHandler").finish()
    }
}

impl RedeliveredHandler {
    /// Create a new redelivered handler with broker dependency.
    ///
    /// Go parity: `NewRedeliveredHandler(ds datastore.Datastore, b broker.Broker)`
    ///
    /// Note: The Go handler accepts a datastore but never uses it in `handle`.
    /// The Rust version only requires the broker, matching actual usage.
    pub fn new(broker: Arc<dyn Broker>) -> Self {
        Self { broker }
    }

    /// Handle a redelivered task.
    ///
    /// Go parity (`handle`):
    /// 1. If `task.redelivered >= MAX_REDELIVERIES`:
    ///    - Mark as FAILED, publish to `QUEUE_ERROR`
    /// 2. Otherwise:
    ///    - Increment redelivered, publish to original queue
    ///
    /// # Errors
    ///
    /// Returns [`HandlerError::Validation`] if requeueing and task has no queue.
    /// Returns [`HandlerError::Broker`] if broker publish fails.
    pub async fn handle(&self, task: &Task) -> Result<(), HandlerError> {
        match process_redelivered(task) {
            RedeliveredOutcome::Failed { task: failed_task } => {
                // Go: `h.broker.PublishTask(ctx, broker.QUEUE_ERROR, t)`
                self.broker
                    .publish_task(queue::QUEUE_ERROR.to_string(), &failed_task)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
            RedeliveredOutcome::Requeued { task: requeued_task } => {
                // Go: `h.broker.PublishTask(ctx, t.Queue, t)`
                let qname = requeued_task
                    .queue
                    .clone()
                    .ok_or_else(|| {
                        HandlerError::Validation("task queue is required for requeue".into())
                    })?;
                self.broker
                    .publish_task(qname, &requeued_task)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tork::task::TASK_STATE_RUNNING;

    /// Helper: creates a minimal running task with given redelivered count.
    fn make_task(redelivered: i64) -> Task {
        Task {
            id: Some("task-1".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            queue: Some("test-queue".to_string()),
            redelivered,
            ..Task::default()
        }
    }

    // -- should_fail_task (pure calc) ----------------------------------------

    #[test]
    fn test_should_fail_zero() {
        assert!(!should_fail_task(0));
    }

    #[test]
    fn test_should_fail_four() {
        assert!(!should_fail_task(4));
    }

    #[test]
    fn test_should_fail_five() {
        assert!(should_fail_task(5));
    }

    #[test]
    fn test_should_fail_six() {
        assert!(should_fail_task(6));
    }

    // -- prepare_failed_task (pure calc) ------------------------------------

    #[test]
    fn test_prepare_failed_task_error_message() {
        let task = make_task(5);
        let failed = prepare_failed_task(&task);
        assert_eq!(
            failed.error.as_deref(),
            Some("task redelivered too many times")
        );
    }

    #[test]
    fn test_prepare_failed_task_state() {
        let task = make_task(5);
        let failed = prepare_failed_task(&task);
        assert_eq!(failed.state, *TASK_STATE_FAILED);
    }

    #[test]
    fn test_prepare_failed_task_has_failed_at() {
        let task = make_task(5);
        let failed = prepare_failed_task(&task);
        assert!(failed.failed_at.is_some());
    }

    #[test]
    fn test_prepare_failed_task_preserves_id() {
        let task = make_task(5);
        let failed = prepare_failed_task(&task);
        assert_eq!(failed.id.as_deref(), Some("task-1"));
    }

    #[test]
    fn test_prepare_failed_task_preserves_queue() {
        let task = make_task(5);
        let failed = prepare_failed_task(&task);
        assert_eq!(failed.queue.as_deref(), Some("test-queue"));
    }

    #[test]
    fn test_prepare_failed_task_preserves_redelivered() {
        let task = make_task(5);
        let failed = prepare_failed_task(&task);
        assert_eq!(failed.redelivered, 5);
    }

    // -- prepare_requeued_task (pure calc) -----------------------------------

    #[test]
    fn test_prepare_requeued_task_increments() {
        let task = make_task(3);
        let requeued = prepare_requeued_task(&task);
        assert_eq!(requeued.redelivered, 4);
    }

    #[test]
    fn test_prepare_requeued_task_from_zero() {
        let task = make_task(0);
        let requeued = prepare_requeued_task(&task);
        assert_eq!(requeued.redelivered, 1);
    }

    #[test]
    fn test_prepare_requeued_task_preserves_state() {
        let task = make_task(0);
        let requeued = prepare_requeued_task(&task);
        assert_eq!(requeued.state, *TASK_STATE_RUNNING);
    }

    #[test]
    fn test_prepare_requeued_task_preserves_id() {
        let task = make_task(0);
        let requeued = prepare_requeued_task(&task);
        assert_eq!(requeued.id.as_deref(), Some("task-1"));
    }

    // -- process_redelivered (Go test parity) --------------------------------

    /// Go test: 5 iterations with redelivered 0..4 → all requeued, state RUNNING.
    /// 6th call with redelivered=5 → FAILED.
    #[test]
    fn test_process_redelivered_go_parity_full_cycle() {
        // Simulate the Go test: 5 successful requeues
        let mut redelivered_count = 0i64;
        for i in 0..5 {
            let task = make_task(redelivered_count);
            let outcome = process_redelivered(&task);
            match outcome {
                RedeliveredOutcome::Requeued { task: requeued } => {
                    assert_eq!(requeued.state, *TASK_STATE_RUNNING);
                    assert_eq!(requeued.redelivered, i + 1);
                    redelivered_count = requeued.redelivered;
                }
                RedeliveredOutcome::Failed { .. } => {
                    panic!("expected Requeued at iteration {i}, got Failed");
                }
            }
        }

        // After 5 requeues, redelivered = 5, should fail
        assert_eq!(redelivered_count, 5);
        let task = make_task(redelivered_count);
        let outcome = process_redelivered(&task);
        match outcome {
            RedeliveredOutcome::Failed { task: failed } => {
                assert_eq!(failed.state, *TASK_STATE_FAILED);
                assert_eq!(failed.redelivered, 5);
                assert!(failed.failed_at.is_some());
                assert_eq!(
                    failed.error.as_deref(),
                    Some("task redelivered too many times")
                );
            }
            RedeliveredOutcome::Requeued { .. } => {
                panic!("expected Failed on 6th call, got Requeued");
            }
        }
    }

    #[test]
    fn test_process_redelivered_at_limit() {
        let task = make_task(MAX_REDELIVERIES);
        let outcome = process_redelivered(&task);
        assert!(matches!(outcome, RedeliveredOutcome::Failed { .. }));
    }

    #[test]
    fn test_process_redelivered_under_limit() {
        let task = make_task(MAX_REDELIVERIES - 1);
        let outcome = process_redelivered(&task);
        assert!(matches!(outcome, RedeliveredOutcome::Requeued { .. }));
    }

    #[test]
    fn test_process_redelivered_far_under_limit() {
        let task = make_task(0);
        let outcome = process_redelivered(&task);
        if let RedeliveredOutcome::Requeued { task: requeued } = outcome {
            assert_eq!(requeued.redelivered, 1);
        } else {
            panic!("expected Requeued");
        }
    }

    // -- RedeliveredHandler construction -------------------------------------

    #[test]
    fn test_redelivered_handler_new() {
        let broker = Arc::new(MockBroker);
        let handler = RedeliveredHandler::new(broker);
        assert_eq!(format!("{handler:?}"), "RedeliveredHandler");
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: RedeliveredOutcome equality
    #[test]
    fn test_redelivered_outcome_equality() {
        let task = make_task(5);
        let fail1 = RedeliveredOutcome::Failed { task: task.clone() };
        let fail2 = RedeliveredOutcome::Failed { task: task.clone() };
        assert_eq!(fail1, fail2);

        let requeue1 = RedeliveredOutcome::Requeued { task: task.clone() };
        let requeue2 = RedeliveredOutcome::Requeued { task };
        assert_eq!(requeue1, requeue2);

        assert_ne!(fail1, requeue1);
    }

    // Go: should_fail_task at boundary values
    #[test]
    fn test_should_fail_negative() {
        assert!(!should_fail_task(-1));
    }

    #[test]
    fn test_should_fail_max_int() {
        assert!(should_fail_task(i64::MAX));
    }

    // Go: prepare_failed_task preserves all non-error fields
    #[test]
    fn test_prepare_failed_task_preserves_all_fields() {
        let task = Task {
            id: Some("t1".into()),
            job_id: Some("j1".into()),
            name: Some("build".into()),
            image: Some("alpine".into()),
            position: 3,
            queue: Some("q1".into()),
            state: TASK_STATE_RUNNING.clone(),
            redelivered: 5,
            ..Task::default()
        };
        let failed = prepare_failed_task(&task);
        assert_eq!(failed.id.as_deref(), Some("t1"));
        assert_eq!(failed.job_id.as_deref(), Some("j1"));
        assert_eq!(failed.name.as_deref(), Some("build"));
        assert_eq!(failed.image.as_deref(), Some("alpine"));
        assert_eq!(failed.position, 3);
        assert_eq!(failed.queue.as_deref(), Some("q1"));
        assert_eq!(failed.redelivered, 5);
        assert_eq!(failed.state, *TASK_STATE_FAILED);
        assert!(failed.error.is_some());
        assert!(failed.failed_at.is_some());
    }

    // Go: prepare_requeued_task preserves all fields
    #[test]
    fn test_prepare_requeued_task_preserves_all_fields() {
        let task = Task {
            id: Some("t1".into()),
            job_id: Some("j1".into()),
            name: Some("deploy".into()),
            position: 7,
            queue: Some("deploy-queue".into()),
            state: TASK_STATE_RUNNING.clone(),
            redelivered: 2,
            error: Some("transient".into()),
            ..Task::default()
        };
        let requeued = prepare_requeued_task(&task);
        assert_eq!(requeued.id.as_deref(), Some("t1"));
        assert_eq!(requeued.job_id.as_deref(), Some("j1"));
        assert_eq!(requeued.name.as_deref(), Some("deploy"));
        assert_eq!(requeued.position, 7);
        assert_eq!(requeued.queue.as_deref(), Some("deploy-queue"));
        assert_eq!(requeued.state, *TASK_STATE_RUNNING);
        assert_eq!(requeued.redelivered, 3);
        assert_eq!(requeued.error.as_deref(), Some("transient"));
    }

    // Go: MAX_REDELIVERIES constant verification
    #[test]
    fn test_max_redeliveries_value() {
        assert_eq!(MAX_REDELIVERIES, 5);
    }

    // -- Integration tests (require real broker) -----------------------------

    /// Go parity: Test_handleRedeliveredTask
    #[tokio::test]
    #[ignore]
    async fn test_handle_redelivered_task_integration() {
        todo!("requires broker integration");
    }

    // -- Mock broker for construction tests ----------------------------------

    struct MockBroker;

    impl Broker for MockBroker {
        fn publish_task(&self, _qname: String, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(
            &self,
            _qname: String,
            _handler: tork::broker::TaskHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(&self, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(
            &self,
            _handler: tork::broker::TaskProgressHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(&self, _node: tork::node::Node) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_heartbeats(
            &self,
            _handler: tork::broker::HeartbeatHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(&self, _job: &tork::job::Job) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(
            &self,
            _handler: tork::broker::JobHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(
            &self,
            _topic: String,
            _event: serde_json::Value,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(
            &self,
            _pattern: String,
            _handler: tork::broker::EventHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_log_part(
            &self,
            _part: &tork::task::TaskLogPart,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(
            &self,
            _handler: tork::broker::TaskLogPartHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn queues(&self) -> tork::broker::BoxedFuture<Vec<tork::broker::QueueInfo>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn queue_info(&self, _qname: String) -> tork::broker::BoxedFuture<tork::broker::QueueInfo> {
            Box::pin(async {
                Ok(tork::broker::QueueInfo {
                    name: String::new(),
                    size: 0,
                    subscribers: 0,
                    unacked: 0,
                })
            })
        }
        fn delete_queue(&self, _qname: String) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn health_check(&self) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }
}
