//! Pending handler for pending task events.
//!
//! Handles tasks in PENDING state by checking conditional expressions
//! and delegating to the scheduler for task routing.
//!
//! Go parity: `pending.go` receives a PENDING task, evaluates its `if`
//! expression, and either skips it (state→SKIPPED, timestamps→now) or
//! delegates to `scheduler.ScheduleTask()`.

use std::sync::Arc;

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
            scheduler: Scheduler::new(ds, broker),
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
            scheduler: Scheduler::new(ds, broker),
            handler,
        }
    }

    /// Create a pending handler with a custom scheduler.
    pub fn with_scheduler(scheduler: Scheduler) -> Self {
        Self {
            scheduler,
            handler: noop_task_handler(),
        }
    }

    /// Create a pending handler with custom scheduler and handler function.
    pub fn with_scheduler_and_handler(scheduler: Scheduler, handler: TaskHandlerFunc) -> Self {
        Self { scheduler, handler }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tork::task::{TASK_STATE_SCHEDULED, TASK_STATE_SKIPPED};

    // -- is_skip_condition (pure calc) --------------------------------------

    #[test]
    fn test_is_skip_condition_explicit_false() {
        let task = Task {
            r#if: Some("false".to_string()),
            ..Task::default()
        };
        assert!(is_skip_condition(&task));
    }

    #[test]
    fn test_is_skip_condition_whitespace_false() {
        let task = Task {
            r#if: Some("  false  ".to_string()),
            ..Task::default()
        };
        assert!(is_skip_condition(&task));
    }

    #[test]
    fn test_is_skip_condition_none() {
        let task = Task::default();
        assert!(!is_skip_condition(&task));
    }

    #[test]
    fn test_is_skip_condition_true() {
        let task = Task {
            r#if: Some("true".to_string()),
            ..Task::default()
        };
        assert!(!is_skip_condition(&task));
    }

    #[test]
    fn test_is_skip_condition_other_string() {
        let task = Task {
            r#if: Some("${var} == 'value'".to_string()),
            ..Task::default()
        };
        assert!(!is_skip_condition(&task));
    }

    // -- evaluate_pending (pure calc) ---------------------------------------

    #[test]
    fn test_evaluate_pending_skip() {
        let task = Task {
            r#if: Some("false".to_string()),
            ..Task::default()
        };
        assert_eq!(evaluate_pending(&task), PendingDecision::Skip);
    }

    #[test]
    fn test_evaluate_pending_schedule() {
        let task = Task::default();
        assert_eq!(evaluate_pending(&task), PendingDecision::Schedule);
    }

    // -- PendingHandler::handle (action boundary) ---------------------------

    #[test]
    fn test_handle_normal_task_becomes_scheduled() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task::default();

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_SCHEDULED);
        assert!(task.scheduled_at.is_some());
    }

    #[test]
    fn test_handle_false_condition_skips_with_timestamps() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            r#if: Some("false".to_string()),
            ..Task::default()
        };

        let before = OffsetDateTime::now_utc();
        let result = handler.handle(ctx, &mut task);
        let after = OffsetDateTime::now_utc();

        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_SKIPPED);

        // All three timestamps must be set and within the [before, after] window.
        for ts in [task.scheduled_at, task.started_at, task.completed_at] {
            let t = ts.expect("timestamp should be set on skip");
            assert!(t >= before && t <= after);
        }
    }

    #[test]
    fn test_handle_whitespace_false_skips() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            r#if: Some("  false  ".to_string()),
            ..Task::default()
        };

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_SKIPPED);
    }

    #[test]
    fn test_handle_true_condition_schedules() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            r#if: Some("true".to_string()),
            ..Task::default()
        };

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_SCHEDULED);
    }

    #[test]
    fn test_handle_non_false_expression_schedules() {
        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            r#if: Some("${result} == 'ok'".to_string()),
            ..Task::default()
        };

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_SCHEDULED);
    }

    #[test]
    fn test_handle_parallel_task_delegates_to_scheduler() {
        use tork::task::{ParallelTask, TASK_STATE_RUNNING};

        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            parallel: Some(ParallelTask {
                tasks: None,
                completions: 0,
            }),
            ..Task::default()
        };

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_RUNNING);
    }

    #[test]
    fn test_handle_each_task_delegates_to_scheduler() {
        use tork::task::{EachTask, TASK_STATE_RUNNING};

        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            each: Some(EachTask {
                var: None,
                list: None,
                task: None,
                size: 0,
                completions: 0,
                concurrency: 0,
                index: 0,
            }),
            ..Task::default()
        };

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_RUNNING);
    }

    #[test]
    fn test_handle_subjob_task_delegates_to_scheduler() {
        use tork::task::{SubJobTask, TASK_STATE_RUNNING};

        let handler = PendingHandler::new();
        let ctx = Arc::new(());
        let mut task = Task {
            subjob: Some(SubJobTask {
                id: None,
                name: None,
                description: None,
                tasks: None,
                inputs: None,
                secrets: None,
                auto_delete: None,
                output: None,
                detached: false,
                webhooks: None,
            }),
            ..Task::default()
        };

        let result = handler.handle(ctx, &mut task);
        assert!(result.is_ok());
        assert_eq!(task.state, *TASK_STATE_RUNNING);
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: Task with if="FALSE" (uppercase) should NOT be skipped
    #[test]
    fn test_is_skip_condition_uppercase_false() {
        let task = Task {
            r#if: Some("FALSE".to_string()),
            ..Task::default()
        };
        assert!(!is_skip_condition(&task));
    }

    // Go: Task with empty string if condition should not be skipped
    #[test]
    fn test_is_skip_condition_empty_string() {
        let task = Task {
            r#if: Some(String::new()),
            ..Task::default()
        };
        assert!(!is_skip_condition(&task));
    }

    // -- PendingHandler with_handler tests -----------------------------------

    #[test]
    fn test_pending_handler_with_handler_called_on_skip() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let call_count = Arc::new(AtomicI32::new(0));
        let call_count_clone = call_count.clone();

        let handler_fn: TaskHandlerFunc = Arc::new(
            move |_ctx: HandlerContext, _et: TaskEventType, _task: &mut Task| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        );

        let handler = PendingHandler::with_handler(handler_fn);
        let ctx = Arc::new(());
        let mut task = Task {
            r#if: Some("false".to_string()),
            ..Task::default()
        };

        handler.handle(ctx, &mut task).expect("should succeed");
        assert_eq!(task.state, *TASK_STATE_SKIPPED);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_pending_handler_with_handler_called_on_schedule() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let call_count = Arc::new(AtomicI32::new(0));
        let call_count_clone = call_count.clone();

        let handler_fn: TaskHandlerFunc = Arc::new(
            move |_ctx: HandlerContext, _et: TaskEventType, _task: &mut Task| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        );

        let handler = PendingHandler::with_handler(handler_fn);
        let ctx = Arc::new(());
        let mut task = Task::default();

        handler.handle(ctx, &mut task).expect("should succeed");
        assert_eq!(task.state, *TASK_STATE_SCHEDULED);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_pending_handler_default() {
        let handler = PendingHandler::default();
        let ctx = Arc::new(());
        let mut task = Task::default();
        assert!(handler.handle(ctx, &mut task).is_ok());
        assert_eq!(task.state, *TASK_STATE_SCHEDULED);
    }

    #[test]
    fn test_pending_handler_debug() {
        let handler = PendingHandler::new();
        let debug_str = format!("{handler:?}");
        assert!(debug_str.contains("PendingHandler"));
    }

    // -- apply_skip_transition timestamp verification ------------------------

    #[test]
    fn test_apply_skip_transition_sets_all_timestamps() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task::default();
        apply_skip_transition(&mut task, now);
        assert_eq!(task.state, *TASK_STATE_SKIPPED);
        assert_eq!(task.scheduled_at, Some(now));
        assert_eq!(task.started_at, Some(now));
        assert_eq!(task.completed_at, Some(now));
    }

    use crate::handlers::test_helpers::{new_uuid, TestEnv};
    use tork::task::TASK_STATE_SCHEDULED;
    use tork::Datastore;

    /// Go parity: Test_handlePendingTask
    #[tokio::test]
    #[ignore]
    async fn test_handle_pending_task_integration() {
        let env = TestEnv::new().await;
        let ds = env.ds.clone() as Arc<dyn tork::Datastore>;
        let scheduler = Scheduler::new();
        let handler = PendingHandler::with_scheduler(scheduler);
        let ctx = Arc::new(());

        let job_id = new_uuid();
        let job = tork::job::Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_SCHEDULED.to_string(),
            ..tork::job::Job::default()
        };
        env.ds.create_job(job).await.expect("create job");

        let mut task = Task {
            id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            name: Some("echo-hello".into()),
            run: Some("echo hello".into()),
            ..Task::default()
        };

        handler.handle(ctx, &mut task).expect("handle pending");
        assert_eq!(task.state, *TASK_STATE_SCHEDULED);
        assert!(task.scheduled_at.is_some());

        env.cleanup().await;
    }

    /// Go parity: Test_handleConditionalTask — task with if="false" gets skipped
    #[tokio::test]
    #[ignore]
    async fn test_handle_conditional_task_integration() {
        let env = TestEnv::new().await;
        let scheduler = Scheduler::new();
        let handler = PendingHandler::with_scheduler(scheduler);
        let ctx = Arc::new(());

        let mut task = Task {
            id: Some(new_uuid()),
            r#if: Some("false".to_string()),
            ..Task::default()
        };

        handler.handle(ctx, &mut task).expect("handle conditional");
        assert_eq!(task.state, *TASK_STATE_SKIPPED);
        assert!(task.scheduled_at.is_some());
        assert!(task.started_at.is_some());
        assert!(task.completed_at.is_some());

        env.cleanup().await;
    }
}
