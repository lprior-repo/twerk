//! Progress handler for task progress updates.
//!
//! Port of Go `internal/coordinator/handlers/progress.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives task progress updates (0.0–100.0)
//! 2. Clamps progress to valid range [0, 100]
//! 3. Updates task progress via `ds.UpdateTask`
//! 4. Fetches the parent job via `ds.GetJobByID`
//! 5. Calculates aggregate job progress (weighted average by position)
//! 6. Updates job progress via `ds.UpdateJob`
//! 7. Calls `onJob` middleware with Progress event
//!
//! # Architecture
//!
//! - **Calc** (`clamp_progress`, `calculate_job_progress`): Pure functions
//!   that compute clamped progress and aggregate job progress without I/O.
//! - **Actions** (`ProgressHandler::handle`): Performs datastore I/O and
//!   delegates to middleware handlers at the shell boundary.

use std::sync::Arc;

use tork::job::Job;
use tork::task::Task;
use tork::Datastore;

use crate::handlers::{HandlerError, JobEventType, JobHandlerFunc};

// ---------------------------------------------------------------------------
// Pure Calculations (Data → Calc)
// ---------------------------------------------------------------------------

/// Clamps a progress value to the valid range [0, 100].
///
/// Go parity:
/// ```go
/// if t.Progress < 0 { t.Progress = 0 } else if t.Progress > 100 { t.Progress = 100 }
/// ```
#[must_use]
pub const fn clamp_progress(progress: f64) -> f64 {
    progress.clamp(0.0, 100.0)
}

/// Calculates aggregate job progress from task position and progress.
///
/// The formula accounts for tasks already completed (position - 1) plus the
/// current task's fractional progress, divided by the total task count.
///
/// Go parity:
/// ```go
/// if t.Progress == 0 {
///     j.Progress = (float64(j.Position - 1)) / float64(j.TaskCount) * 100
/// } else {
///     j.Progress = (float64(j.Position-1) + (t.Progress / 100)) / float64(j.TaskCount) * 100
/// }
/// j.Progress = math.Round(j.Progress*100) / 100
/// ```
///
/// Returns `0.0` when `task_count <= 0` to avoid division by zero.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn calculate_job_progress(position: i64, task_count: i64, task_progress: f64) -> f64 {
    if task_count <= 0 {
        return 0.0;
    }
    let pos = (position - 1) as f64;
    let count = task_count as f64;
    let raw = if task_progress == 0.0 {
        pos / count * 100.0
    } else {
        (pos + task_progress / 100.0) / count * 100.0
    };
    // Round to 2 decimal places (Go: math.Round(j.Progress*100) / 100)
    (raw * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Progress handler for processing task progress updates.
///
/// Holds references to the datastore and job middleware callback.
/// Go parity with `progressHandler` in `progress.go`.
pub struct ProgressHandler {
    ds: Arc<dyn Datastore>,
    on_job: JobHandlerFunc,
}

impl std::fmt::Debug for ProgressHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressHandler").finish()
    }
}

impl ProgressHandler {
    /// Create a new progress handler with datastore and job callback.
    ///
    /// Go parity: `NewProgressHandler(ds datastore.Datastore, onJob job.HandlerFunc)`
    pub fn new(ds: Arc<dyn Datastore>, on_job: JobHandlerFunc) -> Self {
        Self { ds, on_job }
    }

    /// Handle a progress update for a task.
    ///
    /// Go parity (`handle`):
    /// 1. Clamps progress to [0, 100]
    /// 2. Updates task progress via datastore
    /// 3. Fetches job and calculates aggregate progress
    /// 4. Updates job progress via datastore
    /// 5. Calls onJob middleware with Progress event
    ///
    /// # Errors
    ///
    /// Returns [`HandlerError::Validation`] if task or job ID is missing.
    /// Returns [`HandlerError::Datastore`] if datastore operations fail.
    /// Returns [`HandlerError::Handler`] if the onJob callback fails.
    pub async fn handle(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        // Step 1: Clamp progress to [0, 100]
        let clamped = clamp_progress(task.progress);

        // Step 2: Update task progress via datastore
        // Go: `ds.UpdateTask(ctx, t.ID, func(u *tork.Task) error { u.Progress = t.Progress; return nil })`
        let updated_task = Task {
            progress: clamped,
            ..task.clone()
        };
        self.ds
            .update_task(task_id.to_string(), updated_task)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // Step 3: Get job to calculate aggregate progress
        // Go: `j, err := h.ds.GetJobByID(ctx, t.JobID)`
        let job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job {job_id} not found")))?;

        // Step 4: Calculate and update job progress
        let job_progress = calculate_job_progress(job.position, job.task_count, clamped);
        let updated_job = Job {
            progress: job_progress,
            ..job.clone()
        };
        // Go: `ds.UpdateJob(ctx, t.JobID, func(u *tork.Job) error { u.Progress = j.Progress; return nil })`
        self.ds
            .update_job(job_id.to_string(), updated_job.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // Step 5: Call onJob middleware with Progress event
        // Go: `return h.onJob(ctx, job.Progress, j)`
        let mut job_for_callback = updated_job;
        (self.on_job)(Arc::new(()), JobEventType::Progress, &mut job_for_callback)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::noop_job_handler;

    // -- clamp_progress (pure calc) -----------------------------------------

    #[test]
    fn test_clamp_progress_normal() {
        assert_eq!(clamp_progress(50.0), 50.0);
    }

    #[test]
    fn test_clamp_progress_zero() {
        assert_eq!(clamp_progress(0.0), 0.0);
    }

    #[test]
    fn test_clamp_progress_hundred() {
        assert_eq!(clamp_progress(100.0), 100.0);
    }

    #[test]
    fn test_clamp_progress_negative() {
        assert_eq!(clamp_progress(-10.0), 0.0);
    }

    #[test]
    fn test_clamp_progress_over_100() {
        assert_eq!(clamp_progress(101.0), 100.0);
    }

    #[test]
    fn test_clamp_progress_large_negative() {
        assert_eq!(clamp_progress(-1000.0), 0.0);
    }

    #[test]
    fn test_clamp_progress_large_over() {
        assert_eq!(clamp_progress(10000.0), 100.0);
    }

    // -- calculate_job_progress (pure calc) ---------------------------------

    // Go test "no progress": TaskCount=2, Position=1, Progress=0 → 0
    #[test]
    fn test_job_progress_no_progress() {
        assert_eq!(calculate_job_progress(1, 2, 0.0), 0.0);
    }

    // Go test "little progress": TaskCount=2, Position=1, Progress=5 → 2.5
    #[test]
    fn test_job_progress_little_progress() {
        assert_eq!(calculate_job_progress(1, 2, 5.0), 2.5);
    }

    // Go test "half progress": TaskCount=2, Position=1, Progress=50 → 25
    #[test]
    fn test_job_progress_half() {
        assert_eq!(calculate_job_progress(1, 2, 50.0), 25.0);
    }

    // Go test "done": TaskCount=2, Position=1, Progress=100 → 50
    #[test]
    fn test_job_progress_done() {
        assert_eq!(calculate_job_progress(1, 2, 100.0), 50.0);
    }

    // Go test "backward progress": Progress=-10 → clamped to 0, job=0
    #[test]
    fn test_job_progress_backward() {
        let clamped = clamp_progress(-10.0);
        assert_eq!(clamped, 0.0);
        assert_eq!(calculate_job_progress(1, 2, clamped), 0.0);
    }

    // Go test "too much progress": Progress=101 → clamped to 100, job=50
    #[test]
    fn test_job_progress_too_much() {
        let clamped = clamp_progress(101.0);
        assert_eq!(clamped, 100.0);
        assert_eq!(calculate_job_progress(1, 2, clamped), 50.0);
    }

    // Edge: second task (position=2) fully complete → 100% job progress
    #[test]
    fn test_job_progress_second_task_done() {
        assert_eq!(calculate_job_progress(2, 2, 100.0), 100.0);
    }

    // Edge: zero task_count → returns 0.0 (avoids division by zero)
    #[test]
    fn test_job_progress_zero_task_count() {
        assert_eq!(calculate_job_progress(1, 0, 50.0), 0.0);
    }

    // Edge: negative task_count → returns 0.0
    #[test]
    fn test_job_progress_negative_task_count() {
        assert_eq!(calculate_job_progress(1, -1, 50.0), 0.0);
    }

    // Rounding: verify 2-decimal precision
    #[test]
    fn test_job_progress_rounding() {
        // 3 tasks, position 1, progress 33.333... → should round to 2 decimals
        let result = calculate_job_progress(1, 3, 33.333);
        let expected = ((33.333_f64 / 100.0) / 3.0 * 100.0 * 100.0).round() / 100.0;
        assert_eq!(result, expected);
    }

    // Multiple tasks at different positions
    #[test]
    fn test_job_progress_position_3_of_5_done() {
        // Position 3 of 5, fully complete: (2 + 100/100) / 5 * 100 = 60
        assert_eq!(calculate_job_progress(3, 5, 100.0), 60.0);
    }

    #[test]
    fn test_job_progress_position_3_of_5_half() {
        // Position 3 of 5, 50% complete: (2 + 50/100) / 5 * 100 = 50
        assert_eq!(calculate_job_progress(3, 5, 50.0), 50.0);
    }

    #[test]
    fn test_job_progress_position_3_of_5_no_progress() {
        // Position 3 of 5, 0% complete: (3-1) / 5 * 100 = 40
        assert_eq!(calculate_job_progress(3, 5, 0.0), 40.0);
    }

    // -- ProgressHandler construction ---------------------------------------

    #[test]
    fn test_progress_handler_new() {
        let ds = Arc::new(MockDatastore);
        let handler = ProgressHandler::new(ds, noop_job_handler());
        assert_eq!(format!("{handler:?}"), "ProgressHandler");
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: clamp_progress at exact boundaries
    #[test]
    fn test_clamp_progress_exact_zero() {
        assert_eq!(clamp_progress(0.0), 0.0);
    }

    #[test]
    fn test_clamp_progress_exact_hundred() {
        assert_eq!(clamp_progress(100.0), 100.0);
    }

    // Go: Very small positive values
    #[test]
    fn test_clamp_progress_very_small() {
        assert_eq!(clamp_progress(0.001), 0.001);
    }

    // Go: calculate_job_progress with single task
    #[test]
    fn test_job_progress_single_task() {
        // 1 task, position 1, 100% done → 100%
        assert_eq!(calculate_job_progress(1, 1, 100.0), 100.0);
    }

    // Go: calculate_job_progress with many tasks
    #[test]
    fn test_job_progress_ten_tasks_first_done() {
        // 10 tasks, position 1, fully done → 10%
        assert_eq!(calculate_job_progress(1, 10, 100.0), 10.0);
    }

    // Go: calculate_job_progress rounding edge cases
    #[test]
    fn test_job_progress_third_task_fully_done() {
        // 3 tasks, position 3, fully done → 100%
        assert_eq!(calculate_job_progress(3, 3, 100.0), 100.0);
    }

    #[test]
    fn test_job_progress_first_of_three_half() {
        // 3 tasks, position 1, 50% done → 16.67%
        let result = calculate_job_progress(1, 3, 50.0);
        let expected = (50.0_f64 / 100.0 / 3.0 * 100.0 * 100.0).round() / 100.0;
        assert_eq!(result, expected);
    }

    // Go: No progress (task_progress == 0.0) formula branch
    #[test]
    fn test_job_progress_no_progress_branch() {
        // Position 2 of 4, 0 progress: (2-1)/4*100 = 25
        assert_eq!(calculate_job_progress(2, 4, 0.0), 25.0);
    }

    // Go: Progress branch (task_progress != 0.0)
    #[test]
    fn test_job_progress_with_progress_branch() {
        // Position 2 of 4, 25 progress: (1 + 25/100)/4*100 = 31.25
        let result = calculate_job_progress(2, 4, 25.0);
        let raw: f64 = (1.0 + 25.0 / 100.0) / 4.0 * 100.0;
        let expected = (raw * 100.0).round() / 100.0;
        assert_eq!(result, expected);
    }

    use crate::handlers::test_helpers::{new_uuid, TestEnv};

    /// Go parity: Test_handleProgress — updates task and job progress
    #[tokio::test]
    #[ignore]
    async fn test_handle_progress_integration() {
        let env = TestEnv::new().await;
        let handler = ProgressHandler::new(
            env.ds.clone() as Arc<dyn Datastore>,
            noop_job_handler(),
        );

        let job_id = new_uuid();
        let job = Job {
            id: Some(job_id.clone()),
            position: 1,
            task_count: 2,
            ..Job::default()
        };
        env.ds.create_job(job).await.expect("create job");

        let task_id = new_uuid();
        let task = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id.clone()),
            progress: 50.0,
            ..Task::default()
        };

        handler.handle(&task).await.expect("handle progress");

        let updated_task = env.ds.get_task_by_id(task_id).await.expect("get task").expect("task exists");
        assert_eq!(updated_task.progress, 50.0);

        let updated_job = env.ds.get_job_by_id(job_id).await.expect("get job").expect("job exists");
        // Position 1 of 2, progress 50 → (0 + 50/100) / 2 * 100 = 25
        assert_eq!(updated_job.progress, 25.0);

        env.cleanup().await;
    }

    // -- Mock datastore for construction tests ------------------------------

    struct MockDatastore;

    impl Datastore for MockDatastore {
        fn create_task(&self, _task: Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_task(&self, _id: String, _task: Task) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_tasks(&self, _job_id: String) -> tork::datastore::BoxedFuture<Vec<Task>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_next_task(
            &self,
            _parent_task_id: String,
        ) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn create_task_log_part(
            &self,
            _part: tork::task::TaskLogPart,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_log_parts(
            &self,
            _task_id: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 20,
                })
            })
        }
        fn create_node(&self, _node: tork::node::Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_node(&self, _id: String, _node: tork::node::Node) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_node_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::node::Node>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_nodes(&self) -> tork::datastore::BoxedFuture<Vec<tork::node::Node>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn create_job(&self, _job: Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_job(&self, _id: String, _job: Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<Job>> {
            Box::pin(async { Ok(None) })
        }
        fn get_job_log_parts(
            &self,
            _job_id: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 20,
                })
            })
        }
        fn get_jobs(
            &self,
            _current_user: String,
            _q: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::JobSummary>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 20,
                })
            })
        }
        fn create_scheduled_job(&self, _job: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(
            &self,
        ) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_scheduled_jobs(
            &self,
            _current_user: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>> {
            Box::pin(async {
                Ok(tork::datastore::Page {
                    items: vec![],
                    total: 0,
                    page: 1,
                    size: 20,
                })
            })
        }
        fn get_scheduled_job_by_id(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(
            &self,
            _id: String,
            _job: tork::job::ScheduledJob,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn delete_scheduled_job(&self, _id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn create_user(&self, _user: tork::user::User) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_user(
            &self,
            _username: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::user::User>> {
            Box::pin(async { Ok(None) })
        }
        fn create_role(&self, _role: tork::role::Role) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_role(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::role::Role>> {
            Box::pin(async { Ok(None) })
        }
        fn get_roles(&self) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_user_roles(&self, _user_id: String) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn assign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn unassign_role(&self, _user_id: String, _role_id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_metrics(&self) -> tork::datastore::BoxedFuture<tork::stats::Metrics> {
            Box::pin(async { Ok(tork::stats::Metrics::default()) })
        }
        fn health_check(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }
}
