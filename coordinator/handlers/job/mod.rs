//! Job handler for job state change events.
//!
//! Ported from Go `internal/coordinator/handlers/job.go`.
//! Manages the full job lifecycle: start, complete, fail, restart, cancel.

pub mod calc;
pub mod eval;

use std::sync::Arc;

use time::OffsetDateTime;
use tracing::{debug, error};

use tork::broker::queue::{QUEUE_COMPLETED, QUEUE_ERROR, QUEUE_PENDING};
use tork::broker::Broker;
use tork::datastore::Datastore;
use tork::job::{
    Job, JOB_STATE_CANCELLED, JOB_STATE_COMPLETED, JOB_STATE_FAILED, JOB_STATE_PENDING,
    JOB_STATE_RESTART, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED,
};
use tork::task::{TASK_STATE_CANCELLED, TASK_STATE_FAILED, TASK_STATE_PENDING};

use crate::handlers::{HandlerError, JobEventType};

// Re-export from submodules
pub use calc::JobStateTransition;
pub use eval::{evaluate_task, evaluate_template, parse_duration};

// Topic constants matching Go broker package
const TOPIC_JOB_COMPLETED: &str = "job.completed";
const TOPIC_JOB_FAILED: &str = "job.failed";

// ---------------------------------------------------------------------------
// JobHandler
// ---------------------------------------------------------------------------

/// Job handler for processing job state change events.
///
/// Holds datastore and broker references for I/O operations.
/// Ported from Go `internal/coordinator/handlers/job.go`.
#[derive(Clone)]
pub struct JobHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
}

impl std::fmt::Debug for JobHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandler").finish_non_exhaustive()
    }
}

impl JobHandler {
    /// Create a new job handler with datastore and broker dependencies.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self { ds, broker }
    }

    /// Handle a job event, dispatching to the appropriate sub-handler.
    ///
    /// Only `StateChange` events are processed; all others are silently ignored.
    /// Parity with Go `func (h *jobHandler) handle(...)`.
    pub fn handle<'a>(
        &'a self,
        et: JobEventType,
        job: &'a mut Job,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), HandlerError>> + Send + 'a>>
    {
        Box::pin(async move {
            if et != JobEventType::StateChange {
                return Ok(());
            }
            match job.state.as_str() {
                s if s == JOB_STATE_PENDING => self.start_job(job).await,
                s if s == JOB_STATE_CANCELLED => self.cancel_job(job).await,
                s if s == JOB_STATE_RESTART => self.restart_job(job).await,
                s if s == JOB_STATE_COMPLETED => self.complete_job(job).await,
                s if s == JOB_STATE_FAILED => self.fail_job(job).await,
                s if s == JOB_STATE_RUNNING => self.mark_job_as_running(job).await,
                other => Err(HandlerError::InvalidState(format!(
                    "invalid job state: {other}"
                ))),
            }
        })
    }

    /// Start a pending job: create first task, evaluate it, persist, transition to SCHEDULED.
    async fn start_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        debug!("starting job {}", job.id.as_deref().unwrap_or("(no id)"));
        let now = OffsetDateTime::now_utc();
        let job_id = job.id.clone().unwrap_or_default();

        if job.tasks.is_empty() {
            return Err(HandlerError::Handler("job has no tasks".to_string()));
        }

        let ctx_map = job.context.as_map();
        let base_task = &job.tasks[0];
        let mut task = match evaluate_task(base_task, &ctx_map) {
            Ok(t) => t,
            Err(eval_err) => {
                let mut failed = base_task.clone();
                failed.error = Some(eval_err);
                failed.state = TASK_STATE_FAILED.clone();
                failed.failed_at = Some(now);
                failed
            }
        };
        task.id = Some(uuid::Uuid::new_v4().to_string());
        task.job_id = Some(job_id.clone());
        task.position = 1;
        task.created_at = Some(now);
        if task.state != *TASK_STATE_FAILED {
            task.state = TASK_STATE_PENDING.clone();
        }

        let task_clone = task.clone();
        self.ds
            .create_task(task_clone)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let mut updated_job = self
            .ds
            .get_job_by_id(job_id.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job not found: {job_id}")))?;
        updated_job.state = JOB_STATE_SCHEDULED.to_string();
        updated_job.started_at = Some(OffsetDateTime::now_utc());
        updated_job.position = 1;
        let updated_job_id = updated_job.id.clone().unwrap_or_default();
        self.ds
            .update_job(updated_job_id, updated_job)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if task.state == *TASK_STATE_FAILED {
            let now = OffsetDateTime::now_utc();
            job.failed_at = Some(now);
            job.state = JOB_STATE_FAILED.to_string();
            job.error = task.error.clone();
            return self.handle(JobEventType::StateChange, job).await;
        }

        self.broker
            .publish_task(QUEUE_PENDING.to_string(), &task)
            .await
            .map_err(|e| HandlerError::Broker(e.to_string()))?;

        Ok(())
    }

    /// Complete a job: evaluate output, handle auto-delete, handle parent task.
    async fn complete_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        let current = self
            .ds
            .get_job_by_id(job_id.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job not found: {job_id}")))?;

        if current.state != JOB_STATE_RUNNING && current.state != JOB_STATE_SCHEDULED {
            return Err(HandlerError::InvalidState(format!(
                "job {job_id} is {} and can not be completed",
                current.state
            )));
        }

        let now = OffsetDateTime::now_utc();
        let ctx_map = job.context.as_map();

        let (new_state, result, err_msg, delete_at) = match &job.output {
            None => (JOB_STATE_COMPLETED.to_string(), None, None, None),
            Some(output) => match evaluate_template(output, &ctx_map) {
                Ok(evaluated) => {
                    let delete_at = job.auto_delete.as_ref().and_then(|ad| {
                        ad.after.as_ref().and_then(|after| {
                            match parse_duration(after) {
                                Ok(dur) => Some(OffsetDateTime::now_utc() + dur),
                                Err(e) => {
                                    error!(error = %e, duration = %after, "unable to parse auto delete duration");
                                    None
                                }
                            }
                        })
                    });
                    (
                        JOB_STATE_COMPLETED.to_string(),
                        Some(evaluated),
                        None,
                        delete_at,
                    )
                }
                Err(eval_err) => {
                    error!(error = %eval_err, job_id = %job_id, "error evaluating job output");
                    (JOB_STATE_FAILED.to_string(), None, Some(eval_err), None)
                }
            },
        };

        job.state = new_state.clone();
        job.result = result.clone();
        job.error = err_msg.clone();

        let mut updated = current;
        updated.state = new_state.clone();
        if new_state == JOB_STATE_COMPLETED {
            updated.completed_at = Some(now);
            updated.result = result.clone();
            updated.delete_at = delete_at;
        } else {
            updated.failed_at = Some(now);
            updated.error = err_msg.clone();
        }

        let upd_id = updated.id.clone().unwrap_or_default();
        self.ds
            .update_job(upd_id, updated)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let Some(ref parent_id) = job.parent_id {
            if !parent_id.is_empty() {
                let parent = self
                    .ds
                    .get_task_by_id(parent_id.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("parent task not found: {parent_id}"))
                    })?;

                let mut updated_parent = parent.clone();
                if new_state == JOB_STATE_FAILED {
                    updated_parent.state = TASK_STATE_FAILED.clone();
                    updated_parent.failed_at = Some(now);
                    updated_parent.error = err_msg.clone();
                } else {
                    updated_parent.state = tork::task::TASK_STATE_COMPLETED.clone();
                    updated_parent.completed_at = Some(now);
                    updated_parent.result = result.clone();
                }

                let queue = if new_state == JOB_STATE_FAILED {
                    QUEUE_ERROR
                } else {
                    QUEUE_COMPLETED
                };
                return self
                    .broker
                    .publish_task(queue.to_string(), &updated_parent)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()));
            }
        }

        if new_state == JOB_STATE_FAILED {
            let event =
                serde_json::to_value(job).map_err(|e| HandlerError::Handler(e.to_string()))?;
            self.broker
                .publish_event(TOPIC_JOB_FAILED.to_string(), event)
                .await
                .map_err(|e| HandlerError::Broker(e.to_string()))
        } else {
            let event =
                serde_json::to_value(job).map_err(|e| HandlerError::Handler(e.to_string()))?;
            self.broker
                .publish_event(TOPIC_JOB_COMPLETED.to_string(), event)
                .await
                .map_err(|e| HandlerError::Broker(e.to_string()))
        }
    }

    /// Mark a job as RUNNING (transition from SCHEDULED).
    async fn mark_job_as_running(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        let current = self
            .ds
            .get_job_by_id(job_id.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let Some(mut current) = current else {
            return Ok(());
        };

        if current.state != JOB_STATE_SCHEDULED {
            return Ok(());
        }

        current.state = JOB_STATE_RUNNING.to_string();
        current.failed_at = None;
        job.state = current.state.clone();

        let upd_id = current.id.clone().unwrap_or_default();
        self.ds
            .update_job(upd_id, current)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))
    }

    /// Restart a failed or cancelled job: create new task at position 0.
    async fn restart_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        let current = self
            .ds
            .get_job_by_id(job_id.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job not found: {job_id}")))?;

        if current.state != JOB_STATE_FAILED && current.state != JOB_STATE_CANCELLED {
            return Err(HandlerError::InvalidState(format!(
                "job {job_id} is in {} state and can't be restarted",
                current.state
            )));
        }

        let mut updated = current;
        updated.state = JOB_STATE_RUNNING.to_string();
        updated.failed_at = None;
        let upd_id = updated.id.clone().unwrap_or_default();
        self.ds
            .update_job(upd_id, updated)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let now = OffsetDateTime::now_utc();
        let position = job.position.max(1) as usize;
        let task_index = position.saturating_sub(1);

        if task_index >= job.tasks.len() {
            return Err(HandlerError::Handler(format!(
                "task index {task_index} out of range (job has {} tasks)",
                job.tasks.len()
            )));
        }

        let ctx_map = job.context.as_map();
        let base_task = &job.tasks[task_index];
        let mut task = match evaluate_task(base_task, &ctx_map) {
            Ok(t) => t,
            Err(eval_err) => {
                let mut failed = base_task.clone();
                failed.error = Some(eval_err);
                failed.state = TASK_STATE_FAILED.clone();
                failed.failed_at = Some(now);
                failed
            }
        };
        task.id = Some(uuid::Uuid::new_v4().to_string());
        task.job_id = Some(job_id);
        task.state = TASK_STATE_PENDING.clone();
        task.position = job.position;
        task.created_at = Some(now);

        self.ds
            .create_task(task.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        self.broker
            .publish_task(QUEUE_PENDING.to_string(), &task)
            .await
            .map_err(|e| HandlerError::Broker(e.to_string()))
    }

    /// Fail a job: mark as FAILED, handle parent, cancel active tasks.
    async fn fail_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        debug!(job_id = ?job.id, error = ?job.error, "job failed");
        let job_id = job.id.clone().unwrap_or_default();

        let current = self
            .ds
            .get_job_by_id(job_id.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let Some(mut current) = current {
            if current.state == JOB_STATE_RUNNING || current.state == JOB_STATE_SCHEDULED {
                current.state = JOB_STATE_FAILED.to_string();
                current.failed_at = job.failed_at;
                let upd_id = current.id.clone().unwrap_or_default();
                self.ds
                    .update_job(upd_id, current)
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?;
            }
        }

        if let Some(ref parent_id) = job.parent_id {
            if !parent_id.is_empty() {
                let parent = self
                    .ds
                    .get_task_by_id(parent_id.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("parent task not found: {parent_id}"))
                    })?;

                let mut updated_parent = parent;
                updated_parent.state = TASK_STATE_FAILED.clone();
                updated_parent.failed_at = job.failed_at;
                updated_parent.error = job.error.clone();

                return self
                    .broker
                    .publish_task(QUEUE_ERROR.to_string(), &updated_parent)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()));
            }
        }

        self.cancel_active_tasks(&job_id).await?;

        let refreshed = self
            .ds
            .get_job_by_id(job_id)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let Some(refreshed) = refreshed {
            if refreshed.state == JOB_STATE_FAILED {
                job.state = refreshed.state.clone();
                job.error = refreshed.error.clone();
                let event = serde_json::to_value(&refreshed)
                    .map_err(|e| HandlerError::Handler(e.to_string()))?;
                self.broker
                    .publish_event(TOPIC_JOB_FAILED.to_string(), event)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Cancel a job: mark as CANCELLED, notify parent, cancel active tasks.
    async fn cancel_job(&self, job: &mut Job) -> Result<(), HandlerError> {
        let job_id = job.id.clone().unwrap_or_default();

        let current = self
            .ds
            .get_job_by_id(job_id.clone())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let Some(mut current) = current else {
            return Ok(());
        };

        if current.state != JOB_STATE_RUNNING && current.state != JOB_STATE_SCHEDULED {
            return Ok(());
        }

        current.state = JOB_STATE_CANCELLED.to_string();
        job.state = current.state.clone();
        let upd_id = current.id.clone().unwrap_or_default();
        self.ds
            .update_job(upd_id, current)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let Some(ref parent_id) = job.parent_id {
            if !parent_id.is_empty() {
                let parent_task = self
                    .ds
                    .get_task_by_id(parent_id.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("parent task not found: {parent_id}"))
                    })?;

                let parent_job_id = parent_task.job_id.clone().ok_or_else(|| {
                    HandlerError::Handler("parent task has no job_id".to_string())
                })?;

                let mut parent_job = self
                    .ds
                    .get_job_by_id(parent_job_id.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?
                    .ok_or_else(|| {
                        HandlerError::NotFound(format!("parent job not found: {parent_job_id}"))
                    })?;

                parent_job.state = JOB_STATE_CANCELLED.to_string();
                self.broker
                    .publish_job(&parent_job)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
        }

        self.cancel_active_tasks(&job_id).await
    }

    /// Cancel all currently active tasks for a job.
    async fn cancel_active_tasks(&self, job_id: &str) -> Result<(), HandlerError> {
        let tasks = self
            .ds
            .get_active_tasks(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        for mut task in tasks {
            task.state = TASK_STATE_CANCELLED.clone();
            let task_id = task.id.clone().unwrap_or_default();
            let task_clone = task.clone();
            self.ds
                .update_task(task_id, task_clone)
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?;

            if let Some(ref sj) = task.subjob {
                if let Some(ref sj_id) = sj.id {
                    if !sj_id.is_empty() {
                        let subjob = self
                            .ds
                            .get_job_by_id(sj_id.clone())
                            .await
                            .map_err(|e| HandlerError::Datastore(e.to_string()))?
                            .ok_or_else(|| {
                                HandlerError::NotFound(format!("sub-job not found: {sj_id}"))
                            })?;

                        let mut cancelled_subjob = subjob;
                        cancelled_subjob.state = JOB_STATE_CANCELLED.to_string();
                        self.broker
                            .publish_job(&cancelled_subjob)
                            .await
                            .map_err(|e| HandlerError::Broker(e.to_string()))?;
                    }
                }
            } else if let Some(ref node_id) = task.node_id {
                if !node_id.is_empty() {
                    let node = self
                        .ds
                        .get_node_by_id(node_id.clone())
                        .await
                        .map_err(|e| HandlerError::Datastore(e.to_string()))?
                        .ok_or_else(|| {
                            HandlerError::NotFound(format!("node not found: {node_id}"))
                        })?;

                    let queue = node.queue.clone().unwrap_or_default();
                    if !queue.is_empty() {
                        self.broker
                            .publish_task(queue, &task)
                            .await
                            .map_err(|e| HandlerError::Broker(e.to_string()))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Process a job state transition based on the current state.
    pub fn process_state_transition(&self, job: &Job) -> Result<JobStateTransition, HandlerError> {
        let transition = match job.state.as_str() {
            s if s == JOB_STATE_PENDING => JobStateTransition::Start,
            s if s == JOB_STATE_SCHEDULED => JobStateTransition::Schedule,
            s if s == JOB_STATE_RUNNING => JobStateTransition::Run,
            s if s == JOB_STATE_CANCELLED => JobStateTransition::Cancel,
            s if s == JOB_STATE_COMPLETED => JobStateTransition::Complete,
            s if s == JOB_STATE_FAILED => JobStateTransition::Fail,
            s if s == JOB_STATE_RESTART => JobStateTransition::Restart,
            other => {
                return Err(HandlerError::InvalidState(format!(
                    "unknown job state: {other}"
                )));
            }
        };
        Ok(transition)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tork::task::Task;

    fn test_context() -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        map.insert("inputs".to_string(), serde_json::json!({"var1": "hello"}));
        map
    }

    fn new_test_handler() -> JobHandler {
        JobHandler::new(Arc::new(MockDatastore), Arc::new(MockBroker))
    }

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
            Box::pin(async { Ok(Vec::new()) })
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
        fn update_node(
            &self,
            _id: String,
            _node: tork::node::Node,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_node_by_id(
            &self,
            _id: String,
        ) -> tork::datastore::BoxedFuture<Option<tork::node::Node>> {
            Box::pin(async { Ok(None) })
        }
        fn get_active_nodes(&self) -> tork::datastore::BoxedFuture<Vec<tork::node::Node>> {
            Box::pin(async { Ok(Vec::new()) })
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
        fn create_scheduled_job(
            &self,
            _sj: tork::job::ScheduledJob,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(
            &self,
        ) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_scheduled_jobs(
            &self,
            _current_user: String,
            _page: i64,
            _size: i64,
        ) -> tork::datastore::BoxedFuture<tork::datastore::Page<tork::job::ScheduledJobSummary>>
        {
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
            _sj: tork::job::ScheduledJob,
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
            Box::pin(async { Ok(Vec::new()) })
        }
        fn get_user_roles(
            &self,
            _user_id: String,
        ) -> tork::datastore::BoxedFuture<Vec<tork::role::Role>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn assign_role(
            &self,
            _user_id: String,
            _role_id: String,
        ) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn unassign_role(
            &self,
            _user_id: String,
            _role_id: String,
        ) -> tork::datastore::BoxedFuture<()> {
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

    struct MockBroker;
    impl Broker for MockBroker {
        fn publish_task(&self, _qname: String, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_tasks(
            &self,
            _qname: String,
            _handler: tork::broker::TaskHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_progress(&self, _task: &Task) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_progress(
            &self,
            _handler: tork::broker::TaskProgressHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_heartbeat(&self, _node: tork::node::Node) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_heartbeats(
            &self,
            _handler: tork::broker::HeartbeatHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_job(&self, _job: &Job) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_jobs(
            &self,
            _handler: tork::broker::JobHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_event(
            &self,
            _topic: String,
            _event: serde_json::Value,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_events(
            &self,
            _pattern: String,
            _handler: tork::broker::EventHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn publish_task_log_part(
            &self,
            _part: &tork::task::TaskLogPart,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn subscribe_for_task_log_part(
            &self,
            _handler: tork::broker::TaskLogPartHandler,
        ) -> tork::broker::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn queues(&self) -> tork::broker::BoxedFuture<Vec<tork::broker::QueueInfo>> {
            Box::pin(async { Ok(Vec::new()) })
        }
        fn queue_info(&self, _qname: String) -> tork::broker::BoxedFuture<tork::broker::QueueInfo> {
            Box::pin(async {
                Ok(tork::broker::QueueInfo {
                    name: String::new(),
                    size: 0,
                    subscribers: 0,
                    unacked: 0,
                })
            })
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

    #[test]
    fn test_evaluate_template_no_expression() {
        let ctx = test_context();
        assert_eq!(evaluate_template("plain text", &ctx).unwrap(), "plain text");
    }

    #[test]
    fn test_evaluate_template_with_expression() {
        let ctx = test_context();
        assert_eq!(
            evaluate_template("{{ inputs.var1 }}", &ctx).unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_evaluate_template_empty() {
        let ctx = test_context();
        assert_eq!(evaluate_template("", &ctx).unwrap(), "");
    }

    #[test]
    fn test_evaluate_template_bad_expression() {
        let ctx = test_context();
        assert!(evaluate_template("{{ bad_expression }}", &ctx).is_err());
    }

    #[test]
    fn test_evaluate_task_with_bad_env() {
        let ctx = test_context();
        let mut task = Task::default();
        task.env = Some(HashMap::from([(
            "SOMEVAR".to_string(),
            "{{ bad_expression }}".to_string(),
        )]));
        assert!(evaluate_task(&task, &ctx).is_err());
    }

    #[test]
    fn test_evaluate_task_with_good_env() {
        let ctx = test_context();
        let mut task = Task::default();
        task.env = Some(HashMap::from([(
            "SOMEVAR".to_string(),
            "{{ inputs.var1 }}".to_string(),
        )]));
        let result = evaluate_task(&task, &ctx).unwrap();
        assert_eq!(
            result
                .env
                .as_ref()
                .and_then(|e| e.get("SOMEVAR"))
                .map(String::as_str),
            Some("hello")
        );
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            parse_duration("1m").unwrap(),
            std::time::Duration::from_secs(60)
        );
        assert_eq!(
            parse_duration("2h").unwrap(),
            std::time::Duration::from_secs(7200)
        );
        assert_eq!(
            parse_duration("30s").unwrap(),
            std::time::Duration::from_secs(30)
        );
        assert_eq!(
            parse_duration("1h30m").unwrap(),
            std::time::Duration::from_secs(5400)
        );
        assert!(parse_duration("invalid").is_err());
    }

    #[test]
    fn test_process_state_transition_pending() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_PENDING.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Start);
    }

    #[test]
    fn test_process_state_transition_completed() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_COMPLETED.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Complete);
    }

    #[test]
    fn test_process_state_transition_failed() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_FAILED.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Fail);
    }

    #[test]
    fn test_process_state_transition_restart() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = JOB_STATE_RESTART.to_string();
        let transition = handler.process_state_transition(&job).unwrap();
        assert_eq!(transition, JobStateTransition::Restart);
    }

    #[test]
    fn test_process_state_transition_unknown() {
        let handler = new_test_handler();
        let mut job = Job::default();
        job.state = "UNKNOWN".to_string();
        assert!(handler.process_state_transition(&job).is_err());
    }

    #[test]
    fn test_display_job_state_transition() {
        assert_eq!(JobStateTransition::Start.to_string(), "START");
        assert_eq!(JobStateTransition::Cancel.to_string(), "CANCEL");
        assert_eq!(JobStateTransition::Restart.to_string(), "RESTART");
    }
}
