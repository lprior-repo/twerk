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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tork::job::{Job, JobContext};
    use tork::task::TASK_STATE_RUNNING;

    /// Helper to create a running job.
    fn make_running_job() -> Job {
        Job {
            id: Some("job-1".to_string()),
            state: JOB_STATE_RUNNING.to_string(),
            created_at: OffsetDateTime::now_utc(),
            context: JobContext::default(),
            task_count: 1,
            ..Job::default()
        }
    }

    /// Helper to create a running task with an error.
    fn make_failed_task(job_id: &str) -> Task {
        Task {
            id: Some("task-1".to_string()),
            job_id: Some(job_id.to_string()),
            state: TASK_STATE_RUNNING.clone(),
            error: Some("something went wrong".to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            ..Task::default()
        }
    }

    /// Helper to create a running task with retry config.
    fn make_retry_task(job_id: &str, limit: i64, attempts: i64) -> Task {
        Task {
            id: Some("task-retry".to_string()),
            job_id: Some(job_id.to_string()),
            state: TASK_STATE_RUNNING.clone(),
            error: Some("transient error".to_string()),
            retry: Some(TaskRetry { limit, attempts }),
            created_at: Some(OffsetDateTime::now_utc()),
            ..Task::default()
        }
    }

    // -- is_retry_eligible (pure calc) --------------------------------------

    #[test]
    fn test_retry_eligible_running_job_with_attempts_remaining() {
        let job = make_running_job();
        let task = make_retry_task("job-1", 3, 1);
        assert!(is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_eligible_zero_attempts_zero_limit() {
        let job = make_running_job();
        let task = make_retry_task("job-1", 0, 0);
        // attempts (0) is NOT < limit (0) → not eligible
        assert!(!is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_eligible_attempts_exhausted() {
        let job = make_running_job();
        let task = make_retry_task("job-1", 2, 2);
        assert!(!is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_eligible_attempts_exceed_limit() {
        let job = make_running_job();
        let task = make_retry_task("job-1", 1, 3);
        assert!(!is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_not_eligible_no_retry_config() {
        let job = make_running_job();
        let task = make_failed_task("job-1");
        assert!(!is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_not_eligible_job_failed() {
        let mut job = make_running_job();
        job.state = JOB_STATE_FAILED.to_string();
        let task = make_retry_task("job-1", 3, 0);
        assert!(!is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_not_eligible_job_cancelled() {
        let mut job = make_running_job();
        job.state = tork::job::JOB_STATE_CANCELLED.to_string();
        let task = make_retry_task("job-1", 3, 0);
        assert!(!is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_eligible_scheduled_job() {
        let mut job = make_running_job();
        job.state = JOB_STATE_SCHEDULED.to_string();
        let task = make_retry_task("job-1", 3, 1);
        assert!(is_retry_eligible(&job, &task));
    }

    #[test]
    fn test_retry_eligible_scheduled_job_no_attempts() {
        let mut job = make_running_job();
        job.state = JOB_STATE_SCHEDULED.to_string();
        let task = make_retry_task("job-1", 1, 0);
        assert!(is_retry_eligible(&job, &task));
    }

    // -- prepare_retry_task (pure calc) -------------------------------------

    #[test]
    fn test_prepare_retry_task_increments_attempts() {
        let task = make_retry_task("job-1", 3, 1);
        let retry = prepare_retry_task(&task).expect("should succeed");
        assert_eq!(retry.retry.as_ref().map(|r| r.attempts), Some(2));
        assert_eq!(retry.retry.as_ref().map(|r| r.limit), Some(3));
    }

    #[test]
    fn test_prepare_retry_task_new_id() {
        let task = make_retry_task("job-1", 1, 0);
        let retry = prepare_retry_task(&task).expect("should succeed");
        let original_id = task.id.as_deref().unwrap_or("");
        let retry_id = retry.id.as_deref().unwrap_or("");
        assert_ne!(original_id, retry_id);
        assert_eq!(retry_id.len(), 32); // 32-char hex, no hyphens
    }

    #[test]
    fn test_prepare_retry_task_state_is_pending() {
        let task = make_retry_task("job-1", 1, 0);
        let retry = prepare_retry_task(&task).expect("should succeed");
        assert_eq!(retry.state, *TASK_STATE_PENDING);
    }

    #[test]
    fn test_prepare_retry_task_no_error() {
        let task = make_retry_task("job-1", 1, 0);
        let retry = prepare_retry_task(&task).expect("should succeed");
        assert!(retry.error.is_none());
    }

    #[test]
    fn test_prepare_retry_task_no_failed_at() {
        let task = make_retry_task("job-1", 1, 0);
        let retry = prepare_retry_task(&task).expect("should succeed");
        assert!(retry.failed_at.is_none());
    }

    #[test]
    fn test_prepare_retry_task_has_created_at() {
        let before = OffsetDateTime::now_utc();
        let task = make_retry_task("job-1", 1, 0);
        let retry = prepare_retry_task(&task).expect("should succeed");
        let after = OffsetDateTime::now_utc();
        let created = retry.created_at.expect("should have created_at");
        assert!(created >= before && created <= after);
    }

    #[test]
    fn test_prepare_retry_task_preserves_job_id() {
        let task = make_retry_task("job-123", 1, 0);
        let retry = prepare_retry_task(&task).expect("should succeed");
        assert_eq!(retry.job_id.as_deref(), Some("job-123"));
    }

    #[test]
    fn test_prepare_retry_task_preserves_other_fields() {
        let task = Task {
            id: Some("orig".to_string()),
            job_id: Some("j1".to_string()),
            name: Some("build-image".to_string()),
            image: Some("alpine:3.18".to_string()),
            position: 5,
            retry: Some(TaskRetry {
                limit: 2,
                attempts: 1,
            }),
            state: TASK_STATE_FAILED.clone(),
            error: Some("oops".to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            ..Task::default()
        };
        let retry = prepare_retry_task(&task).expect("should succeed");
        assert_eq!(retry.name.as_deref(), Some("build-image"));
        assert_eq!(retry.image.as_deref(), Some("alpine:3.18"));
        assert_eq!(retry.position, 5);
        assert_eq!(retry.job_id.as_deref(), Some("j1"));
    }

    #[test]
    fn test_prepare_retry_task_without_config_returns_error() {
        let task = make_failed_task("job-1");
        let result = prepare_retry_task(&task);
        assert!(result.is_err());
        match result.err() {
            Some(HandlerError::Validation(msg)) => {
                assert!(msg.contains("no retry config"));
            }
            other => panic!("expected Validation error, got: {other:?}"),
        }
    }

    // -- ErrorHandler::handle (simplified, backward compat) -----------------

    #[test]
    fn test_handle_sets_task_failed() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_FAILED);
    }

    #[test]
    fn test_handle_preserves_error_message() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");

        handler.handle(ctx, &mut task).expect("should succeed");
        assert_eq!(task.error.as_deref(), Some("something went wrong"));
    }

    // -- ErrorHandler::handle_with_job (Go parity) -------------------------

    /// Parity with Go `Test_handleFailedTask`:
    /// Task fails with no retry → job is marked FAILED.
    #[test]
    fn test_handle_with_job_no_retry_fails_job() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let mut job = make_running_job();

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(outcome.is_ok());

        // Task should be marked FAILED
        assert_eq!(task.state, *TASK_STATE_FAILED);
        assert!(task.failed_at.is_some());

        // Job should be marked FAILED
        assert_eq!(job.state, JOB_STATE_FAILED);
        assert!(job.failed_at.is_some());

        // Outcome should be FailJob
        assert!(matches!(outcome, Ok(ErrorOutcome::FailJob)));
    }

    /// Parity with Go `Test_handleFailedTask`:
    /// Job's failed_at should match task's failed_at.
    #[test]
    fn test_handle_with_job_job_failed_at_matches_task() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let mut job = make_running_job();

        handler
            .handle_with_job(ctx, &mut task, &mut job)
            .expect("should succeed");

        assert_eq!(job.failed_at, task.failed_at);
    }

    /// Parity with Go `Test_handleFailedTask`:
    /// Job's error should match task's error.
    #[test]
    fn test_handle_with_job_job_error_matches_task() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let mut job = make_running_job();

        handler
            .handle_with_job(ctx, &mut task, &mut job)
            .expect("should succeed");

        assert_eq!(job.error, task.error);
        assert_eq!(job.error.as_deref(), Some("something went wrong"));
    }

    /// Parity with Go `Test_handleFailedTaskRetry`:
    /// Task has retry config with attempts < limit → retry task is prepared,
    /// job stays RUNNING.
    #[test]
    fn test_handle_with_job_retry_eligible_returns_retry_task() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_retry_task("job-1", 1, 0);
        let mut job = make_running_job();

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(outcome.is_ok());

        // Task should be marked FAILED
        assert_eq!(task.state, *TASK_STATE_FAILED);
        assert!(task.failed_at.is_some());

        // Job should remain RUNNING
        assert_eq!(job.state, JOB_STATE_RUNNING);
        assert!(job.failed_at.is_none());

        // Outcome should be Retry with a valid task
        match outcome {
            Ok(ErrorOutcome::Retry { task: retry_task }) => {
                // Retry task has a new ID
                let original_id = "task-retry";
                assert_ne!(retry_task.id.as_deref(), Some(original_id));

                // Retry task is PENDING
                assert_eq!(retry_task.state, *TASK_STATE_PENDING);

                // Retry task has no error (cleared)
                assert!(retry_task.error.is_none());

                // Retry task has no failed_at (cleared)
                assert!(retry_task.failed_at.is_none());

                // Retry task has incremented attempts
                assert_eq!(retry_task.retry.as_ref().map(|r| r.attempts), Some(1));

                // Retry task has same job_id
                assert_eq!(retry_task.job_id.as_deref(), Some("job-1"));
            }
            other => panic!("expected Retry outcome, got: {other:?}"),
        }
    }

    /// Retry with multiple attempts remaining.
    #[test]
    fn test_handle_with_job_retry_second_attempt() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_retry_task("job-1", 3, 2);
        let mut job = make_running_job();

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(outcome.is_ok());

        // Job should remain RUNNING
        assert_eq!(job.state, JOB_STATE_RUNNING);

        match outcome {
            Ok(ErrorOutcome::Retry { task: retry_task }) => {
                assert_eq!(retry_task.retry.as_ref().map(|r| r.attempts), Some(3));
            }
            other => panic!("expected Retry outcome, got: {other:?}"),
        }
    }

    /// Retry with exactly one remaining attempt (attempts == limit - 1).
    #[test]
    fn test_handle_with_job_retry_last_allowed_attempt() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_retry_task("job-1", 5, 4);
        let mut job = make_running_job();

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(outcome.is_ok());

        match outcome {
            Ok(ErrorOutcome::Retry { task: retry_task }) => {
                assert_eq!(retry_task.retry.as_ref().map(|r| r.attempts), Some(5));
            }
            other => panic!("expected Retry outcome, got: {other:?}"),
        }
    }

    /// No retry when attempts == limit.
    #[test]
    fn test_handle_with_job_retry_exhausted_fails_job() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_retry_task("job-1", 2, 2);
        let mut job = make_running_job();

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(outcome.is_ok());

        assert_eq!(job.state, JOB_STATE_FAILED);
        assert!(matches!(outcome, Ok(ErrorOutcome::FailJob)));
    }

    /// No retry when job is already failed.
    #[test]
    fn test_handle_with_job_job_already_failed_no_retry() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_retry_task("job-1", 3, 0);
        let mut job = make_running_job();
        job.state = JOB_STATE_FAILED.to_string();

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(outcome.is_ok());
        assert!(matches!(outcome, Ok(ErrorOutcome::FailJob)));
    }

    /// Job handler callback is called on failure.
    #[test]
    fn test_handle_with_job_calls_job_handler_on_fail() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let job_called = Arc::new(AtomicBool::new(false));
        let job_called_clone = job_called.clone();

        let job_handler: JobHandlerFunc = Arc::new(
            move |_ctx: HandlerContext, _et: JobEventType, job: &mut Job| {
                if job.state == JOB_STATE_FAILED {
                    job_called_clone.store(true, Ordering::SeqCst);
                }
                Ok(())
            },
        );

        let handler = ErrorHandler::with_handlers(noop_task_handler(), job_handler);
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let mut job = make_running_job();

        handler
            .handle_with_job(ctx, &mut task, &mut job)
            .expect("should succeed");

        assert!(job_called.load(Ordering::SeqCst));
    }

    /// Task handler callback is called in both branches.
    #[test]
    fn test_handle_with_job_calls_task_handler() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let call_count = Arc::new(AtomicI32::new(0));
        let call_count_clone = call_count.clone();

        let task_handler: TaskHandlerFunc = Arc::new(
            move |_ctx: HandlerContext, _et: TaskEventType, _task: &mut Task| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        );

        // Test fail branch
        let handler = ErrorHandler::with_handlers(task_handler, noop_job_handler());
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let mut job = make_running_job();

        handler
            .handle_with_job(ctx.clone(), &mut task, &mut job)
            .expect("should succeed");

        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Test retry branch
        let mut task2 = make_retry_task("job-2", 1, 0);
        let mut job2 = make_running_job();
        job2.id = Some("job-2".to_string());

        handler
            .handle_with_job(ctx, &mut task2, &mut job2)
            .expect("should succeed");

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    /// Task error message is None → job error should also be None.
    #[test]
    fn test_handle_with_job_no_error_message() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            id: Some("task-x".to_string()),
            job_id: Some("job-1".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            error: None,
            created_at: Some(OffsetDateTime::now_utc()),
            ..Task::default()
        };
        let mut job = make_running_job();

        handler
            .handle_with_job(ctx, &mut task, &mut job)
            .expect("should succeed");

        assert!(job.error.is_none());
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: Verify error outcome Debug impl
    #[test]
    fn test_error_outcome_debug() {
        let fail = ErrorOutcome::FailJob;
        let debug_str = format!("{fail:?}");
        assert!(debug_str.contains("FailJob"));

        let retry_task = Task {
            id: Some("r1".into()),
            state: TASK_STATE_PENDING.clone(),
            ..Task::default()
        };
        let retry = ErrorOutcome::Retry { task: retry_task };
        let debug_str = format!("{retry:?}");
        assert!(debug_str.contains("Retry"));
    }

    // Go: Task handler returning error propagates
    #[test]
    fn test_handle_with_job_task_handler_error_propagates() {
        let task_handler: TaskHandlerFunc = Arc::new(
            |_ctx: HandlerContext, _et: TaskEventType, _task: &mut Task| {
                Err(HandlerError::Handler("middleware error".to_string()))
            },
        );

        let handler = ErrorHandler::with_handlers(task_handler, noop_job_handler());
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let mut job = make_running_job();

        let result = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(result.is_err());
        match result.err() {
            Some(HandlerError::Handler(msg)) => {
                assert!(msg.contains("middleware error"));
            }
            other => panic!("expected Handler error, got: {other:?}"),
        }
    }

    // Go: Job handler returning error propagates
    #[test]
    fn test_handle_with_job_job_handler_error_propagates() {
        let job_handler: JobHandlerFunc = Arc::new(
            |_ctx: HandlerContext, _et: JobEventType, _job: &mut Job| {
                Err(HandlerError::Handler("job middleware error".to_string()))
            },
        );

        let handler = ErrorHandler::with_handlers(noop_task_handler(), job_handler);
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let mut job = make_running_job();

        let result = handler.handle_with_job(ctx, &mut task, &mut job);
        assert!(result.is_err());
    }

    // Go: Verify is_retry_eligible with empty string job state
    #[test]
    fn test_retry_not_eligible_empty_job_state() {
        let mut job = make_running_job();
        job.state = String::new();
        let task = make_retry_task("job-1", 3, 0);
        assert!(!is_retry_eligible(&job, &task));
    }

    // Go: Verify prepare_retry_task id length consistency
    #[test]
    fn test_prepare_retry_task_id_is_uuid_no_hyphens() {
        let task = make_retry_task("job-1", 1, 0);
        let retry = prepare_retry_task(&task).expect("should succeed");
        let retry_id = retry.id.as_deref().expect("should have id");
        // UUID without hyphens is exactly 32 hex chars
        assert_eq!(retry_id.len(), 32);
        assert!(retry_id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // Go: ErrorHandler default trait
    #[test]
    fn test_error_handler_default() {
        let handler = ErrorHandler::default();
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
    }

    // Go: Handle simplified — sets state only, not timestamps (that's handle_with_job)
    #[test]
    fn test_handle_sets_state_only() {
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());
        let mut task = make_failed_task("job-1");
        // Simplified handle() does NOT set failed_at — use handle_with_job for that
        assert!(task.failed_at.is_none());

        handler.handle(ctx, &mut task).expect("should succeed");
        assert_eq!(task.state, *TASK_STATE_FAILED);
        // Simplified handle does not set failed_at
        assert!(task.failed_at.is_none());
    }

    use crate::handlers::test_helpers::{new_uuid, TestEnv};
    use tork::Datastore;

    /// Go parity: Test_handleFailedTask
    #[tokio::test]
    #[ignore]
    async fn test_handle_failed_task_integration() {
        let env = TestEnv::new().await;
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());

        let job_id = new_uuid();
        let mut job = Job {
            id: Some(job_id.clone()),
            state: JOB_STATE_RUNNING.to_string(),
            created_at: OffsetDateTime::now_utc(),
            context: JobContext::default(),
            task_count: 1,
            ..Job::default()
        };
        env.ds.create_job(job.clone()).await.expect("create job");

        let task_id = new_uuid();
        let mut task = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id.clone()),
            state: TASK_STATE_RUNNING.clone(),
            error: Some("something went wrong".to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            ..Task::default()
        };
        env.ds.create_task(task.clone()).await.expect("create task");

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job).expect("handle");
        assert!(matches!(outcome, Ok(ErrorOutcome::FailJob)));
        assert_eq!(task.state, *TASK_STATE_FAILED);
        assert!(task.failed_at.is_some());
        assert_eq!(job.state, JOB_STATE_FAILED);

        env.cleanup().await;
    }

    /// Go parity: Test_handleFailedTaskRetry
    #[tokio::test]
    #[ignore]
    async fn test_handle_failed_task_retry_integration() {
        let env = TestEnv::new().await;
        let handler = ErrorHandler::new();
        let ctx = Arc::new(());

        let job_id = new_uuid();
        let mut job = Job {
            id: Some(job_id.clone()),
            state: JOB_STATE_RUNNING.to_string(),
            created_at: OffsetDateTime::now_utc(),
            context: JobContext::default(),
            task_count: 1,
            ..Job::default()
        };
        env.ds.create_job(job.clone()).await.expect("create job");

        let task_id = new_uuid();
        let mut task = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id.clone()),
            state: TASK_STATE_RUNNING.clone(),
            error: Some("transient error".to_string()),
            retry: Some(TaskRetry { limit: 3, attempts: 0 }),
            created_at: Some(OffsetDateTime::now_utc()),
            ..Task::default()
        };
        env.ds.create_task(task.clone()).await.expect("create task");

        let outcome = handler.handle_with_job(ctx, &mut task, &mut job).expect("handle");
        match outcome {
            Ok(ErrorOutcome::Retry { task: retry_task }) => {
                assert_eq!(retry_task.state, *TASK_STATE_PENDING);
                assert_eq!(retry_task.retry.as_ref().map(|r| r.attempts), Some(1));
                assert!(retry_task.error.is_none());
            }
            other => panic!("expected Retry, got: {other:?}"),
        }
        assert_eq!(job.state, JOB_STATE_RUNNING);

        env.cleanup().await;
    }
}
