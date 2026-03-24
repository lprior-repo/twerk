//! Error handler for task error events.
//!
//! 100% parity with Go `internal/coordinator/handlers/error.go`.
//!
//! Handles two scenarios when a task fails:
//! 1. **Retry**: If the task has remaining retry attempts and the job is
//!    active, prepares a new retry task and returns it for persistence/publishing.
//! 2. **Fail job**: If no retries remain, marks the job as failed and
//!    cascades to the job handler (which cancels active tasks, publishes events).
//!
//! # Architecture
//!
//! - **Calc** (`is_retry_eligible`, `prepare_retry_task`): Pure functions that
//!   compute retry decisions and prepare retry tasks without I/O.
//! - **Actions** (`ErrorHandler::handle_with_job`): Applies state transitions
//!   and delegates to middleware handlers at the shell boundary.
//!
//! # Note
//!
//! The Go handler calls `eval.EvaluateTask` on the retry task before persisting.
//! In this functional Rust port, the caller is responsible for evaluation since
//! the `eval` module lives in a separate crate to avoid circular dependencies.

use time::OffsetDateTime;
use tork::job::{Job, JOB_STATE_FAILED, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED};
use tork::task::{Task, TaskRetry, TASK_STATE_FAILED, TASK_STATE_PENDING};

use crate::handlers::{
    noop_job_handler, noop_task_handler, HandlerContext, HandlerError, JobEventType,
    JobHandlerFunc, TaskEventType, TaskHandlerFunc,
};

// ---------------------------------------------------------------------------
// Pure calculation functions (Data → Calc)
// ---------------------------------------------------------------------------

/// Checks if a failed task is eligible for retry.
///
/// A task is eligible for retry when all three conditions hold:
/// 1. The job is in [`JOB_STATE_RUNNING`] or [`JOB_STATE_SCHEDULED`] state
/// 2. The task has a retry configuration
/// 3. The number of retry attempts is strictly less than the retry limit
///
/// Parity with Go:
/// ```go
/// (j.State == tork.JobStateRunning || j.State == tork.JobStateScheduled) &&
///     t.Retry != nil &&
///     t.Retry.Attempts < t.Retry.Limit
/// ```
#[must_use]
pub fn is_retry_eligible(job: &Job, task: &Task) -> bool {
    let job_active = job.state == JOB_STATE_RUNNING || job.state == JOB_STATE_SCHEDULED;
    if !job_active {
        return false;
    }
    task.retry.as_ref().is_some_and(|r| r.attempts < r.limit)
}

/// Prepares a retry task from the failed task.
///
/// Clones the task, assigns a new ID, sets `created_at` to now, increments
/// the retry counter, resets state to PENDING, and clears error/failed_at.
///
/// Returns the retry task for the caller to evaluate and persist.
///
/// Parity with Go:
/// ```go
/// rt := t.Clone()
/// rt.ID = uuid.NewUUID()
/// rt.CreatedAt = &now
/// rt.Retry.Attempts = rt.Retry.Attempts + 1
/// rt.State = tork.TaskStatePending
/// rt.Error = ""
/// rt.FailedAt = nil
/// ```
///
/// # Errors
///
/// Returns [`HandlerError::Validation`] if the task has no retry configuration.
pub fn prepare_retry_task(task: &Task) -> Result<Task, HandlerError> {
    let retry = task.retry.as_ref().ok_or_else(|| {
        HandlerError::Validation("cannot prepare retry task: no retry config".to_string())
    })?;

    let base = task.deep_clone();

    Ok(Task {
        id: Some(new_task_id()),
        created_at: Some(OffsetDateTime::now_utc()),
        retry: Some(TaskRetry {
            limit: retry.limit,
            attempts: retry.attempts + 1,
        }),
        state: TASK_STATE_PENDING.clone(),
        error: None,
        failed_at: None,
        ..base
    })
}

/// Generates a new task ID (32-char hex, no hyphens).
/// Matches Go's `uuid.NewUUID()`.
#[must_use]
fn new_task_id() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

// ---------------------------------------------------------------------------
// ErrorOutcome — result of processing a task error event
// ---------------------------------------------------------------------------

/// The result of processing a task error event.
///
/// Tells the caller what action to take next:
/// - [`ErrorOutcome::FailJob`]: Job has been marked as failed; the caller
///   should persist the job state (cancel active tasks is handled by job handler).
/// - [`ErrorOutcome::Retry`]: A retry task was prepared; the caller should
///   evaluate it with `eval::evaluate_task`, persist, and publish to the pending queue.
#[derive(Debug)]
pub enum ErrorOutcome {
    /// No more retries available; the job has been marked as failed.
    FailJob,

    /// A retry task was prepared and should be evaluated, persisted, and published.
    Retry {
        /// The prepared retry task (caller should apply eval before persisting).
        task: Task,
    },
}

// ---------------------------------------------------------------------------
// ErrorHandler
// ---------------------------------------------------------------------------

/// Error handler for processing task error events.
///
/// Go parity with `errorHandler` in `error.go`.
///
/// The Go constructor `NewErrorHandler(ds, b, mw...)` wires together a
/// datastore, broker, and job middleware. In this functional port, the handler
/// holds callback functions that the wiring layer provides, keeping the
/// handler itself free of I/O dependencies.
#[derive(Clone)]
pub struct ErrorHandler {
    /// Task handler middleware chain (e.g., datastore persistence, logging).
    handler: TaskHandlerFunc,
    /// Job handler for cascading job state changes on failure
    /// (cancels active tasks, publishes events).
    on_job: JobHandlerFunc,
}

impl std::fmt::Debug for ErrorHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorHandler").finish()
    }
}

impl ErrorHandler {
    /// Create a new error handler with no-op handlers.
    pub fn new() -> Self {
        Self {
            handler: noop_task_handler(),
            on_job: noop_job_handler(),
        }
    }

    /// Create an error handler with custom task and job handlers.
    ///
    /// This mirrors Go's `NewErrorHandler(ds, b, mw...)` where the task
    /// handler and job handler middleware are provided by the wiring layer.
    pub fn with_handlers(handler: TaskHandlerFunc, on_job: JobHandlerFunc) -> Self {
        Self { handler, on_job }
    }

    /// Handle a task error event (simplified — no job context).
    ///
    /// Marks the task as failed and delegates to the task handler middleware.
    /// Use [`handle_with_job`](Self::handle_with_job) for full Go parity with
    /// retry logic and job failure cascading.
    pub fn handle(&self, ctx: HandlerContext, task: &mut Task) -> Result<(), HandlerError> {
        task.state = TASK_STATE_FAILED.clone();
        (self.handler)(ctx, TaskEventType::StateChange, task)
    }

    /// Handle a task error event with full Go parity logic.
    ///
    /// 1. Marks the task as FAILED and sets `failed_at`.
    /// 2. Checks retry eligibility based on job state and retry config.
    /// 3. If retry eligible: prepares a retry task, calls task handler,
    ///    returns [`ErrorOutcome::Retry`] for the caller to eval/persist/publish.
    /// 4. If not retry eligible: marks the job as FAILED, cascades to the
    ///    job handler (which cancels active tasks, publishes events),
    ///    returns [`ErrorOutcome::FailJob`].
    ///
    /// Parity with Go `errorHandler.handle()`.
    pub fn handle_with_job(
        &self,
        ctx: HandlerContext,
        task: &mut Task,
        job: &mut Job,
    ) -> Result<ErrorOutcome, HandlerError> {
        // Step 1: Mark task as failed with timestamp
        task.state = TASK_STATE_FAILED.clone();
        task.failed_at = Some(OffsetDateTime::now_utc());

        // Step 2–3: Check retry eligibility and prepare retry task
        if is_retry_eligible(job, task) {
            let retry_task = prepare_retry_task(task)?;
            (self.handler)(ctx, TaskEventType::StateChange, task)?;
            return Ok(ErrorOutcome::Retry { task: retry_task });
        }

        // Step 4: No more retries — fail the job
        job.state = JOB_STATE_FAILED.to_string();
        job.failed_at = task.failed_at;
        job.error = task.error.clone();

        // Cascade to job handler (Go: h.onJob(ctx, job.StateChange, j))
        // This cancels active tasks and publishes events.
        (self.on_job)(ctx.clone(), JobEventType::StateChange, job)?;
        (self.handler)(ctx, TaskEventType::StateChange, task)?;

        Ok(ErrorOutcome::FailJob)
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
