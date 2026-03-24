//! Started handler for task start events.
//!
//! Port of Go `internal/coordinator/handlers/started.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives STARTED tasks
//! 2. Validates job is in RUNNING or SCHEDULED state
//! 3. If job was SCHEDULED, transition to RUNNING
//! 4. Updates task state and node assignment via ds.UpdateTask
//! 5. Publishes cancellation to node queue if job is not running

use std::sync::Arc;

use tork::job::{JOB_STATE_RUNNING, JOB_STATE_SCHEDULED};
use tork::task::{Task, TASK_STATE_CANCELLED, TASK_STATE_RUNNING, TASK_STATE_SCHEDULED};
use tork::{Broker, Datastore};

use crate::handlers::{HandlerContext, HandlerError, JobEventType, JobHandlerFunc};

// ---------------------------------------------------------------------------
// Pure Calculations (Data → Calc)
// ---------------------------------------------------------------------------

/// Result of analyzing a started task against its job state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartedAction {
    /// Task should proceed (job is RUNNING or SCHEDULED).
    Proceed,
    /// Task should be cancelled (job is in a terminal or unexpected state).
    Cancel,
}

/// Determines what action to take based on the job state.
/// Go: `if j.State != tork.JobStateRunning && j.State != tork.JobStateScheduled`
#[must_use]
pub(crate) fn calculate_started_action(job_state: &str) -> StartedAction {
    match job_state {
        s if s == JOB_STATE_RUNNING || s == JOB_STATE_SCHEDULED => StartedAction::Proceed,
        _ => StartedAction::Cancel,
    }
}

/// Checks if a job needs a SCHEDULED → RUNNING transition.
/// Go: `if j.State == tork.JobStateScheduled`
#[must_use]
pub(crate) fn needs_job_transition(job_state: &str) -> bool {
    job_state == JOB_STATE_SCHEDULED
}

/// Prepares the task update for a valid start event.
/// Only marks RUNNING if the task was SCHEDULED (guards against out-of-order
/// completions). Always updates node_id to track the latest worker.
/// Go: `if u.State == tork.TaskStateScheduled { u.State = RUNNING; u.StartedAt = &now }`
///     `u.NodeID = t.NodeID`
#[must_use]
pub(crate) fn prepare_task_start_update(current: &Task, incoming: &Task) -> Task {
    let (state, started_at) = if current.state == *TASK_STATE_SCHEDULED {
        (
            TASK_STATE_RUNNING.clone(),
            Some(time::OffsetDateTime::now_utc()),
        )
    } else {
        (current.state.clone(), current.started_at)
    };

    Task {
        state,
        started_at,
        node_id: incoming.node_id.clone(),
        ..current.clone()
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

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Started handler for processing task start events.
///
/// Holds references to the datastore and broker for I/O operations.
/// All core logic is delegated to pure calculation functions above.
pub struct StartedHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
    on_job: JobHandlerFunc,
}

impl std::fmt::Debug for StartedHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartedHandler").finish()
    }
}

impl StartedHandler {
    /// Create a new started handler with datastore and broker dependencies.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self {
            ds,
            broker,
            on_job: Arc::new(|_ctx: HandlerContext, _et: JobEventType, _job: &mut tork::job::Job| Ok(())),
        }
    }

    /// Create a started handler with datastore, broker, and job handler.
    pub fn with_on_job(
        ds: Arc<dyn Datastore>,
        broker: Arc<dyn Broker>,
        on_job: JobHandlerFunc,
    ) -> Self {
        Self { ds, broker, on_job }
    }

    /// Handle a task start event.
    ///
    /// Go parity (`handle`):
    /// 1. Get job by ID from datastore
    /// 2. If job is not RUNNING/SCHEDULED, cancel the task via broker
    /// 3. If job is SCHEDULED, transition to RUNNING via datastore update
    /// 4. Update task state and node assignment via datastore
    pub async fn handle(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        // 1. Get job and validate state
        let job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job {job_id} not found")))?;

        match calculate_started_action(&job.state) {
            StartedAction::Cancel => {
                // 2a. Job is not running — cancel the task via the node's queue
                let node_id = task.node_id.as_deref().ok_or_else(|| {
                    HandlerError::Validation("node ID required for cancellation".into())
                })?;
                let node = self
                    .ds
                    .get_node_by_id(node_id.to_string())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| HandlerError::NotFound(format!("node {node_id} not found")))?;
                let queue_name = node
                    .queue
                    .clone()
                    .ok_or_else(|| HandlerError::Validation("node has no queue".into()))?;

                let cancelled = prepare_cancelled_task(task);
                self.broker
                    .publish_task(queue_name, &cancelled)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
                Ok(())
            }
            StartedAction::Proceed => {
                // 2b. If job is SCHEDULED, transition to RUNNING
                // Go: if j.State == tork.JobStateScheduled { j.State = tork.JobStateRunning; h.onJob(ctx, job.StateChange, j) }
                if needs_job_transition(&job.state) {
                    let mut updated_job = tork::Job {
                        state: JOB_STATE_RUNNING.to_string(),
                        ..job.clone()
                    };
                    self.ds
                        .update_job(job_id.to_string(), updated_job.clone())
                        .await
                        .map_err(|e| HandlerError::Datastore(e.to_string()))?;
                    // Call onJob with StateChange to trigger middleware/webhooks
                    // Go: h.onJob(ctx, job.StateChange, j)
                    (self.on_job)(Arc::new(()), JobEventType::StateChange, &mut updated_job)?;
                }

                // 3. Update task state and node assignment
                let current = self
                    .ds
                    .get_task_by_id(task_id.to_string())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| HandlerError::NotFound(format!("task {task_id} not found")))?;

                let updated_task = prepare_task_start_update(&current, task);
                self.ds
                    .update_task(task_id.to_string(), updated_task)
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?;

                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
