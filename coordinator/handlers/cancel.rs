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

#[cfg(test)]
mod tests {
    use super::*;
    use tork::task::SubJobTask;

    #[test]
    fn test_calculate_cancel_eligibility_running() {
        assert_eq!(
            calculate_cancel_eligibility(JOB_STATE_RUNNING),
            CancelEligibility::Eligible
        );
    }

    #[test]
    fn test_calculate_cancel_eligibility_scheduled() {
        assert_eq!(
            calculate_cancel_eligibility(JOB_STATE_SCHEDULED),
            CancelEligibility::Eligible
        );
    }

    #[test]
    fn test_calculate_cancel_eligibility_cancelled() {
        assert_eq!(
            calculate_cancel_eligibility(JOB_STATE_CANCELLED),
            CancelEligibility::NoOp
        );
    }

    #[test]
    fn test_calculate_cancel_eligibility_completed() {
        assert_eq!(
            calculate_cancel_eligibility(tork::job::JOB_STATE_COMPLETED),
            CancelEligibility::NoOp
        );
    }

    #[test]
    fn test_calculate_cancel_eligibility_failed() {
        assert_eq!(
            calculate_cancel_eligibility(tork::job::JOB_STATE_FAILED),
            CancelEligibility::NoOp
        );
    }

    #[test]
    fn test_calculate_cancel_eligibility_pending() {
        assert_eq!(
            calculate_cancel_eligibility(tork::job::JOB_STATE_PENDING),
            CancelEligibility::NoOp
        );
    }

    #[test]
    fn test_calculate_cancel_eligibility_restart() {
        assert_eq!(
            calculate_cancel_eligibility(tork::job::JOB_STATE_RESTART),
            CancelEligibility::NoOp
        );
    }

    #[test]
    fn test_calculate_cancel_eligibility_empty() {
        assert_eq!(calculate_cancel_eligibility(""), CancelEligibility::NoOp);
    }

    #[test]
    fn test_prepare_cancelled_job() {
        let job = Job {
            id: Some("job-1".to_string()),
            state: JOB_STATE_RUNNING.to_string(),
            ..Job::default()
        };
        let cancelled = prepare_cancelled_job(&job);
        assert_eq!(cancelled.id.as_deref(), Some("job-1"));
        assert_eq!(cancelled.state, JOB_STATE_CANCELLED);
    }

    #[test]
    fn test_prepare_cancelled_job_preserves_fields() {
        let job = Job {
            id: Some("job-1".to_string()),
            parent_id: Some("parent-1".to_string()),
            name: Some("test-job".to_string()),
            state: JOB_STATE_SCHEDULED.to_string(),
            task_count: 5,
            ..Job::default()
        };
        let cancelled = prepare_cancelled_job(&job);
        assert_eq!(cancelled.parent_id.as_deref(), Some("parent-1"));
        assert_eq!(cancelled.name.as_deref(), Some("test-job"));
        assert_eq!(cancelled.task_count, 5);
    }

    #[test]
    fn test_prepare_cancelled_job_preserves_schedule() {
        let job = Job {
            id: Some("job-1".to_string()),
            state: JOB_STATE_RUNNING.to_string(),
            schedule: Some(tork::job::JobSchedule {
                id: Some("sched-1".to_string()),
                cron: Some("* * * * *".to_string()),
            }),
            ..Job::default()
        };
        let cancelled = prepare_cancelled_job(&job);
        assert_eq!(
            cancelled
                .schedule
                .as_ref()
                .and_then(|s| s.id.as_deref()),
            Some("sched-1")
        );
    }

    #[test]
    fn test_prepare_cancelled_task() {
        let task = Task {
            id: Some("task-1".to_string()),
            state: tork::task::TASK_STATE_RUNNING.clone(),
            ..Task::default()
        };
        let cancelled = prepare_cancelled_task(&task);
        assert_eq!(cancelled.id.as_deref(), Some("task-1"));
        assert_eq!(cancelled.state, TASK_STATE_CANCELLED);
    }

    #[test]
    fn test_prepare_cancelled_task_preserves_fields() {
        let task = Task {
            id: Some("task-1".to_string()),
            job_id: Some("job-1".to_string()),
            position: 3,
            node_id: Some("node-1".to_string()),
            state: tork::task::TASK_STATE_RUNNING.clone(),
            ..Task::default()
        };
        let cancelled = prepare_cancelled_task(&task);
        assert_eq!(cancelled.job_id.as_deref(), Some("job-1"));
        assert_eq!(cancelled.position, 3);
        assert_eq!(cancelled.node_id.as_deref(), Some("node-1"));
    }

    #[test]
    fn test_calculate_task_cancel_action_subjob() {
        let task = Task {
            id: Some("task-1".to_string()),
            subjob: Some(SubJobTask {
                id: Some("subjob-1".to_string()),
                ..tork::task::SubJobTask {
                id: None,
                name: None,
                description: None,
                tasks: None,
                inputs: None,
                secrets: None,
                auto_delete: None,
                output: None,
                webhooks: None,
                detached: false,
            }
            }),
            ..Task::default()
        };
        assert_eq!(
            calculate_task_cancel_action(&task),
            TaskCancelAction::CancelSubJob {
                sub_job_id: "subjob-1".to_string()
            }
        );
    }

    #[test]
    fn test_calculate_task_cancel_action_node() {
        let task = Task {
            id: Some("task-1".to_string()),
            node_id: Some("node-1".to_string()),
            ..Task::default()
        };
        assert_eq!(
            calculate_task_cancel_action(&task),
            TaskCancelAction::NotifyNode {
                node_id: "node-1".to_string()
            }
        );
    }

    #[test]
    fn test_calculate_task_cancel_action_subjob_priority_over_node() {
        let task = Task {
            id: Some("task-1".to_string()),
            subjob: Some(SubJobTask {
                id: Some("subjob-1".to_string()),
                ..tork::task::SubJobTask {
                id: None,
                name: None,
                description: None,
                tasks: None,
                inputs: None,
                secrets: None,
                auto_delete: None,
                output: None,
                webhooks: None,
                detached: false,
            }
            }),
            node_id: Some("node-1".to_string()),
            ..Task::default()
        };
        assert_eq!(
            calculate_task_cancel_action(&task),
            TaskCancelAction::CancelSubJob {
                sub_job_id: "subjob-1".to_string()
            }
        );
    }

    #[test]
    fn test_calculate_task_cancel_action_noop() {
        let task = Task::default();
        assert_eq!(calculate_task_cancel_action(&task), TaskCancelAction::NoOp);
    }

    #[test]
    fn test_calculate_task_cancel_action_empty_subjob_id() {
        let task = Task {
            id: Some("task-1".to_string()),
            subjob: Some(SubJobTask {
                id: Some(String::new()),
                ..tork::task::SubJobTask {
                id: None,
                name: None,
                description: None,
                tasks: None,
                inputs: None,
                secrets: None,
                auto_delete: None,
                output: None,
                webhooks: None,
                detached: false,
            }
            }),
            ..Task::default()
        };
        assert_eq!(calculate_task_cancel_action(&task), TaskCancelAction::NoOp);
    }

    #[test]
    fn test_calculate_task_cancel_action_none_subjob_id() {
        let task = Task {
            id: Some("task-1".to_string()),
            subjob: Some(SubJobTask {
                id: None,
                ..tork::task::SubJobTask {
                id: None,
                name: None,
                description: None,
                tasks: None,
                inputs: None,
                secrets: None,
                auto_delete: None,
                output: None,
                webhooks: None,
                detached: false,
            }
            }),
            ..Task::default()
        };
        assert_eq!(calculate_task_cancel_action(&task), TaskCancelAction::NoOp);
    }

    #[test]
    fn test_calculate_task_cancel_action_empty_node_id() {
        let task = Task {
            id: Some("task-1".to_string()),
            node_id: Some(String::new()),
            ..Task::default()
        };
        assert_eq!(calculate_task_cancel_action(&task), TaskCancelAction::NoOp);
    }

    #[test]
    fn test_debug_impl() {
        let debug_str = format!("{:?}", CancelEligibility::Eligible);
        assert!(debug_str.contains("Eligible"));
        let debug_str = format!("{:?}", CancelEligibility::NoOp);
        assert!(debug_str.contains("NoOp"));
    }

    #[test]
    fn test_task_cancel_action_debug() {
        let action = TaskCancelAction::CancelSubJob {
            sub_job_id: "sj-1".to_string(),
        };
        let debug_str = format!("{action:?}");
        assert!(debug_str.contains("CancelSubJob"));
        assert!(debug_str.contains("sj-1"));

        let action = TaskCancelAction::NotifyNode {
            node_id: "n-1".to_string(),
        };
        let debug_str = format!("{action:?}");
        assert!(debug_str.contains("NotifyNode"));

        let debug_str = format!("{:?}", TaskCancelAction::NoOp);
        assert!(debug_str.contains("NoOp"));
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: CancelHandler construction tests
    #[test]
    fn test_cancel_handler_debug() {
        let handler = CancelHandler::new(
            std::sync::Arc::new(MockDs),
            std::sync::Arc::new(MockBrk),
        );
        let debug_str = format!("{handler:?}");
        assert!(debug_str.contains("CancelHandler"));
    }

    // Go: calculate_cancel_eligibility with unknown states
    #[test]
    fn test_calculate_cancel_eligibility_random_string() {
        assert_eq!(calculate_cancel_eligibility("UNKNOWN"), CancelEligibility::NoOp);
    }

    // Go: prepare_cancelled_job preserves all fields
    #[test]
    fn test_prepare_cancelled_job_preserves_position() {
        let job = Job {
            id: Some("j1".into()),
            state: JOB_STATE_RUNNING.to_string(),
            position: 5,
            task_count: 10,
            progress: 50.0,
            ..Job::default()
        };
        let cancelled = prepare_cancelled_job(&job);
        assert_eq!(cancelled.position, 5);
        assert_eq!(cancelled.task_count, 10);
        assert_eq!(cancelled.progress, 50.0);
    }

    // Go: prepare_cancelled_task preserves error info
    #[test]
    fn test_prepare_cancelled_task_preserves_error() {
        let task = Task {
            id: Some("t1".into()),
            state: tork::task::TASK_STATE_FAILED.clone(),
            error: Some("oom killed".into()),
            ..Task::default()
        };
        let cancelled = prepare_cancelled_task(&task);
        assert_eq!(cancelled.error.as_deref(), Some("oom killed"));
    }

    // Go: calculate_task_cancel_action with node_id only (no subjob)
    #[test]
    fn test_calculate_task_cancel_action_node_only() {
        let task = Task {
            id: Some("t1".into()),
            node_id: Some("node-42".into()),
            ..Task::default()
        };
        assert_eq!(
            calculate_task_cancel_action(&task),
            TaskCancelAction::NotifyNode { node_id: "node-42".to_string() }
        );
    }

    // Go: TaskCancelAction equality
    #[test]
    fn test_task_cancel_action_equality() {
        let a1 = TaskCancelAction::CancelSubJob { sub_job_id: "sj1".to_string() };
        let a2 = TaskCancelAction::CancelSubJob { sub_job_id: "sj1".to_string() };
        assert_eq!(a1, a2);

        let n1 = TaskCancelAction::NotifyNode { node_id: "n1".to_string() };
        let n2 = TaskCancelAction::NotifyNode { node_id: "n1".to_string() };
        assert_eq!(n1, n2);

        assert_ne!(a1, n1);
    }

    // -- Mock implementations -----------------------------------------------

    struct MockDs;

    impl tork::Datastore for MockDs {
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
        fn get_next_task(&self, _parent_task_id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            Box::pin(async { Ok(None) })
        }
        fn create_task_log_part(&self, _part: tork::task::TaskLogPart) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_task_log_parts(
            &self, _task_id: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
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
            &self, _job_id: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::task::TaskLogPart>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn get_jobs(
            &self, _current_user: String, _q: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::JobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn create_scheduled_job(&self, _job: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(&self) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn get_scheduled_jobs(
            &self, _current_user: String, _page: i64, _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>> {
            Box::pin(async { Ok(tork::datastore::Page { items: vec![], total: 0, page: 0, size: 0 }) })
        }
        fn get_scheduled_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(&self, _id: String, _job: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn delete_scheduled_job(&self, _id: String) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn create_user(&self, _user: tork::user::User) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_user(&self, _username: String) -> tork::datastore::BoxedFuture<Option<tork::user::User>> {
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

    struct MockBrk;

    impl tork::Broker for MockBrk {
        fn publish_task(&self, _qname: String, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(&self, _qname: String, _handler: tork::broker::TaskHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(&self, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(&self, _handler: tork::broker::TaskProgressHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(&self, _node: tork::node::Node) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_heartbeats(&self, _handler: tork::broker::HeartbeatHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(&self, _job: &tork::job::Job) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(&self, _handler: tork::broker::JobHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(&self, _topic: String, _event: serde_json::Value) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(&self, _pattern: String, _handler: tork::broker::EventHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_log_part(&self, _part: &tork::task::TaskLogPart) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(&self, _handler: tork::broker::TaskLogPartHandler) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn queues(&self) -> tork::broker::BoxedFuture<Vec<tork::broker::QueueInfo>> {
            Box::pin(async { Ok(vec![]) })
        }
        fn queue_info(&self, _qname: String) -> tork::broker::BoxedFuture<tork::broker::QueueInfo> {
            Box::pin(async { Ok(tork::broker::QueueInfo { name: String::new(), size: 0, subscribers: 0, unacked: 0 }) })
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

    use crate::handlers::test_helpers::{new_uuid, TestEnv};

    /// Go parity: Test_cancelActiveTasks
    #[tokio::test]
    #[ignore]
    async fn test_cancel_active_tasks_integration() {
        let env = TestEnv::new().await;
        let handler = CancelHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());

        let job_id = new_uuid();
        let job = Job {
            id: Some(job_id.clone()),
            state: JOB_STATE_RUNNING.to_string(),
            ..Job::default()
        };
        env.ds.create_job(job).await.expect("create job");

        let node_id = new_uuid();
        let queue_name = new_uuid();
        let n1 = tork::node::Node {
            id: Some(node_id.clone()),
            queue: Some(queue_name.clone()),
            started_at: time::OffsetDateTime::now_utc(),
            last_heartbeat_at: time::OffsetDateTime::now_utc(),
            cpu_percent: 0.0,
            status: tork::node::NODE_STATUS_UP.to_string(),
            hostname: Some("localhost".into()),
            port: 8080,
            task_count: 0,
            name: None,
            version: String::new(),
        };
        env.ds.create_node(n1).await.expect("create node");

        let t1 = Task {
            id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            state: TASK_STATE_RUNNING.clone(),
            node_id: Some(node_id.clone()),
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task");

        let cancel_job = Job {
            id: Some(job_id.clone()),
            state: JOB_STATE_RUNNING.to_string(),
            ..Job::default()
        };
        handler.handle(&cancel_job).await.expect("handle cancel");

        let cancelled_task = env.ds.get_task_by_id(t1.id.clone().unwrap()).await.expect("get task").expect("task exists");
        assert_eq!(cancelled_task.state, *TASK_STATE_CANCELLED);

        env.cleanup().await;
    }
}
