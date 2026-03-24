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
