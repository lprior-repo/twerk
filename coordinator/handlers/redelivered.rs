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

