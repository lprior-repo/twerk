//! Scheduler module for task scheduling.
//!
//! This module provides functionality for scheduling tasks based on
//! various task types:
//! - Regular tasks
//! - Parallel tasks
//! - Each (loop) tasks
//! - Sub-job tasks
//!
//! Go parity: `scheduler.go` — `ScheduleTask` dispatches to the correct
//! scheduling path, applies state + timestamp transitions, reads jobs from
//! datastore, applies job defaults, creates subtasks, and publishes to broker.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![allow(clippy::pedantic)]
#![warn(clippy::nursery)]

use std::collections::HashMap;
use std::sync::Arc;

use regex::Regex;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tork::broker::{queue, queue::QUEUE_ERROR, Broker};
use tork::datastore::Datastore;
use tork::job::JobDefaults;
use tork::task::{
    Task, TASK_STATE_CREATED, TASK_STATE_FAILED, TASK_STATE_PENDING, TASK_STATE_RUNNING,
    TASK_STATE_SCHEDULED,
};

/// Regex to match `{{ expr }}` template patterns.
#[allow(clippy::expect_used)]
static TEMPLATE_REGEX: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\{\{\s*(.+?)\s*\}\}").expect("invalid template regex")
});

// ---------------------------------------------------------------------------
// Pure calculations
// ---------------------------------------------------------------------------

/// Determines the scheduling path for a task based on its type fields.
///
/// This is a pure decision — no mutation, no I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledTaskType {
    /// A regular task
    Regular,
    /// A parallel task with subtasks
    Parallel,
    /// An each (loop) task with iterations
    Each,
    /// A sub-job task
    SubJob,
}

impl std::fmt::Display for ScheduledTaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduledTaskType::Regular => write!(f, "REGULAR"),
            ScheduledTaskType::Parallel => write!(f, "PARALLEL"),
            ScheduledTaskType::Each => write!(f, "EACH"),
            ScheduledTaskType::SubJob => write!(f, "SUBJOB"),
        }
    }
}

/// Classifies a task's scheduling type from its type fields (pure calc).
#[must_use]
pub fn classify_task_type(task: &Task) -> ScheduledTaskType {
    if task.each.is_some() {
        ScheduledTaskType::Each
    } else if task.parallel.is_some() {
        ScheduledTaskType::Parallel
    } else if task.subjob.is_some() {
        ScheduledTaskType::SubJob
    } else {
        ScheduledTaskType::Regular
    }
}

// ---------------------------------------------------------------------------
// State transitions (applied at the action boundary)
// ---------------------------------------------------------------------------

/// Applies the regular-task scheduling transition.
///
/// Sets state→SCHEDULED and scheduled_at→now.
/// Mirrors Go's `scheduleRegularTask`.
pub fn apply_regular_transition(task: &mut Task, now: OffsetDateTime) {
    task.state = TASK_STATE_SCHEDULED.clone();
    task.scheduled_at = Some(now);
}

/// Applies the parallel-task scheduling transition.
///
/// Sets state→RUNNING, scheduled_at→now, started_at→now.
/// Mirrors Go's `scheduleParallelTask`.
pub fn apply_parallel_transition(task: &mut Task, now: OffsetDateTime) {
    task.state = TASK_STATE_RUNNING.clone();
    task.scheduled_at = Some(now);
    task.started_at = Some(now);
}

/// Applies the each-task scheduling transition.
///
/// Sets state→RUNNING, scheduled_at→now, started_at→now.
/// Mirrors Go's `scheduleEachTask`.
pub fn apply_each_transition(task: &mut Task, now: OffsetDateTime) {
    task.state = TASK_STATE_RUNNING.clone();
    task.scheduled_at = Some(now);
    task.started_at = Some(now);
}

/// Applies the sub-job scheduling transition.
///
/// Sets state→RUNNING, scheduled_at→now, started_at→now.
/// Mirrors Go's `scheduleAttachedSubJob`.
pub fn apply_subjob_transition(task: &mut Task, now: OffsetDateTime) {
    task.state = TASK_STATE_RUNNING.clone();
    task.scheduled_at = Some(now);
    task.started_at = Some(now);
}

// ---------------------------------------------------------------------------
// Pure calculation: apply job defaults to task
// ---------------------------------------------------------------------------

/// Applies job defaults to a task if the task doesn't already have the field set.
///
/// This is a pure calculation — applies default values from job defaults to task.
/// Mirrors Go's `applyJobDefaults`.
pub fn apply_job_defaults(task: &mut Task, defaults: &JobDefaults) {
    // Apply queue default if task doesn't have one
    if task.queue.is_none() {
        task.queue = defaults.queue.clone();
    }

    // Apply timeout default if task doesn't have one
    if task.timeout.is_none() {
        task.timeout = defaults.timeout.clone();
    }

    // Apply priority default if task doesn't have one (0 means unset)
    if task.priority == 0 {
        task.priority = defaults.priority;
    }

    // Apply retry default if task doesn't have one
    if task.retry.is_none() {
        task.retry = defaults.retry.clone();
    }

    // Apply limits default if task doesn't have one
    if task.limits.is_none() {
        task.limits = defaults.limits.clone();
    }
}

// ---------------------------------------------------------------------------
// Scheduler (async I/O)
// ---------------------------------------------------------------------------

/// Scheduler for scheduling tasks.
///
/// Go parity: `Scheduler` in `scheduler.go`.
#[derive(Clone)]
pub struct Scheduler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler").finish()
    }
}

impl Scheduler {
    /// Create a new scheduler with datastore and broker references.
    ///
    /// Go parity: `NewScheduler(ds, broker)`.
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self { ds, broker }
    }

    /// Schedule a task based on its type.
    ///
    /// Go parity: `ScheduleTask` — classifies the task, reads job from datastore
    /// to apply defaults, applies state transitions, creates subtasks for
    /// parallel/each constructs, updates datastore, and publishes to broker.
    ///
    /// Note: Expression evaluation is handled by the caller since the eval
    /// crate is not available in the coordinator context. Use the
    /// `coordinator::handlers::completed::evaluate_task` function to
    /// evaluate expressions before calling this method.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] if any I/O operation fails.
    pub async fn schedule_task(
        &self,
        task: &mut Task,
    ) -> Result<ScheduledTaskType, SchedulerError> {
        let task_type = classify_task_type(task);
        let now = OffsetDateTime::now_utc();

        // 1. Read job from datastore to apply job defaults
        if let Some(job_id) = &task.job_id {
            if let Ok(Some(job)) = self.ds.get_job_by_id(job_id.clone()).await {
                if let Some(defaults) = &job.defaults {
                    apply_job_defaults(task, defaults);
                }
            }
        }

        // 2. Apply state transition based on task type
        match task_type {
            ScheduledTaskType::Each => apply_each_transition(task, now),
            ScheduledTaskType::Parallel => apply_parallel_transition(task, now),
            ScheduledTaskType::SubJob => apply_subjob_transition(task, now),
            ScheduledTaskType::Regular => apply_regular_transition(task, now),
        }

        // 3. Create subtasks for parallel/each constructs and publish
        match task_type {
            ScheduledTaskType::Parallel => {
                self.create_parallel_subtasks(task).await?;
            }
            ScheduledTaskType::Each => {
                self.create_each_subtasks(task).await?;
            }
            ScheduledTaskType::SubJob => {
                self.create_subjob_tasks(task).await?;
            }
            ScheduledTaskType::Regular => {
                // 4. Update task in datastore for regular tasks
                if let Some(task_id) = &task.id {
                    self.ds
                        .update_task(task_id.clone(), task.clone())
                        .await
                        .map_err(|e| SchedulerError::Datastore(e.to_string()))?;
                }

                // 5. Publish to broker
                let queue_name = task
                    .queue
                    .clone()
                    .unwrap_or_else(|| queue::QUEUE_DEFAULT.to_string());
                self.broker
                    .publish_task(queue_name, task)
                    .await
                    .map_err(|e| SchedulerError::Broker(e.to_string()))?;
            }
        }

        Ok(task_type)
    }

    /// Schedule a regular task directly.
    ///
    /// Go parity: `scheduleRegularTask` — sets state→SCHEDULED, scheduled_at→now,
    /// updates datastore, and publishes to broker.
    pub async fn schedule_regular_task(&self, task: &mut Task) -> Result<(), SchedulerError> {
        // =====================================================================
        // GAP 1 FIX: Apply job defaults like Go does
        // =====================================================================
        // Go scheduleRegularTask (lines 40-76):
        //   1. Fetches job from datastore
        //   2. Applies job.Defaults (queue, limits, timeout, retry, priority)
        //   3. Sets queue to QUEUE_DEFAULT if empty
        if let Some(job_id) = &task.job_id {
            if let Ok(Some(job)) = self.ds.get_job_by_id(job_id.clone()).await {
                if let Some(defaults) = &job.defaults {
                    apply_job_defaults(task, defaults);
                }
            }
        }
        if task.queue.is_none() {
            task.queue = Some(queue::QUEUE_DEFAULT.to_string());
        }

        let now = OffsetDateTime::now_utc();
        apply_regular_transition(task, now);

        // Update in datastore
        if let Some(task_id) = &task.id {
            self.ds
                .update_task(task_id.clone(), task.clone())
                .await
                .map_err(|e| SchedulerError::Datastore(e.to_string()))?;
        }

        // Publish to broker
        let queue_name = task
            .queue
            .clone()
            .unwrap_or_else(|| queue::QUEUE_DEFAULT.to_string());
        self.broker
            .publish_task(queue_name, task)
            .await
            .map_err(|e| SchedulerError::Broker(e.to_string()))?;

        Ok(())
    }

    /// Schedule a parallel task directly.
    ///
    /// Go parity: `scheduleParallelTask` — marks parent as RUNNING, creates
    /// subtasks, updates datastore, and publishes subtasks to broker.
    pub async fn schedule_parallel_task(&self, task: &mut Task) -> Result<(), SchedulerError> {
        let now = OffsetDateTime::now_utc();
        apply_parallel_transition(task, now);

        // Create subtasks for parallel construct
        self.create_parallel_subtasks(task).await?;

        // Update parent task in datastore
        if let Some(task_id) = &task.id {
            self.ds
                .update_task(task_id.clone(), task.clone())
                .await
                .map_err(|e| SchedulerError::Datastore(e.to_string()))?;
        }

        Ok(())
    }

    /// Schedule an each (loop) task directly.
    ///
    /// Go parity: `scheduleEachTask` — marks parent as RUNNING, creates
    /// subtasks for each iteration, updates datastore, and publishes subtasks.
    pub async fn schedule_each_task(&self, task: &mut Task) -> Result<(), SchedulerError> {
        let now = OffsetDateTime::now_utc();
        apply_each_transition(task, now);

        // Get the each task details
        let each = task
            .each
            .as_ref()
            .ok_or_else(|| SchedulerError::Validation("each task has no each field".into()))?;

        // Get concurrency limit (0 means unlimited)
        let concurrency = each.concurrency;

        // Build context for evaluating the list from job inputs
        let context = self.build_eval_context(task).await;

        // Get the list expression
        let list_expr = each
            .list
            .as_ref()
            .ok_or_else(|| SchedulerError::Validation("each task has no list expression".into()))?;

        // Evaluate the list expression to get items
        let items = self.parse_list_expression(list_expr, &context).await;

        // Update parent's each.size and each.index (Go parity: u.Each.Size = len(list); u.Each.Index = u.Each.Concurrency)
        if let Some(ref mut e) = task.each {
            e.size = items.len() as i64;
            e.index = concurrency;
        }

        // Create subtasks for each construct
        self.create_each_subtasks(task).await?;

        // Update parent task in datastore
        if let Some(task_id) = &task.id {
            self.ds
                .update_task(task_id.clone(), task.clone())
                .await
                .map_err(|e| SchedulerError::Datastore(e.to_string()))?;
        }

        Ok(())
    }

    /// Schedule a sub-job task directly.
    ///
    /// Go parity: `scheduleAttachedSubJob` — marks parent as RUNNING, creates
    /// subjob tasks, updates datastore, and publishes subjobs.
    pub async fn schedule_subjob_task(&self, task: &mut Task) -> Result<(), SchedulerError> {
        let now = OffsetDateTime::now_utc();
        apply_subjob_transition(task, now);

        // Create subjob tasks
        self.create_subjob_tasks(task).await?;

        // Update parent task in datastore
        if let Some(task_id) = &task.id {
            self.ds
                .update_task(task_id.clone(), task.clone())
                .await
                .map_err(|e| SchedulerError::Datastore(e.to_string()))?;
        }

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Private: Create parallel subtasks
    // ---------------------------------------------------------------------------

    /// Creates subtasks for a parallel task construct.
    ///
    /// Go parity: `scheduleParallelTask` creates subtasks for each task in
    /// the parallel.tasks list.
    async fn create_parallel_subtasks(&self, parent: &Task) -> Result<(), SchedulerError> {
        let parallel = parent.parallel.as_ref().ok_or_else(|| {
            SchedulerError::Validation("parallel task has no parallel field".into())
        })?;

        let tasks = parallel
            .tasks
            .as_ref()
            .ok_or_else(|| SchedulerError::Validation("parallel task has no tasks".into()))?;

        let now = OffsetDateTime::now_utc();

        for (_index, subtask) in tasks.iter().enumerate() {
            let mut child_task = subtask.clone();

            // Set parent_id to link subtask to parent
            child_task.parent_id = parent.id.clone();

            // Assign a unique ID if not present
            if child_task.id.is_none() {
                child_task.id = Some(uuid::Uuid::new_v4().to_string().replace('-', ""));
            }

            // Set position to parent's position (Go parity: pt.Position = t.Position)
            child_task.position = parent.position;

            // =====================================================================
            // GAP 4 FIX: Parallel subtasks missing created_at timestamp
            // =====================================================================
            // Go scheduleParallelTask (line 288): pt.CreatedAt = &now
            child_task.created_at = Some(now);

            // Apply parent job_id and defaults
            child_task.job_id = parent.job_id.clone();
            if let Some(job_id) = &parent.job_id {
                if let Ok(Some(job)) = self.ds.get_job_by_id(job_id.clone()).await {
                    if let Some(defaults) = &job.defaults {
                        apply_job_defaults(&mut child_task, defaults);
                    }
                }
            }

            // Apply state transition
            apply_regular_transition(&mut child_task, now);

            // Create the subtask in datastore
            self.ds
                .create_task(child_task.clone())
                .await
                .map_err(|e| SchedulerError::Datastore(e.to_string()))?;

            // Publish subtask to broker
            let queue_name = child_task
                .queue
                .clone()
                .unwrap_or_else(|| queue::QUEUE_DEFAULT.to_string());
            self.broker
                .publish_task(queue_name, &child_task)
                .await
                .map_err(|e| SchedulerError::Broker(e.to_string()))?;
        }

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Private: Create each subtasks
    // ---------------------------------------------------------------------------

    /// Creates subtasks for an each (loop) task construct.
    ///
    /// Go parity: `scheduleEachTask` evaluates the list expression and creates
    /// a subtask for each item in the list. Item values are injected into the
    /// child task's context via template evaluation.
    async fn create_each_subtasks(&self, parent: &Task) -> Result<(), SchedulerError> {
        let each = parent
            .each
            .as_ref()
            .ok_or_else(|| SchedulerError::Validation("each task has no each field".into()))?;

        let inner_task = each
            .task
            .as_ref()
            .ok_or_else(|| SchedulerError::Validation("each task has no inner task".into()))?;

        // Get the list expression
        let list_expr = each
            .list
            .as_ref()
            .ok_or_else(|| SchedulerError::Validation("each task has no list expression".into()))?;

        // Build context for evaluating the list from job inputs
        let context = self.build_eval_context(parent).await;

        // Try to evaluate the list expression as a template
        // If evaluation fails, treat as comma-separated literal values
        let items = self.parse_list_expression(list_expr, &context).await;

        // =====================================================================
        // GAP 2 FIX: Each task list evaluation failure not handled
        // =====================================================================
        // Go scheduleEachTask (lines 207-210):
        //   } else {
        //       t.Error = "each.list does not evaluate to a list"
        //       t.State = tork.TaskStateFailed
        //       return s.broker.PublishTask(ctx, broker.QUEUE_ERROR, t)
        //   }
        if items.is_empty() {
            let mut failed_parent = parent.clone();
            failed_parent.state = TASK_STATE_FAILED.clone();
            failed_parent.error = Some("each.list does not evaluate to a list".to_string());
            let _ = self
                .broker
                .publish_task(QUEUE_ERROR.into(), &failed_parent)
                .await;
            return Err(SchedulerError::Validation(
                "each.list does not evaluate to a list".into(),
            ));
        }

        let var_name = each.var.clone().unwrap_or_else(|| "item".to_string());

        // Get concurrency limit (0 means unlimited)
        let concurrency = each.concurrency;

        // Capture now for created_at timestamps (Go parity: line 244)
        let now = OffsetDateTime::now_utc();

        for (index, item) in items.iter().enumerate() {
            // Build a child-specific context with the loop variable injected as an object with index and value
            // Go parity: cx[eachVar] = map[string]any{"index": fmt.Sprintf("%d", ix), "value": item}
            let mut child_context = context.clone();
            child_context.insert(
                var_name.clone(),
                serde_json::json!({
                    "index": index.to_string(),
                    "value": item.clone()
                }),
            );

            // Evaluate the inner task template with the item value substituted
            // Go parity: if err := eval.EvaluateTask(et, cx); err != nil { t.Error = err.Error(); t.State = tork.TaskStateFailed; return s.broker.PublishTask(ctx, broker.QUEUE_ERROR, t) }
            let mut child_task = match crate::handlers::job::eval::evaluate_task(
                inner_task,
                &child_context,
            ) {
                Ok(ct) => ct,
                Err(e) => {
                    // Failed to evaluate task template - fail the parent task
                    let mut failed_parent = parent.clone();
                    failed_parent.state = TASK_STATE_FAILED.clone();
                    failed_parent.error = Some(e.clone());
                    if let Err(pub_err) = self
                        .broker
                        .publish_task(QUEUE_ERROR.into(), &failed_parent)
                        .await
                    {
                        tracing::error!(error = %pub_err, "failed to publish failed each task to error queue");
                    }
                    return Err(SchedulerError::Task(e));
                }
            };

            // Set parent_id
            child_task.parent_id = parent.id.clone();

            // Assign unique ID if not present
            if child_task.id.is_none() {
                child_task.id = Some(uuid::Uuid::new_v4().to_string().replace('-', ""));
            }

            // Set position to parent's position (Go parity: et.Position = t.Position)
            child_task.position = parent.position;

            // =====================================================================
            // GAP 3 FIX: Each subtasks missing created_at timestamp
            // =====================================================================
            // Go scheduleEachTask (line 244): et.CreatedAt = &now
            child_task.created_at = Some(now);

            // Set the var field to the item value (Go parity: task.Var = item)
            child_task.var = Some(item.clone());

            // Apply parent job_id and defaults
            child_task.job_id = parent.job_id.clone();
            if let Some(job_id) = &parent.job_id {
                if let Ok(Some(job)) = self.ds.get_job_by_id(job_id.clone()).await {
                    if let Some(defaults) = &job.defaults {
                        apply_job_defaults(&mut child_task, defaults);
                    }
                }
            }

            // Determine if this task should be published based on concurrency
            // Go parity: if t.Each.Concurrency == 0 || ix < t.Each.Concurrency { et.State = tork.TaskStatePending } else { et.State = tork.TaskStateCreated }
            let should_publish = concurrency == 0 || (index as i64) < concurrency;

            if should_publish {
                child_task.state = TASK_STATE_PENDING.clone();
            } else {
                child_task.state = TASK_STATE_CREATED.clone();
            }

            // Create the subtask in datastore
            self.ds
                .create_task(child_task.clone())
                .await
                .map_err(|e| SchedulerError::Datastore(e.to_string()))?;

            // Only publish to broker if within concurrency limit (Go parity: if t.Each.Concurrency == 0 || ix < t.Each.Concurrency { s.broker.PublishTask(ctx, broker.QUEUE_PENDING, et) })
            if should_publish {
                let queue_name = child_task
                    .queue
                    .clone()
                    .unwrap_or_else(|| queue::QUEUE_DEFAULT.to_string());
                self.broker
                    .publish_task(queue_name, &child_task)
                    .await
                    .map_err(|e| SchedulerError::Broker(e.to_string()))?;
            }
        }

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Private: Create subjob tasks
    // ---------------------------------------------------------------------------

    /// Creates subtasks for a sub-job task construct.
    ///
    /// Go parity: `scheduleAttachedSubJob` creates child jobs/tasks for
    /// sub-jobs that are not detached. For detached subjobs (`scheduleDetachedSubJob`),
    /// a standalone job is created, persisted, and published — fire-and-forget.
    async fn create_subjob_tasks(&self, parent: &Task) -> Result<(), SchedulerError> {
        let subjob = parent
            .subjob
            .as_ref()
            .ok_or_else(|| SchedulerError::Validation("subjob task has no subjob field".into()))?;

        // Detached subjob: create standalone job and publish (fire-and-forget)
        if subjob.detached {
            return self.create_detached_subjob(parent, subjob).await;
        }

        // =====================================================================
        // GAP 5 FIX: Attached subjob - missing entire subjob Job creation flow
        // =====================================================================
        // Go scheduleAttachedSubJob (lines 102-142):
        //   1. Creates subjob Job object with ParentID=t.ID
        //   2. Updates parent task with SubJob.ID = subjob.ID (BEFORE CreateJob)
        //   3. Calls ds.CreateJob(ctx, subjob)
        //   4. Calls broker.PublishJob(ctx, subjob)
        //
        // Rust create_subjob_tasks was incorrectly creating subtasks directly
        // without creating a subjob Job and publishing it. This is now fixed!

        // For attached subjobs, we need to create a proper subjob Job first
        let now = OffsetDateTime::now_utc();
        let job_id = uuid::Uuid::new_v4().to_string().replace('-', "");

        // Get parent job for created_by and permissions (Go lines 103-106)
        let (created_by, permissions) = if let Some(ref job_id) = parent.job_id {
            match self.ds.get_job_by_id(job_id.clone()).await {
                Ok(Some(job)) => (job.created_by, job.permissions),
                _ => (None, None),
            }
        } else {
            (None, None)
        };

        // Build the subjob Job (Go lines 108-128)
        let mut subjob_job = tork::job::Job {
            id: Some(job_id.clone()),
            parent_id: parent.id.clone(), // Go: ParentID: t.ID
            created_by,
            created_at: now,
            permissions,
            name: subjob.name.clone(),
            description: subjob.description.clone(),
            state: tork::job::JOB_STATE_PENDING.to_string(),
            inputs: subjob.inputs.clone(),
            secrets: subjob.secrets.clone(),
            context: tork::job::JobContext {
                inputs: subjob.inputs.clone(),
                secrets: subjob.secrets.clone(),
                ..Default::default()
            },
            task_count: subjob.tasks.as_ref().map_or(0, |t| t.len() as i64),
            tasks: subjob.tasks.as_ref().map_or_else(Vec::new, |t| t.clone()),
            output: subjob.output.clone(),
            ..Default::default()
        };
        subjob_job.webhooks = subjob.webhooks.clone();
        subjob_job.auto_delete = subjob.auto_delete.clone();

        // Create and publish the subjob Job (Go lines 138-141)
        self.ds
            .create_job(subjob_job.clone())
            .await
            .map_err(|e| SchedulerError::Datastore(e.to_string()))?;
        self.broker
            .publish_job(&subjob_job)
            .await
            .map_err(|e| SchedulerError::Broker(e.to_string()))?;

        // Return early since we've handled attached subjob above
        // The rest of the old code was for detached subjobs only
        Ok(())
    }

    /// Handles detached subjob: creates a standalone job, persists it,
    /// publishes it to the broker, and immediately completes the parent task.
    ///
    /// Go parity: `scheduleDetachedSubJob` — fire-and-forget pattern.
    async fn create_detached_subjob(
        &self,
        parent: &Task,
        subjob: &tork::task::SubJobTask,
    ) -> Result<(), SchedulerError> {
        let now = OffsetDateTime::now_utc();

        // =====================================================================
        // GAP 6 FIX: Detached subjob missing created_by and permissions
        // =====================================================================
        // Go scheduleDetachedSubJob (lines 150-154):
        //   subjob := &tork.Job{
        //       ID:          uuid.NewUUID(),
        //       CreatedBy:   job.CreatedBy,    // <-- MISSING
        //       CreatedAt:   now,
        //       Permissions: job.Permissions,  // <-- MISSING
        //       ...
        //   }

        // Get parent job for created_by and permissions
        let (created_by, permissions) = if let Some(ref job_id) = parent.job_id {
            match self.ds.get_job_by_id(job_id.clone()).await {
                Ok(Some(job)) => (job.created_by, job.permissions),
                _ => (None, None),
            }
        } else {
            (None, None)
        };

        // 1. Build a standalone Job from the subjob definition
        let job_id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let mut new_job = tork::job::Job {
            id: Some(job_id.clone()),
            parent_id: parent.job_id.clone(),
            created_by,
            created_at: now,
            permissions,
            name: subjob.name.clone(),
            description: subjob.description.clone(),
            state: tork::job::JOB_STATE_PENDING.to_string(),
            tasks: subjob.tasks.clone().unwrap_or_default(),
            inputs: subjob.inputs.clone(),
            secrets: subjob.secrets.clone(),
            context: tork::job::JobContext {
                inputs: subjob.inputs.clone(),
                secrets: subjob.secrets.clone(),
                ..tork::job::JobContext::default()
            },
            task_count: subjob.tasks.as_ref().map_or(0, |t| t.len() as i64),
            ..tork::job::Job::default()
        };
        new_job.webhooks = subjob.webhooks.clone();
        new_job.auto_delete = subjob.auto_delete.clone();

        // 2. Persist the new job
        self.ds
            .create_job(new_job.clone())
            .await
            .map_err(|e| SchedulerError::Datastore(e.to_string()))?;

        // 3. Publish the job to the broker
        self.broker
            .publish_job(&new_job)
            .await
            .map_err(|e| SchedulerError::Broker(e.to_string()))?;

        tracing::info!(
            parent_task_id = parent.id.as_deref().unwrap_or("unknown"),
            detached_job_id = %job_id,
            "published detached subjob"
        );

        // 4. Immediately complete the parent task (fire-and-forget)
        let mut completed_parent = parent.clone();
        completed_parent.state = tork::task::TASK_STATE_COMPLETED.clone();
        completed_parent.completed_at = Some(now);

        self.broker
            .publish_task(queue::QUEUE_COMPLETED.to_string(), &completed_parent)
            .await
            .map_err(|e| SchedulerError::Broker(e.to_string()))?;

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Private: Build eval context from job inputs
    // ---------------------------------------------------------------------------

    /// Builds the evaluation context from job inputs and task variables.
    async fn build_eval_context(&self, task: &Task) -> HashMap<String, serde_json::Value> {
        let mut context = HashMap::new();

        // Load job context if we have a job_id
        if let Some(job_id) = &task.job_id {
            if let Ok(Some(job)) = self.ds.get_job_by_id(job_id.clone()).await {
                // Add job inputs to context
                if let Some(inputs) = &job.inputs {
                    for (key, value) in inputs {
                        context.insert(key.clone(), serde_json::Value::String(value.clone()));
                    }
                }
                // Add job context secrets (sanitized)
                if let Some(secrets) = &job.context.secrets {
                    for (key, value) in secrets {
                        // Only expose non-sensitive data in context
                        context.insert(key.clone(), serde_json::Value::String(value.clone()));
                    }
                }
            }
        }

        // Add task var if present (for each loop iteration)
        if let Some(var) = &task.var {
            context.insert(
                var.clone(),
                serde_json::Value::String("{{item}}".to_string()),
            );
        }

        context
    }

    // ---------------------------------------------------------------------------
    // Private: Parse list expression
    // ---------------------------------------------------------------------------

    /// Evaluates the list expression to get items
    async fn parse_list_expression(
        &self,
        list_expr: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Vec<String> {
        let sanitized = list_expr.trim();

        // 1. Try to parse as JSON array first
        if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(sanitized) {
            return items.into_iter().map(|v| v.to_string().replace('"', "")).collect();
        }

        // 2. Try expression evaluation via evalexpr
        if sanitized.starts_with("{{") && sanitized.ends_with("}}") {
            if let Some(items) = self.try_eval_list_expression(sanitized, context) {
                return items;
            }
            // If it was an expression and failed, return empty to trigger error
            return vec![];
        }

        // 3. Fall back to comma-separated values ONLY if it contains a comma
        if sanitized.contains(',') {
            return sanitized
                .split(',')
                .map(str::trim)
                .map(String::from)
                .collect();
        }

        // 4. Not a list
        vec![]
    }

    /// Attempts to evaluate a list expression using evalexpr.
    ///
    /// Supports expressions like `{{ sequence(1,5) }}` or variables
    /// that resolve to arrays. Returns `None` if evaluation fails
    /// or the result is not a list.
    fn try_eval_list_expression(
        &self,
        list_expr: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Option<Vec<String>> {
        // Build the evalexpr context (including built-in functions like sequence)
        let evalexpr_ctx = crate::handlers::job::eval::create_eval_context(context).ok()?;

        // Sanitize: strip {{ }} wrappers if present
        let sanitized = list_expr.trim();
        let inner = TEMPLATE_REGEX
            .captures(sanitized)
            .map_or_else(|| sanitized.to_string(), |caps| caps[1].trim().to_string());

        if inner.is_empty() {
            return None;
        }

        // Evaluate the expression
        let result = evalexpr::eval_with_context(&inner, &evalexpr_ctx).ok()?;

        // Convert result to a list of strings
        eval_value_to_string_list(&result)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converts an evalexpr Value to a list of strings, if it is a list/tuple.
/// Returns `None` if the value cannot be interpreted as a list.
fn eval_value_to_string_list(value: &evalexpr::Value) -> Option<Vec<String>> {
    match value {
        evalexpr::Value::Tuple(items) => {
            let strings: Vec<String> = items
                .iter()
                .map(|v| match v {
                    evalexpr::Value::String(s) => s.clone(),
                    evalexpr::Value::Int(i) => i.to_string(),
                    evalexpr::Value::Float(f) => f.to_string(),
                    evalexpr::Value::Boolean(b) => b.to_string(),
                    _ => v.to_string(),
                })
                .collect();
            Some(strings)
        }
        evalexpr::Value::String(s) => {
            // A single string — try parsing as JSON array
            serde_json::from_str::<Vec<String>>(s).ok()
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during scheduling.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SchedulerError {
    #[error("scheduling error: {0}")]
    Schedule(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("task error: {0}")]
    Task(String),

    #[error("datastore error: {0}")]
    Datastore(String),

    #[error("broker error: {0}")]
    Broker(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tork::task::{EachTask, ParallelTask, SubJobTask};

    // -- classify_task_type (pure calc) ------------------------------------

    #[test]
    fn test_classify_regular() {
        let task = Task::default();
        assert_eq!(classify_task_type(&task), ScheduledTaskType::Regular);
    }

    #[test]
    fn test_classify_parallel() {
        let task = Task {
            parallel: Some(ParallelTask {
                tasks: None,
                completions: 0,
            }),
            ..Task::default()
        };
        assert_eq!(classify_task_type(&task), ScheduledTaskType::Parallel);
    }

    #[test]
    fn test_classify_each() {
        let task = Task {
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
        assert_eq!(classify_task_type(&task), ScheduledTaskType::Each);
    }

    #[test]
    fn test_classify_subjob() {
        let task = Task {
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
        assert_eq!(classify_task_type(&task), ScheduledTaskType::SubJob);
    }

    // -- state transitions --------------------------------------------------

    #[test]
    fn test_apply_regular_transition() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task::default();
        apply_regular_transition(&mut task, now);
        assert_eq!(task.state.as_ref(), TASK_STATE_SCHEDULED);
        assert_eq!(task.scheduled_at, Some(now));
    }

    #[test]
    fn test_apply_parallel_transition() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task::default();
        apply_parallel_transition(&mut task, now);
        assert_eq!(task.state.as_ref(), TASK_STATE_RUNNING);
        assert_eq!(task.scheduled_at, Some(now));
        assert_eq!(task.started_at, Some(now));
    }

    #[test]
    fn test_apply_each_transition() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task::default();
        apply_each_transition(&mut task, now);
        assert_eq!(task.state.as_ref(), TASK_STATE_RUNNING);
        assert_eq!(task.scheduled_at, Some(now));
        assert_eq!(task.started_at, Some(now));
    }

    #[test]
    fn test_apply_subjob_transition() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task::default();
        apply_subjob_transition(&mut task, now);
        assert_eq!(task.state.as_ref(), TASK_STATE_RUNNING);
        assert_eq!(task.scheduled_at, Some(now));
        assert_eq!(task.started_at, Some(now));
    }

    // -- apply_job_defaults ------------------------------------------------

    #[test]
    fn test_apply_job_defaults_queue() {
        let mut task = Task::default();
        let defaults = JobDefaults {
            queue: Some("my-queue".to_string()),
            ..JobDefaults::default()
        };
        apply_job_defaults(&mut task, &defaults);
        assert_eq!(task.queue.as_deref(), Some("my-queue"));
    }

    #[test]
    fn test_apply_job_defaults_preserves_existing_queue() {
        let mut task = Task {
            queue: Some("existing-queue".to_string()),
            ..Task::default()
        };
        let defaults = JobDefaults {
            queue: Some("default-queue".to_string()),
            ..JobDefaults::default()
        };
        apply_job_defaults(&mut task, &defaults);
        assert_eq!(task.queue.as_deref(), Some("existing-queue"));
    }

    #[test]
    fn test_apply_job_defaults_timeout() {
        let mut task = Task::default();
        let defaults = JobDefaults {
            timeout: Some("5m".to_string()),
            ..JobDefaults::default()
        };
        apply_job_defaults(&mut task, &defaults);
        assert_eq!(task.timeout.as_deref(), Some("5m"));
    }

    #[test]
    fn test_apply_job_defaults_priority() {
        let mut task = Task::default();
        let defaults = JobDefaults {
            priority: 10,
            ..JobDefaults::default()
        };
        apply_job_defaults(&mut task, &defaults);
        assert_eq!(task.priority, 10);
    }

    #[test]
    fn test_apply_job_defaults_retry() {
        let mut task = Task::default();
        let defaults = JobDefaults {
            retry: Some(tork::task::TaskRetry {
                limit: 3,
                attempts: 1,
            }),
            ..JobDefaults::default()
        };
        apply_job_defaults(&mut task, &defaults);
        assert!(task.retry.is_some());
        assert_eq!(task.retry.as_ref().map(|r| r.limit), Some(3));
    }

    #[test]
    fn test_apply_job_defaults_limits() {
        let mut task = Task::default();
        let defaults = JobDefaults {
            limits: Some(tork::task::TaskLimits {
                cpus: Some("2".to_string()),
                memory: Some("4Gi".to_string()),
            }),
            ..JobDefaults::default()
        };
        apply_job_defaults(&mut task, &defaults);
        assert!(task.limits.is_some());
        assert_eq!(
            task.limits.as_ref().and_then(|l| l.cpus.as_deref()),
            Some("2")
        );
    }

    // -- Scheduler ----------------------------------------------------------

    #[test]
    fn test_scheduled_task_type_display() {
        assert_eq!(ScheduledTaskType::Regular.to_string(), "REGULAR");
        assert_eq!(ScheduledTaskType::Parallel.to_string(), "PARALLEL");
        assert_eq!(ScheduledTaskType::Each.to_string(), "EACH");
        assert_eq!(ScheduledTaskType::SubJob.to_string(), "SUBJOB");
    }

    // -- Additional edge cases from Go test parity ---------------------------

    // Go: Classify priority — task with each AND parallel (each wins per order)
    #[test]
    fn test_classify_each_over_parallel() {
        let task = Task {
            each: Some(EachTask {
                var: None,
                list: None,
                task: None,
                size: 0,
                completions: 0,
                concurrency: 0,
                index: 0,
            }),
            parallel: Some(ParallelTask {
                tasks: None,
                completions: 0,
            }),
            ..Task::default()
        };
        assert_eq!(classify_task_type(&task), ScheduledTaskType::Each);
    }

    // Go: Classify priority — parallel wins over subjob (per check order: each > parallel > subjob)
    #[test]
    fn test_classify_parallel_over_subjob() {
        let task = Task {
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
            parallel: Some(ParallelTask {
                tasks: None,
                completions: 0,
            }),
            ..Task::default()
        };
        // parallel is checked before subjob in classify_task_type
        assert_eq!(classify_task_type(&task), ScheduledTaskType::Parallel);
    }

    // Go: State transition preserves existing fields
    #[test]
    fn test_apply_regular_transition_preserves_fields() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task {
            id: Some("t1".into()),
            job_id: Some("j1".into()),
            name: Some("build".into()),
            position: 5,
            queue: Some("my-queue".into()),
            ..Task::default()
        };
        apply_regular_transition(&mut task, now);
        assert_eq!(task.id.as_deref(), Some("t1"));
        assert_eq!(task.job_id.as_deref(), Some("j1"));
        assert_eq!(task.name.as_deref(), Some("build"));
        assert_eq!(task.position, 5);
        assert_eq!(task.queue.as_deref(), Some("my-queue"));
        assert_eq!(task.state.as_ref(), TASK_STATE_SCHEDULED);
    }

    #[test]
    fn test_apply_parallel_transition_preserves_fields() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task {
            id: Some("t1".into()),
            parallel: Some(ParallelTask {
                tasks: Some(vec![Task::default()]),
                completions: 0,
            }),
            ..Task::default()
        };
        apply_parallel_transition(&mut task, now);
        assert_eq!(task.id.as_deref(), Some("t1"));
        assert!(task.parallel.is_some());
        assert_eq!(task.state.as_ref(), TASK_STATE_RUNNING);
    }

    #[test]
    fn test_apply_each_transition_preserves_fields() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task {
            id: Some("t1".into()),
            each: Some(EachTask {
                var: None,
                list: None,
                task: None,
                size: 5,
                completions: 0,
                concurrency: 2,
                index: 0,
            }),
            ..Task::default()
        };
        apply_each_transition(&mut task, now);
        assert_eq!(task.id.as_deref(), Some("t1"));
        assert!(task.each.is_some());
        assert_eq!(task.state.as_ref(), TASK_STATE_RUNNING);
    }

    #[test]
    fn test_apply_subjob_transition_preserves_fields() {
        let now = OffsetDateTime::now_utc();
        let mut task = Task {
            id: Some("t1".into()),
            subjob: Some(SubJobTask {
                id: None,
                name: Some("sub".into()),
                description: None,
                tasks: Some(vec![]),
                inputs: None,
                secrets: None,
                auto_delete: None,
                output: None,
                detached: false,
                webhooks: None,
            }),
            ..Task::default()
        };
        apply_subjob_transition(&mut task, now);
        assert_eq!(task.id.as_deref(), Some("t1"));
        assert!(task.subjob.is_some());
        assert_eq!(task.state.as_ref(), TASK_STATE_RUNNING);
    }

    // Go: SchedulerError variants
    #[test]
    fn test_scheduler_error_display() {
        let err = SchedulerError::Schedule("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = SchedulerError::Validation("bad input".to_string());
        assert!(err.to_string().contains("bad input"));

        let err = SchedulerError::Task("task failed".to_string());
        assert!(err.to_string().contains("task failed"));

        let err = SchedulerError::Datastore("db error".to_string());
        assert!(err.to_string().contains("db error"));

        let err = SchedulerError::Broker("broker error".to_string());
        assert!(err.to_string().contains("broker error"));
    }

    // Go: Timestamps are set to current time
    #[test]
    fn test_apply_regular_transition_timestamp_is_recent() {
        let before = OffsetDateTime::now_utc();
        let mut task = Task::default();
        apply_regular_transition(&mut task, OffsetDateTime::now_utc());
        let after = OffsetDateTime::now_utc();
        let scheduled = task.scheduled_at.expect("should have scheduled_at");
        assert!(scheduled >= before && scheduled <= after);
    }

    #[test]
    fn test_apply_parallel_transition_both_timestamps_recent() {
        let before = OffsetDateTime::now_utc();
        let mut task = Task::default();
        apply_parallel_transition(&mut task, OffsetDateTime::now_utc());
        let after = OffsetDateTime::now_utc();
        let scheduled = task.scheduled_at.expect("should have scheduled_at");
        let started = task.started_at.expect("should have started_at");
        assert!(scheduled >= before && scheduled <= after);
        assert!(started >= before && started <= after);
    }

    // -- Parity tests from Go scheduler_test.go ----------------------------

    use dashmap::DashMap;
    use tork::job::Job;

    struct TestDatastore {
        jobs: Arc<DashMap<String, Job>>,
        tasks: Arc<DashMap<String, Task>>,
    }

    impl TestDatastore {
        fn new() -> Self {
            Self {
                jobs: Arc::new(DashMap::new()),
                tasks: Arc::new(DashMap::new()),
            }
        }
    }

    impl Datastore for TestDatastore {
        fn create_task(&self, task: Task) -> tork::datastore::BoxedFuture<()> {
            let id = task.id.clone().expect("task id required");
            self.tasks.insert(id, task);
            Box::pin(async { Ok(()) })
        }
        fn update_task(&self, id: String, task: Task) -> tork::datastore::BoxedFuture<()> {
            self.tasks.insert(id, task);
            Box::pin(async { Ok(()) })
        }
        fn get_task_by_id(&self, id: String) -> tork::datastore::BoxedFuture<Option<Task>> {
            let task = self.tasks.get(&id).map(|r| r.value().clone());
            Box::pin(async move { Ok(task) })
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
                    size: 10,
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
        fn create_job(&self, job: Job) -> tork::datastore::BoxedFuture<()> {
            let id = job.id.clone().expect("job id required");
            self.jobs.insert(id, job);
            Box::pin(async { Ok(()) })
        }
        fn update_job(&self, id: String, job: Job) -> tork::datastore::BoxedFuture<()> {
            self.jobs.insert(id, job);
            Box::pin(async { Ok(()) })
        }
        fn get_job_by_id(&self, id: String) -> tork::datastore::BoxedFuture<Option<Job>> {
            let job = self.jobs.get(&id).map(|r| r.value().clone());
            Box::pin(async move { Ok(job) })
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
                    size: 10,
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
                    size: 10,
                })
            })
        }
        fn create_scheduled_job(&self, _sj: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
        fn get_active_scheduled_jobs(&self) -> tork::datastore::BoxedFuture<Vec<tork::job::ScheduledJob>> {
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
                    size: 10,
                })
            })
        }
        fn get_scheduled_job_by_id(&self, _id: String) -> tork::datastore::BoxedFuture<Option<tork::job::ScheduledJob>> {
            Box::pin(async { Ok(None) })
        }
        fn update_scheduled_job(&self, _id: String, _sj: tork::job::ScheduledJob) -> tork::datastore::BoxedFuture<()> {
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

    #[tokio::test]
    async fn test_schedule_regular_task() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_tasks("test-queue".to_string(), Arc::new(move |_task: Arc<Task>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j1 = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j1).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            queue: Some("test-queue".to_string()),
            job_id: Some(job_id),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        s.schedule_regular_task(&mut tk).await.unwrap();

        // Wait for processing
        rx.recv().await.unwrap();

        let tk = ds.get_task_by_id(task_id).await.unwrap().unwrap();
        assert_eq!(tk.state.as_ref(), TASK_STATE_SCHEDULED);
    }

    #[tokio::test]
    async fn test_schedule_regular_task_job_defaults() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let job_id = uuid::Uuid::new_v4().to_string();
        let j1 = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            defaults: Some(JobDefaults {
                queue: Some("some-queue".to_string()),
                retry: Some(tork::task::TaskRetry {
                    limit: 5,
                    attempts: 0,
                }),
                limits: Some(tork::task::TaskLimits {
                    cpus: Some(".5".to_string()),
                    memory: Some("10m".to_string()),
                }),
                timeout: Some("5s".to_string()),
                priority: 3,
            }),
            ..Default::default()
        };
        ds.create_job(j1).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        s.schedule_regular_task(&mut tk).await.unwrap();

        let tk = ds.get_task_by_id(task_id).await.unwrap().unwrap();
        assert_eq!(tk.state.as_ref(), TASK_STATE_SCHEDULED);
        assert_eq!(tk.queue.as_deref(), Some("some-queue"));
        assert_eq!(tk.retry.as_ref().map(|r| r.limit), Some(5));
        assert_eq!(tk.limits.as_ref().and_then(|l| l.cpus.as_deref()), Some(".5"));
        assert_eq!(tk.limits.as_ref().and_then(|l| l.memory.as_deref()), Some("10m"));
        assert_eq!(tk.timeout.as_deref(), Some("5s"));
        assert_eq!(tk.priority, 3);
    }

    #[tokio::test]
    async fn test_schedule_parallel_task() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_tasks("test-queue".to_string(), Arc::new(move |tk: Arc<Task>| {
            let tx = tx.clone();
            assert_eq!(tk.queue.as_deref(), Some("test-queue"));
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            parallel: Some(ParallelTask {
                tasks: Some(vec![Task {
                    name: Some("my parallel task".to_string()),
                    queue: Some("test-queue".to_string()),
                    ..Default::default()
                }]),
                completions: 0,
            }),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        s.schedule_parallel_task(&mut tk).await.unwrap();

        // wait for the task to get processed
        rx.recv().await.unwrap();

        let tk = ds.get_task_by_id(task_id).await.unwrap().unwrap();
        assert_eq!(tk.state.as_ref(), TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_each_task() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        b.subscribe_for_tasks("test-queue".to_string(), Arc::new(move |tk: Arc<Task>| {
            let tx = tx.clone();
            assert_eq!(tk.queue.as_deref(), Some("test-queue"));
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            each: Some(EachTask {
                list: Some("{{ sequence(1,3) }}".to_string()),
                task: Some(Box::new(Task {
                    queue: Some("test-queue".to_string()),
                    env: Some(HashMap::from([
                        ("ITEM_INDEX".to_string(), "{{item.index}}".to_string()),
                        ("ITEM_VAL".to_string(), "{{item.value}}".to_string()),
                    ])),
                    ..Default::default()
                })),
                ..Default::default()
            }),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        s.schedule_each_task(&mut tk).await.unwrap();

        // wait for the tasks to get processed
        rx.recv().await.unwrap();
        rx.recv().await.unwrap();

        let tk = ds.get_task_by_id(task_id).await.unwrap().unwrap();
        assert_eq!(tk.state.as_ref(), TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_subjob_task() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_jobs(Arc::new(move |j: tork::job::Job| {
            let tx = tx.clone();
            assert!(j.parent_id.is_some());
            assert_eq!(j.inputs.as_ref().and_then(|i: &HashMap<String, String>| i.get("some_input")), Some(&"https://example.com".to_string()));
            assert_eq!(j.secrets.as_ref().and_then(|s: &HashMap<String, String>| s.get("some_secret")), Some(&"password".to_string()));
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            subjob: Some(SubJobTask {
                name: Some("my sub job".to_string()),
                inputs: Some(HashMap::from([("some_input".to_string(), "https://example.com".to_string())])),
                secrets: Some(HashMap::from([("some_secret".to_string(), "password".to_string())])),
                tasks: Some(vec![Task {
                    name: Some("some task".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        s.schedule_subjob_task(&mut tk).await.unwrap();

        // wait for the task to get processed
        rx.recv().await.unwrap();

        let tk = ds.get_task_by_id(task_id).await.unwrap().unwrap();
        assert_eq!(tk.state.as_ref(), TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_detached_subjob_task() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx_job, mut rx_job) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_jobs(Arc::new(move |j: tork::job::Job| {
            let tx = tx_job.clone();
            assert!(j.parent_id.is_some());
            assert_eq!(j.webhooks.as_ref().and_then(|w| w.first()).map(|w| w.url.as_deref()), Some(Some("http://example.com/callback")));
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let (tx_comp, mut rx_comp) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_tasks(tork::broker::queue::QUEUE_COMPLETED.to_string(), Arc::new(move |_tk: Arc<Task>| {
            let tx = tx_comp.clone();
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            subjob: Some(SubJobTask {
                name: Some("my sub job".to_string()),
                detached: true,
                tasks: Some(vec![Task {
                    name: Some("some task".to_string()),
                    ..Default::default()
                }]),
                webhooks: Some(vec![tork::task::Webhook {
                    url: Some("http://example.com/callback".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        s.schedule_subjob_task(&mut tk).await.unwrap();

        // wait for the job to get processed
        rx_job.recv().await.unwrap();

        // wait for the completion task
        rx_comp.recv().await.unwrap();

        let tk = ds.get_task_by_id(task_id).await.unwrap().unwrap();
        assert_eq!(tk.state.as_ref(), TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_each_task_concurrency() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_tasks("test-queue".to_string(), Arc::new(move |tk: Arc<Task>| {
            let tx = tx.clone();
            assert_eq!(tk.queue.as_deref(), Some("test-queue"));
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            each: Some(EachTask {
                list: Some("{{ sequence(1,3) }}".to_string()),
                concurrency: 1,
                task: Some(Box::new(Task {
                    queue: Some("test-queue".to_string()),
                    env: Some(HashMap::from([
                        ("ITEM_INDEX".to_string(), "{{item.index}}".to_string()),
                        ("ITEM_VAL".to_string(), "{{item.value}}".to_string()),
                    ])),
                    ..Default::default()
                })),
                ..Default::default()
            }),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        s.schedule_each_task(&mut tk).await.unwrap();

        // wait for one task to get processed
        rx.recv().await.unwrap();

        // check that the other task is NOT in the broker yet (due to concurrency=1)
        
        let subtasks: Vec<Task> = ds.tasks.iter().filter(|e| e.value().parent_id.as_deref() == Some(&task_id)).map(|e| e.value().clone()).collect();
        assert_eq!(subtasks.len(), 2);
        
        let pending = subtasks.iter().filter(|t| t.state.as_ref() == TASK_STATE_PENDING).count();
        let created = subtasks.iter().filter(|t| t.state.as_ref() == TASK_STATE_CREATED).count();
        
        assert_eq!(pending, 1);
        assert_eq!(created, 1);

        let tk = ds.get_task_by_id(task_id).await.unwrap().unwrap();
        assert_eq!(tk.state.as_ref(), TASK_STATE_RUNNING);
    }

    #[tokio::test]
    async fn test_schedule_each_task_not_a_list() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_tasks(tork::broker::queue::QUEUE_ERROR.to_string(), Arc::new(move |_tk: Arc<Task>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            each: Some(EachTask {
                list: Some("1".to_string()),
                task: Some(Box::new(Task::default())),
                ..Default::default()
            }),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        // should fail since "1" is not a list
        let result = s.schedule_each_task(&mut tk).await;
        assert!(result.is_err());

        // wait for the error task to be published
        rx.recv().await.unwrap();
    }

    #[tokio::test]
    async fn test_schedule_each_task_bad_expression() {
        let b = Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());
        let ds = Arc::new(TestDatastore::new());
        let s = Scheduler::new(ds.clone(), b.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        b.subscribe_for_tasks(tork::broker::queue::QUEUE_ERROR.to_string(), Arc::new(move |_tk: Arc<Task>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(()).await;
            })
        })).await.unwrap();

        let job_id = uuid::Uuid::new_v4().to_string();
        let j = Job {
            id: Some(job_id.clone()),
            name: Some("test job".to_string()),
            ..Default::default()
        };
        ds.create_job(j).await.unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        let mut tk = Task {
            id: Some(task_id.clone()),
            job_id: Some(job_id),
            each: Some(EachTask {
                list: Some("{{ bad_expression }}".to_string()),
                task: Some(Box::new(Task::default())),
                ..Default::default()
            }),
            ..Default::default()
        };

        ds.create_task(tk.clone()).await.unwrap();

        // should fail due to bad expression
        let result = s.schedule_each_task(&mut tk).await;
        assert!(result.is_err());

        // wait for the error task to be published
        rx.recv().await.unwrap();
    }
}
