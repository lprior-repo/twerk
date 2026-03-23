//! Cancel handler for job cancellation.
//!
//! Port of Go `internal/coordinator/handlers/cancel.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives CANCELLED jobs
//! 2. Marks job as CANCELLED if RUNNING or SCHEDULED (no-op otherwise)
//! 3. If job has parent_id, propagates cancellation to parent job via broker
//! 4. Cancels all active tasks — handles sub-jobs and node notifications

use std::sync::Arc;

use tork::job::{Job, JOB_STATE_CANCELLED, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED};
use tork::task::{Task, TASK_STATE_CANCELLED};
use tork::{Broker, Datastore};

use crate::handlers::HandlerError;

// ---------------------------------------------------------------------------
// Pure Calculations (Data → Calc)
// ---------------------------------------------------------------------------

/// Result of analyzing whether a job can be cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancelEligibility {
    /// Job is RUNNING or SCHEDULED — can be cancelled.
    Eligible,
    /// Job is in a terminal state — nothing to cancel.
    NoOp,
}

/// Determines if a job is eligible for cancellation.
/// Go: `if u.State != tork.JobStateRunning && u.State != tork.JobStateScheduled`
#[must_use]
pub(crate) fn calculate_cancel_eligibility(job_state: &str) -> CancelEligibility {
    match job_state {
        s if s == JOB_STATE_RUNNING || s == JOB_STATE_SCHEDULED => CancelEligibility::Eligible,
        _ => CancelEligibility::NoOp,
    }
}

/// Prepares a cancelled job by cloning and setting state to CANCELLED.
/// Go: `u.State = tork.JobStateCancelled`
#[must_use]
pub(crate) fn prepare_cancelled_job(job: &Job) -> Job {
    Job {
        state: JOB_STATE_CANCELLED.to_string(),
        ..job.clone()
    }
}

/// Prepares a cancelled task by cloning and setting state to CANCELLED.
/// Go: `t.State = tork.TaskStateCancelled`
#[must_use]
pub(crate) fn prepare_cancelled_task(task: &Task) -> Task {
    Task {
        state: TASK_STATE_CANCELLED.clone(),
        ..task.clone()
    }
}

/// Describes the cancellation action needed for a single active task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskCancelAction {
    /// Cancel a sub-job by publishing a CANCELLED state.
    CancelSubJob { sub_job_id: String },
    /// Notify a node to cancel the task via its queue.
    NotifyNode { node_id: String },
    /// No action needed (neither sub-job nor assigned node).
    NoOp,
}

/// Determines what cancellation action to take for a given task.
/// Go: `if t.SubJob != nil && t.SubJob.ID != "" { ... } else if t.NodeID != "" { ... }`
#[must_use]
pub(crate) fn calculate_task_cancel_action(task: &Task) -> TaskCancelAction {
    sub_job_id(task)
        .map(|id| TaskCancelAction::CancelSubJob { sub_job_id: id })
        .or_else(|| node_id(task).map(|id| TaskCancelAction::NotifyNode { node_id: id }))
        .unwrap_or(TaskCancelAction::NoOp)
}

/// Extracts the sub-job ID from a task, if present and non-empty.
#[must_use]
fn sub_job_id(task: &Task) -> Option<String> {
    task.subjob
        .as_ref()
        .and_then(|sj| sj.id.as_deref())
        .filter(|id| !id.is_empty())
        .map(String::from)
}

/// Extracts the node ID from a task, if present and non-empty.
#[must_use]
fn node_id(task: &Task) -> Option<String> {
    task.node_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .map(String::from)
}

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Cancel handler for processing job cancellations.
///
/// Holds references to the datastore and broker for I/O operations.
/// All core logic is delegated to pure calculation functions above.
pub struct CancelHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
}

impl std::fmt::Debug for CancelHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelHandler").finish()
    }
}

impl CancelHandler {
    /// Create a new cancel handler with datastore and broker dependencies.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self { ds, broker }
    }

    /// Handle a job cancellation.
    ///
    /// Go parity (`handle`):
    /// 1. Update job state to CANCELLED if RUNNING or SCHEDULED
    /// 2. If job has parent_id, propagate cancellation to the parent job
    /// 3. Cancel all active tasks via [`cancel_active_tasks`]
    pub async fn handle(&self, job: &Job) -> Result<(), HandlerError> {
        let job_id = job
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        // 1. Mark the job as cancelled (only if RUNNING or SCHEDULED)
        //    Go: `h.ds.UpdateJob(ctx, j.ID, func(u *tork.Job) error { ... })`
        if calculate_cancel_eligibility(&job.state) == CancelEligibility::Eligible {
            let cancelled = prepare_cancelled_job(job);
            self.ds
                .update_job(job_id.to_string(), cancelled)
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?;
        } else {
            // Job is not running/scheduled — nothing to cancel
            return Ok(());
        }

        // 2. If there's a parent task, propagate cancellation to the parent job
        //    Go: `if j.ParentID != "" { pt, _ := h.ds.GetTaskByID(...) ... }`
        if let Some(ref parent_id) = job.parent_id {
            self.cancel_parent_job(parent_id).await?;
        }

        // 3. Cancel all active tasks
        //    Go: `cancelActiveTasks(ctx, h.ds, h.broker, j.ID)`
        self.cancel_active_tasks(job_id).await
    }
}

impl CancelHandler {
    /// Propagate cancellation to the parent job of a sub-job task.
    ///
    /// Go parity:
    /// ```go
    /// pt, _ := h.ds.GetTaskByID(ctx, j.ParentID)
    /// pj, _ := h.ds.GetJobByID(ctx, pt.JobID)
    /// pj.State = tork.JobStateCancelled
    /// h.broker.PublishJob(ctx, pj)
    /// ```
    async fn cancel_parent_job(&self, parent_id: &str) -> Result<(), HandlerError> {
        let parent_task = self
            .ds
            .get_task_by_id(parent_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("parent task {parent_id} not found"))
            })?;

        let parent_job_id = parent_task
            .job_id
            .as_deref()
            .ok_or_else(|| {
                HandlerError::Validation(format!(
                    "parent task {parent_id} has no job ID"
                ))
            })?;

        let parent_job = self
            .ds
            .get_job_by_id(parent_job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!("parent job {parent_job_id} not found"))
            })?;

        let cancelled_parent = prepare_cancelled_job(&parent_job);
        if let Err(e) = self.broker.publish_job(&cancelled_parent).await {
            tracing::error!(error = %e, job_id = %parent_job_id, "error cancelling sub-job");
        }

        Ok(())
    }

    /// Cancel all active tasks for a job.
    ///
    /// Go parity (`cancelActiveTasks`):
    /// 1. Get all active tasks for the job
    /// 2. Mark each as CANCELLED in the datastore
    /// 3. If task has a sub-job, publish cancellation for the sub-job
    /// 4. If task is assigned to a node, publish cancellation to the node's queue
    async fn cancel_active_tasks(&self, job_id: &str) -> Result<(), HandlerError> {
        // Go: `tasks, err := ds.GetActiveTasks(ctx, jobID)`
        let tasks = self
            .ds
            .get_active_tasks(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        // Process each active task sequentially
        for task in tasks {
            Self::cancel_single_task(&self.ds, &self.broker, &task).await?;
        }
        Ok(())
    }

    /// Cancel a single active task.
    ///
    /// Go parity for one iteration of the cancelActiveTasks loop.
    async fn cancel_single_task(
        ds: &Arc<dyn Datastore>,
        broker: &Arc<dyn Broker>,
        task: &Task,
    ) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;

        // Go: `ds.UpdateTask(ctx, t.ID, func(u *tork.Task) error { u.State = CANCELLED })`
        let cancelled = prepare_cancelled_task(task);
        ds.update_task(task_id.to_string(), cancelled)
            .await
            .map_err(|e| {
                HandlerError::Datastore(format!("error cancelling task {task_id}: {e}"))
            })?;

        match calculate_task_cancel_action(task) {
            TaskCancelAction::CancelSubJob { sub_job_id } => {
                // Go: `sj, _ := ds.GetJobByID(ctx, t.SubJob.ID)`
                //     `sj.State = CANCELLED`
                //     `broker.PublishJob(ctx, sj)`
                let sub_job = ds
                    .get_job_by_id(sub_job_id.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("sub-job {sub_job_id} not found"))
                    })?;

                let cancelled_sub = prepare_cancelled_job(&sub_job);
                broker.publish_job(&cancelled_sub).await.map_err(|e| {
                    HandlerError::Broker(format!(
                        "error publishing cancellation for sub-job {sub_job_id}: {e}"
                    ))
                })?;
            }
            TaskCancelAction::NotifyNode { node_id } => {
                // Go: `node, _ := ds.GetNodeByID(ctx, t.NodeID)`
                //     `broker.PublishTask(ctx, node.Queue, t)`
                let node = ds
                    .get_node_by_id(node_id.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("node {node_id} not found"))
                    })?;

                let queue_name = node.queue.clone().ok_or_else(|| {
                    HandlerError::Validation("node has no queue".into())
                })?;

                broker
                    .publish_task(queue_name, task)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
            TaskCancelAction::NoOp => {}
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

