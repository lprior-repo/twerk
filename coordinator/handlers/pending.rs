//! Pending handler for pending task events.
//!
//! Handles tasks in PENDING state by checking conditional expressions
//! and delegating to the scheduler for task routing.
//!
//! Go parity: `pending.go` receives a PENDING task, evaluates its `if`
//! expression, and either skips it (state→SKIPPED, timestamps→now) or
//! delegates to `scheduler.ScheduleTask()`.

use std::sync::Arc;

use tork::broker::queue::QUEUE_COMPLETED;
use tork::Broker;
use tork::Datastore;

use crate::handlers::{
    noop_task_handler, HandlerContext, HandlerError, TaskEventType, TaskHandlerFunc,
};
use crate::scheduler::Scheduler;
use time::OffsetDateTime;
use tork::task::{Task, TASK_STATE_SKIPPED};

// ---------------------------------------------------------------------------
// Pure calculations
// ---------------------------------------------------------------------------

/// Evaluates whether a task's `r#if` condition means it should be skipped.
///
/// Returns `true` when the condition string, after trimming whitespace,
/// is exactly `"false"`.  Absent or any other value means "do not skip".
#[must_use]
fn is_skip_condition(task: &Task) -> bool {
    task.r#if
        .as_deref()
        .map(str::trim)
        .is_some_and(|cond| cond == "false")
}

/// Computes the result of the pending-task evaluation.
///
/// This is a pure decision function — no mutation, no I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingDecision {
    /// The task's `if` condition is explicitly "false" — skip it.
    Skip,
    /// No skip condition (or condition is not "false") — schedule it.
    Schedule,
}

fn evaluate_pending(task: &Task) -> PendingDecision {
    if is_skip_condition(task) {
        PendingDecision::Skip
    } else {
        PendingDecision::Schedule
    }
}

// ---------------------------------------------------------------------------
// State transitions (applied at the action boundary)
// ---------------------------------------------------------------------------

/// Applies the skip transition: sets state to SKIPPED and all lifecycle
/// timestamps (scheduled_at, started_at, completed_at) to `now`.
fn apply_skip_transition(task: &mut Task, now: OffsetDateTime) {
    task.state = TASK_STATE_SKIPPED.clone();
    task.scheduled_at = Some(now);
    task.started_at = Some(now);
    task.completed_at = Some(now);
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Pending handler for processing pending task events.
///
/// Go parity with `pendingHandler` in `pending.go`.
#[derive(Clone)]
pub struct PendingHandler {
    scheduler: Scheduler,
    broker: Arc<dyn Broker>,
    handler: TaskHandlerFunc,
}

impl std::fmt::Debug for PendingHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingHandler")
            .field("scheduler", &self.scheduler)
            .finish()
    }
}

impl PendingHandler {
    /// Create a new pending handler with datastore, broker, and noop callback.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self {
            scheduler: Scheduler::new(ds.clone(), broker.clone()),
            broker,
            handler: noop_task_handler(),
        }
    }

    /// Create a pending handler with datastore, broker, and custom handler function.
    pub fn with_handler(
        ds: Arc<dyn Datastore>,
        broker: Arc<dyn Broker>,
        handler: TaskHandlerFunc,
    ) -> Self {
        Self {
            scheduler: Scheduler::new(ds, broker.clone()),
            broker,
            handler,
        }
    }

    /// Create a pending handler with a custom scheduler.
    pub fn with_scheduler(scheduler: Scheduler, broker: Arc<dyn Broker>) -> Self {
        Self {
            scheduler,
            broker,
            handler: noop_task_handler(),
        }
    }

    /// Create a pending handler with custom scheduler and handler function.
    pub fn with_scheduler_and_handler(
        scheduler: Scheduler,
        broker: Arc<dyn Broker>,
        handler: TaskHandlerFunc,
    ) -> Self {
        Self {
            scheduler,
            broker,
            handler,
        }
    }

    /// Handle a pending task event.
    ///
    /// Mirrors Go's `handle()`:
    /// 1. Evaluates the `r#if` condition — if `"false"`, skips the task.
    /// 2. Otherwise delegates to [`Scheduler::schedule_task`] which routes
    ///    by task type (regular / parallel / each / sub-job) and applies
    ///    the correct state transition.
    pub async fn handle(&self, ctx: HandlerContext, task: &mut Task) -> Result<(), HandlerError> {
        let decision = evaluate_pending(task);
        match decision {
            PendingDecision::Skip => {
                apply_skip_transition(task, OffsetDateTime::now_utc());
                // Go parity: publish skipped task to QUEUE_COMPLETED
                self.broker
                    .publish_task(QUEUE_COMPLETED.to_string(), task)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
            PendingDecision::Schedule => {
                self.scheduler
                    .schedule_task(task)
                    .await
                    .map_err(|e| HandlerError::Handler(e.to_string()))?;
            }
        }
        (self.handler)(ctx, TaskEventType::StateChange, task)
    }
}

// Note: PendingHandler does not implement Default because it requires
// ds and broker to be useful. Use PendingHandler::new(ds, broker) instead.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
