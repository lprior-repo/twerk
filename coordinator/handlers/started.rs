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

use crate::handlers::HandlerError;

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
}

impl std::fmt::Debug for StartedHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartedHandler").finish()
    }
}

impl StartedHandler {
    /// Create a new started handler with datastore and broker dependencies.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self { ds, broker }
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
                let node_id = task
                    .node_id
                    .as_deref()
                    .ok_or_else(|| {
                        HandlerError::Validation("node ID required for cancellation".into())
                    })?;
                let node = self
                    .ds
                    .get_node_by_id(node_id.to_string())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("node {node_id} not found"))
                    })?;
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
                if needs_job_transition(&job.state) {
                    let updated_job = tork::Job {
                        state: JOB_STATE_RUNNING.to_string(),
                        ..job.clone()
                    };
                    self.ds
                        .update_job(job_id.to_string(), updated_job)
                        .await
                        .map_err(|e| HandlerError::Datastore(e.to_string()))?;
                }

                // 3. Update task state and node assignment
                let current = self
                    .ds
                    .get_task_by_id(task_id.to_string())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("task {task_id} not found"))
                    })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tork::Job;

    #[test]
    fn test_calculate_started_action_running() {
        assert_eq!(
            calculate_started_action(JOB_STATE_RUNNING),
            StartedAction::Proceed
        );
    }

    #[test]
    fn test_calculate_started_action_scheduled() {
        assert_eq!(
            calculate_started_action(JOB_STATE_SCHEDULED),
            StartedAction::Proceed
        );
    }

    #[test]
    fn test_calculate_started_action_failed() {
        assert_eq!(
            calculate_started_action(tork::job::JOB_STATE_FAILED),
            StartedAction::Cancel
        );
    }

    #[test]
    fn test_calculate_started_action_cancelled() {
        assert_eq!(
            calculate_started_action(tork::job::JOB_STATE_CANCELLED),
            StartedAction::Cancel
        );
    }

    #[test]
    fn test_calculate_started_action_completed() {
        assert_eq!(
            calculate_started_action(tork::job::JOB_STATE_COMPLETED),
            StartedAction::Cancel
        );
    }

    #[test]
    fn test_needs_job_transition_scheduled() {
        assert!(needs_job_transition(JOB_STATE_SCHEDULED));
    }

    #[test]
    fn test_needs_job_transition_running() {
        assert!(!needs_job_transition(JOB_STATE_RUNNING));
    }

    #[test]
    fn test_needs_job_transition_failed() {
        assert!(!needs_job_transition(tork::job::JOB_STATE_FAILED));
    }

    #[test]
    fn test_prepare_task_start_update_from_scheduled() {
        let current = Task {
            state: TASK_STATE_SCHEDULED.clone(),
            node_id: Some("old-node".to_string()),
            ..Task::default()
        };
        let incoming = Task {
            node_id: Some("new-node".to_string()),
            ..Task::default()
        };
        let updated = prepare_task_start_update(&current, &incoming);
        assert_eq!(updated.state, *TASK_STATE_RUNNING);
        assert!(updated.started_at.is_some());
        assert_eq!(updated.node_id.as_deref(), Some("new-node"));
    }

    #[test]
    fn test_prepare_task_start_update_from_running_preserves() {
        let now = time::OffsetDateTime::now_utc();
        let current = Task {
            state: TASK_STATE_RUNNING.clone(),
            started_at: Some(now),
            node_id: Some("old-node".to_string()),
            ..Task::default()
        };
        let incoming = Task {
            node_id: Some("new-node".to_string()),
            ..Task::default()
        };
        let updated = prepare_task_start_update(&current, &incoming);
        assert_eq!(updated.state, *TASK_STATE_RUNNING);
        assert!(updated.started_at.is_some());
        assert_eq!(updated.node_id.as_deref(), Some("new-node"));
    }

    #[test]
    fn test_prepare_task_start_update_from_completed_no_change() {
        let current = Task {
            state: TASK_STATE_CANCELLED.clone(),
            node_id: Some("old-node".to_string()),
            ..Task::default()
        };
        let incoming = Task {
            node_id: Some("new-node".to_string()),
            ..Task::default()
        };
        let updated = prepare_task_start_update(&current, &incoming);
        assert_eq!(updated.state, *TASK_STATE_CANCELLED);
        assert!(updated.started_at.is_none());
        assert_eq!(updated.node_id.as_deref(), Some("new-node"));
    }

    #[test]
    fn test_prepare_cancelled_task() {
        let task = Task {
            id: Some("task-1".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            ..Task::default()
        };
        let cancelled = prepare_cancelled_task(&task);
        assert_eq!(cancelled.id.as_deref(), Some("task-1"));
        assert_eq!(cancelled.state, *TASK_STATE_CANCELLED);
    }

    #[test]
    fn test_prepare_cancelled_task_preserves_other_fields() {
        let task = Task {
            id: Some("task-1".to_string()),
            job_id: Some("job-1".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            position: 3,
            ..Task::default()
        };
        let cancelled = prepare_cancelled_task(&task);
        assert_eq!(cancelled.job_id.as_deref(), Some("job-1"));
        assert_eq!(cancelled.position, 3);
    }

    #[test]
    fn test_debug_impl() {
        let debug_str = format!("{:?}", StartedAction::Proceed);
        assert!(debug_str.contains("Proceed"));
        let debug_str = format!("{:?}", StartedAction::Cancel);
        assert!(debug_str.contains("Cancel"));
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: Test_handleStartedTaskOfFailedJob — verifies cancel on FAILED job
    #[test]
    fn test_calculate_started_action_pending() {
        assert_eq!(
            calculate_started_action(tork::job::JOB_STATE_PENDING),
            StartedAction::Cancel
        );
    }

    // Go: job state transitions through SCHEDULED → RUNNING
    #[test]
    fn test_needs_job_transition_completed() {
        assert!(!needs_job_transition(tork::job::JOB_STATE_COMPLETED));
    }

    // -- prepare_task_start_update preserves all fields ---------------------

    #[test]
    fn test_prepare_task_start_update_preserves_job_id() {
        let current = Task {
            state: TASK_STATE_SCHEDULED.clone(),
            job_id: Some("job-42".into()),
            node_id: Some("old-node".into()),
            ..Task::default()
        };
        let incoming = Task {
            node_id: Some("new-node".into()),
            ..Task::default()
        };
        let updated = prepare_task_start_update(&current, &incoming);
        assert_eq!(updated.job_id.as_deref(), Some("job-42"));
    }

    #[test]
    fn test_prepare_task_start_update_preserves_position() {
        let current = Task {
            state: TASK_STATE_SCHEDULED.clone(),
            position: 5,
            node_id: Some("n1".into()),
            ..Task::default()
        };
        let incoming = Task { node_id: Some("n2".into()), ..Task::default() };
        let updated = prepare_task_start_update(&current, &incoming);
        assert_eq!(updated.position, 5);
    }

    #[test]
    fn test_prepare_task_start_update_preserves_id() {
        let current = Task {
            state: TASK_STATE_SCHEDULED.clone(),
            id: Some("task-abc".into()),
            node_id: Some("n1".into()),
            ..Task::default()
        };
        let incoming = Task { node_id: Some("n2".into()), ..Task::default() };
        let updated = prepare_task_start_update(&current, &incoming);
        assert_eq!(updated.id.as_deref(), Some("task-abc"));
    }

    // -- StartedAction exhaustive match test ---------------------------------

    #[test]
    fn test_started_action_all_variants() {
        let all_actions = [StartedAction::Proceed, StartedAction::Cancel];
        for action in &all_actions {
            let _ = format!("{action:?}"); // verify Debug impl
        }
    }

    // -- Handler construction tests ------------------------------------------

    #[test]
    fn test_started_handler_debug() {
        let handler = StartedHandler::new(
            std::sync::Arc::new(MockDs),
            std::sync::Arc::new(MockBrk),
        );
        let debug_str = format!("{handler:?}");
        assert!(debug_str.contains("StartedHandler"));
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
        fn create_job(&self, _job: tork::job::Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn update_job(&self, _id: String, _job: tork::job::Job) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::job::Job>> {
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

    /// Go parity: Test_handleStartedTask
    #[tokio::test]
    #[ignore]
    async fn test_handle_started_task_integration() {
        let env = TestEnv::new().await;
        let handler = StartedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_SCHEDULED.to_string(),
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let t1 = Task {
            id: Some(new_uuid()),
            state: TASK_STATE_SCHEDULED.clone(),
            started_at: Some(now),
            node_id: Some(new_uuid()),
            job_id: Some(job_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task");

        handler.handle(&t1).await.expect("handle started");

        let t2 = env.ds.get_task_by_id(t1.id.clone().unwrap()).await.expect("get task").expect("task exists");
        assert_eq!(t2.state, *TASK_STATE_RUNNING);
        assert_eq!(t2.node_id, t1.node_id);

        let j2 = env.ds.get_job_by_id(job_id).await.expect("get job").expect("job exists");
        assert_eq!(j2.state, *JOB_STATE_RUNNING);

        env.cleanup().await;
    }

    /// Go parity: Test_handleStartedTaskOfFailedJob
    #[tokio::test]
    #[ignore]
    async fn test_handle_started_task_of_failed_job_integration() {
        let env = TestEnv::new().await;
        let handler = StartedHandler::new(env.ds.clone() as Arc<dyn tork::Datastore>, env.broker.clone());
        let now = time::OffsetDateTime::now_utc();

        let job_id = new_uuid();
        let j1 = Job {
            id: Some(job_id.clone()),
            state: tork::job::JOB_STATE_FAILED.to_string(),
            ..Job::default()
        };
        env.ds.create_job(j1).await.expect("create job");

        let node_id = new_uuid();
        let queue_name = new_uuid();
        let n1 = tork::node::Node {
            id: Some(node_id.clone()),
            queue: Some(queue_name.clone()),
            started_at: now,
            last_heartbeat_at: now,
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
            state: TASK_STATE_SCHEDULED.clone(),
            started_at: Some(now),
            job_id: Some(job_id.clone()),
            node_id: Some(node_id.clone()),
            created_at: Some(now),
            ..Task::default()
        };
        env.ds.create_task(t1.clone()).await.expect("create task");

        handler.handle(&t1).await.expect("handle started");

        // Allow time for broker message
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let t2 = env.ds.get_task_by_id(t1.id.clone().unwrap()).await.expect("get task").expect("task exists");
        // Task should NOT have been updated to RUNNING since job is FAILED
        assert_eq!(t2.state, *TASK_STATE_SCHEDULED);

        env.cleanup().await;
    }
}
