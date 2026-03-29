//! Event handlers for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use twerk_infrastructure::broker::queue::{QUEUE_COMPLETED, QUEUE_FAILED, QUEUE_PENDING};
use crate::engine::TOPIC_JOB_COMPLETED;
use crate::engine::coordinator::scheduler::Scheduler;
use crate::engine::coordinator::webhook::fire_job_webhooks;
use crate::engine::coordinator::webhook::fire_task_webhooks;
use twerk_core::eval::evaluate_task;
use twerk_core::job::JOB_STATE_CANCELLED;
use twerk_core::task::{TaskLogPart, TASK_STATE_CANCELLED, TASK_STATE_RUNNING};
use twerk_core::node::NodeStatus;
use crate::engine::types::JobHandlerError;

// ── Helper Functions ───────────────────────────────────────────

/// Extracts job ID string or returns "unknown".
fn job_id_str(job: &twerk_core::job::Job) -> &str {
    job.id.as_deref().unwrap_or("unknown")
}

/// Extracts task ID string or returns "unknown".
fn task_id_str(task: &twerk_core::task::Task) -> &str {
    task.id.as_deref().unwrap_or("unknown")
}

/// Builds job context from job, merging inputs as strings.
fn build_job_context(job: &twerk_core::job::Job) -> std::collections::HashMap<String, serde_json::Value> {
    let mut ctx = job.context.as_ref()
        .map(twerk_core::job::JobContext::as_map)
        .unwrap_or_default();
    
    if let Some(inputs) = job.context.as_ref().and_then(|c| c.inputs.as_ref()) {
        for (k, v) in inputs {
            ctx.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
    }
    ctx
}

/// Checks if job is in an active state (running or scheduled).
fn is_job_active(job_state: &str) -> bool {
    matches!(job_state, s if s == twerk_core::job::JOB_STATE_RUNNING || s == twerk_core::job::JOB_STATE_SCHEDULED)
}

/// Checks if retry is available and not exhausted.
fn can_retry(retry: &twerk_core::task::TaskRetry) -> bool {
    retry.attempts < retry.limit
}

/// Validates task index is within bounds.
fn validate_task_index(tasks: &[twerk_core::task::Task], job_position: i64) -> Result<usize> {
    let task_index = (job_position - 1) as usize;
    if task_index >= tasks.len() {
        return Err(anyhow::anyhow!("job position {} out of bounds", job_position));
    }
    Ok(task_index)
}

// ── Public Handlers ─────────────────────────────────────────────

/// Handles job events from the broker.
pub async fn handle_job_event(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    debug!("Handling job event for job {} (state={})", job_id_str(&job), job.state);
    
    let result = match job.state.as_str() {
        twerk_core::job::JOB_STATE_PENDING => start_job(ds, broker, job).await,
        twerk_core::job::JOB_STATE_COMPLETED => complete_job(ds, broker, job).await,
        twerk_core::job::JOB_STATE_RESTART => restart_job(ds, broker, job).await,
        JOB_STATE_CANCELLED => handle_cancel(ds, broker, job).await
            .map_err(|e| anyhow::anyhow!("{}", e)),
        _ => Ok(()),
    };

    if let Err(ref e) = result {
        error!("failed to handle job event: {}", e);
    }
    result
}

/// Handles task progress updates from workers.
pub async fn handle_task_progress(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    match task.state.as_str() {
        twerk_core::task::TASK_STATE_PENDING => handle_pending_task(ds, broker, task).await,
        twerk_core::task::TASK_STATE_COMPLETED => handle_task_completed(ds, broker, task).await,
        _ => {
            let task_id = task.id.clone().unwrap_or_default();
            ds.update_task(&task_id, Box::new(move |mut u| {
                u.state.clone_from(&task.state);
                u.started_at = task.started_at;
                u.completed_at = task.completed_at;
                u.failed_at = task.failed_at;
                u.result.clone_from(&task.result);
                u.error.clone_from(&task.error);
                Ok(u)
            })).await.map_err(|e| anyhow::anyhow!("failed to update task: {e}"))?;
            Ok(())
        }
    }
}

/// Handles pending task by scheduling it.
pub async fn handle_pending_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let scheduler = Scheduler::new(ds, broker);
    scheduler.schedule_task(task).await
}

/// Handles redelivered tasks by incrementing redelivery counter.
pub async fn handle_redelivered(
    _ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    debug!("Handling redelivered task {} (redelivered={})", task_id_str(&task), task.redelivered);
    let task = twerk_core::task::Task {
        redelivered: task.redelivered + 1,
        ..task
    };
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await?;
    Ok(())
}

/// Handles task started event.
pub async fn handle_started(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    info!("Task {} started", task_id_str(&task));
    let task_id = task.id.clone().unwrap_or_default();
    let now = time::OffsetDateTime::now_utc();
    ds.update_task(&task_id, Box::new(move |mut u| {
        u.state = TASK_STATE_RUNNING.to_string();
        u.started_at = Some(now);
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update task: {e}"))?;
    fire_task_webhooks(ds, &task, "task.Started").await;
    Ok(())
}

/// Handles task log part.
pub async fn handle_log_part(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    part: TaskLogPart,
) -> Result<()> {
    debug!("Received log part {} for task {}", part.number, part.task_id.as_deref().unwrap_or("unknown"));
    ds.create_task_log_part(&part).await.map_err(|e| anyhow::anyhow!("failed to store log part: {e}"))?;
    Ok(())
}

/// Handles job cancellation.
pub async fn handle_cancel(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<(), JobHandlerError> {
    let job_id = job.id.clone().unwrap_or_default();
    let job_state = job.state.clone();

    if is_job_active(&job_state) {
        ds.update_job(&job_id, Box::new(|mut u| {
            u.state = JOB_STATE_CANCELLED.to_string();
            Ok(u)
        })).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    }

    if let Some(ref parent_id) = job.parent_id {
        cancel_parent_job(&ds, &broker, parent_id).await?;
    }

    cancel_active_tasks(&ds, &broker, &job_id).await?;
    Ok(())
}

// ── Private Handlers ────────────────────────────────────────────

/// Creates and schedules the first task for a job.
async fn start_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    info!("Starting job {}", job_id_str(&job));

    let tasks = job.tasks.as_ref()
        .ok_or_else(|| anyhow::anyhow!("job has no tasks"))?;
    
    if tasks.is_empty() {
        return Err(anyhow::anyhow!("job has no tasks"));
    }
    
    let now = time::OffsetDateTime::now_utc();
    let job_ctx = build_job_context(&job);
    
    let first_task = evaluate_task(&tasks[0], &job_ctx)
        .map_err(|e| anyhow::anyhow!("failed to evaluate task: {e}"))?;
    
    let task_id = uuid::Uuid::new_v4().to_string();
    let job_id = job.id.clone().unwrap_or_default();
    
    let task = twerk_core::task::Task {
        id: Some(task_id.clone().into()),
        job_id: job.id.clone(),
        state: twerk_core::task::TASK_STATE_PENDING.to_string(),
        position: 1,
        created_at: Some(now),
        ..first_task
    };
    
    ds.create_task(&task).await
        .map_err(|e| anyhow::anyhow!("failed to create task: {e}"))?;
    
    debug!("Created first task {} for job {}", task_id_str(&task), job_id);
    
    ds.update_job(&job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_SCHEDULED.to_string();
        u.started_at = Some(now);
        u.position = 1;
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update job: {e}"))?;

    fire_job_webhooks(&job, "job.Scheduled");
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await?;
    Ok(())
}

/// Restarts a job from its current position.
async fn restart_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    info!("Restarting job {}", job_id_str(&job));
    let job_id = job.id.clone().unwrap_or_default();
    let job_position = job.position;
    let now = time::OffsetDateTime::now_utc();

    ds.update_job(&job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_RUNNING.to_string();
        u.failed_at = None;
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update job: {e}"))?;

    let tasks = job.tasks.as_ref()
        .ok_or_else(|| anyhow::anyhow!("job has no tasks"))?;
    
    if tasks.is_empty() {
        return Err(anyhow::anyhow!("job has no tasks"));
    }
    
    let task_index = validate_task_index(tasks, job_position)?;
    let job_ctx = build_job_context(&job);
    
    let evaluated_task = evaluate_task(&tasks[task_index], &job_ctx)
        .map_err(|e| anyhow::anyhow!("failed to evaluate task: {e}"))?;

    let task = twerk_core::task::Task {
        id: Some(uuid::Uuid::new_v4().to_string().into()),
        job_id: job.id.clone(),
        state: twerk_core::task::TASK_STATE_PENDING.to_string(),
        position: job_position,
        created_at: Some(now),
        started_at: None,
        completed_at: None,
        failed_at: None,
        result: None,
        error: None,
        ..evaluated_task
    };

    ds.create_task(&task).await
        .map_err(|e| anyhow::anyhow!("failed to create task: {e}"))?;

    debug!("Restarted task {} for job {} at position {}", task_id_str(&task), job_id, job_position);
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await?;
    Ok(())
}

/// Completes a job and handles parent/sibling task coordination.
async fn complete_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    info!("Completing job {}", job_id_str(&job));
    let now = time::OffsetDateTime::now_utc();
    let job_id = job.id.clone().unwrap_or_default();
    
    ds.update_job(&job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_COMPLETED.to_string();
        u.completed_at = Some(now);
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update job: {e}"))?;

    let updated_job = ds.get_job_by_id(&job_id).await
        .map_err(|e| anyhow::anyhow!("failed to get job: {e}"))?;
    fire_job_webhooks(&updated_job, "job.Completed");

    match &job.parent_id {
        Some(parent_id) => {
            let parent = ds.get_task_by_id(parent_id).await
                .map_err(|e| anyhow::anyhow!("failed to get parent task: {e}"))?;
            let completed_parent = twerk_core::task::Task {
                state: twerk_core::task::TASK_STATE_COMPLETED.to_string(),
                completed_at: Some(now),
                ..parent
            };
            broker.publish_task(QUEUE_COMPLETED.to_string(), &completed_parent).await?;
        }
        None => {
            broker.publish_event(TOPIC_JOB_COMPLETED.to_string(), serde_json::to_value(&job)?).await?;
        }
    }
    Ok(())
}

/// Handles task completion and triggers next task or job completion.
pub async fn handle_task_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let task_id = task.id.clone().unwrap_or_default();
    let job_id = task.job_id.clone().unwrap_or_default();
    let parent_id = task.parent_id.clone();
    let completed_at = task.completed_at;
    let result = task.result.clone();
    
    ds.update_task(&task_id, Box::new(move |mut u| {
        u.state = twerk_core::task::TASK_STATE_COMPLETED.to_string();
        u.completed_at = completed_at;
        u.result = result;
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update task: {e}"))?;
    
    fire_task_webhooks(ds.clone(), &task, "task.Completed").await;

    match parent_id {
        Some(pid) => handle_subtask_completed(ds, broker, task, &pid).await,
        None => handle_top_level_task_completed(ds, broker, job_id.to_string()).await,
    }
}

/// Advances job to next task or marks job complete.
async fn handle_top_level_task_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: String,
) -> Result<()> {
    let job = ds.get_job_by_id(&job_id).await
        .map_err(|e| anyhow::anyhow!("failed to get job: {e}"))?;
    let next_position = job.position + 1;
    let now = time::OffsetDateTime::now_utc();
    
    let tasks = job.tasks.as_ref();
    let has_more_tasks = tasks.is_some_and(|t| next_position <= t.len() as i64);
    
    if has_more_tasks {
        let next_task_index = (next_position - 1) as usize;
        let base_task = tasks.and_then(|t| t.get(next_task_index));
        
        if let Some(base) = base_task {
            let task = twerk_core::task::Task {
                id: Some(uuid::Uuid::new_v4().to_string().into()),
                job_id: Some(job_id.clone().into()),
                state: twerk_core::task::TASK_STATE_PENDING.to_string(),
                position: next_position,
                created_at: Some(now),
                ..base.clone()
            };
            
            ds.create_task(&task).await?;
            ds.update_job(&job_id, Box::new(move |mut u| {
                u.position = next_position;
                Ok(u)
            })).await?;
            broker.publish_task(QUEUE_PENDING.to_string(), &task).await?;
        }
    } else {
        let completed_job = twerk_core::job::Job {
            state: twerk_core::job::JOB_STATE_COMPLETED.to_string(),
            ..job
        };
        broker.publish_job(&completed_job).await?;
    }
    Ok(())
}

/// Handles subtask completion based on parent task type.
async fn handle_subtask_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
    parent_id: &str,
) -> Result<()> {
    let parent = ds.get_task_by_id(parent_id).await?;
    
    if parent.parallel.is_some() {
        handle_parallel_subtask_completed(ds, broker, task, parent).await
    } else if parent.each.is_some() {
        handle_each_subtask_completed(ds, broker, task, parent).await
    } else {
        Ok(())
    }
}

/// Handles completion of a subtask in a parallel task group.
async fn handle_parallel_subtask_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    _task: twerk_core::task::Task,
    parent: twerk_core::task::Task,
) -> Result<()> {
    let parent_id = parent.id.clone().unwrap_or_default();
    let is_last = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let is_last_clone = is_last.clone();
    
    ds.update_task(&parent_id, Box::new(move |mut u| {
        if let Some(ref mut p) = u.parallel {
            p.completions += 1;
            if let Some(ref tasks) = p.tasks {
                is_last_clone.store(p.completions >= tasks.len() as i64, std::sync::atomic::Ordering::SeqCst);
            }
        }
        Ok(u)
    })).await?;
    
    if is_last.load(std::sync::atomic::Ordering::SeqCst) {
        let completed_parent = twerk_core::task::Task {
            state: twerk_core::task::TASK_STATE_COMPLETED.to_string(),
            completed_at: Some(time::OffsetDateTime::now_utc()),
            ..parent
        };
        broker.publish_task(QUEUE_COMPLETED.to_string(), &completed_parent).await?;
    }
    Ok(())
}

/// Handles completion of a subtask in an each-loop task group.
async fn handle_each_subtask_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    _task: twerk_core::task::Task,
    parent: twerk_core::task::Task,
) -> Result<()> {
    let parent_id = parent.id.clone().unwrap_or_default();
    let is_last = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let is_last_clone = is_last.clone();
    
    ds.update_task(&parent_id, Box::new(move |mut u| {
        if let Some(ref mut e) = u.each {
            e.completions += 1;
            is_last_clone.store(e.completions >= e.size, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(u)
    })).await?;
    
    if is_last.load(std::sync::atomic::Ordering::SeqCst) {
        let completed_parent = twerk_core::task::Task {
            state: twerk_core::task::TASK_STATE_COMPLETED.to_string(),
            completed_at: Some(time::OffsetDateTime::now_utc()),
            ..parent
        };
        broker.publish_task(QUEUE_COMPLETED.to_string(), &completed_parent).await?;
    }
    Ok(())
}

/// Creates and publishes a retry task for a failed task.
async fn create_retry_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
    job: &twerk_core::job::Job,
    now: time::OffsetDateTime,
) -> Result<()> {
    let retry_config = task.retry.as_ref()
        .ok_or_else(|| anyhow::anyhow!("task has no retry config"))?;
    
    let retry_task = twerk_core::task::Task {
        id: Some(uuid::Uuid::new_v4().to_string().into()),
        created_at: Some(now),
        state: twerk_core::task::TASK_STATE_PENDING.to_string(),
        error: None,
        failed_at: None,
        retry: Some(twerk_core::task::TaskRetry {
            attempts: retry_config.attempts + 1,
            limit: retry_config.limit,
        }),
        ..task
    };
    
    let job_ctx = build_job_context(job);
    let evaluated = evaluate_task(&retry_task, &job_ctx)
        .map_err(|e| anyhow::anyhow!("failed to evaluate retry task: {e}"))?;
    
    let final_task = twerk_core::task::Task {
        retry: Some(twerk_core::task::TaskRetry {
            attempts: retry_config.attempts + 1,
            limit: retry_config.limit,
        }),
        ..evaluated
    };
    
    ds.create_task(&final_task).await
        .map_err(|e| anyhow::anyhow!("failed to create retry task: {e}"))?;
    
    broker.publish_task(QUEUE_PENDING.to_string(), &final_task).await?;
    Ok(())
}

/// Handles task error event.
pub async fn handle_error(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    error!("Task {} failed: {}", task_id_str(&task), task.error.as_deref().unwrap_or("unknown error"));
    let task_id = task.id.clone().unwrap_or_default();
    let job_id = task.job_id.clone().unwrap_or_default();
    let now = time::OffsetDateTime::now_utc();
    let task_error = task.error.clone();
    let task_result = task.result.clone();
    
    ds.update_task(&task_id, Box::new(move |mut u| {
        u.state = twerk_core::task::TASK_STATE_FAILED.to_string();
        u.failed_at = Some(now);
        u.error.clone_from(&task_error);
        u.result.clone_from(&task_result);
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update task: {e}"))?;

    let job = ds.get_job_by_id(&job_id).await
        .map_err(|e| anyhow::anyhow!("failed to get job: {e}"))?;

    // Early return if job is not active
    if !is_job_active(&job.state) {
        let failed_task = twerk_core::task::Task {
            state: twerk_core::task::TASK_STATE_FAILED.to_string(),
            ..task
        };
        broker.publish_task(QUEUE_FAILED.to_string(), &failed_task).await?;
        fire_task_webhooks(ds, &failed_task, "task.Error").await;
        return Ok(());
    }

    // Try to create retry task if retry is available
    if let Some(ref retry) = task.retry {
        if can_retry(retry) {
            create_retry_task(ds, broker, task, &job, now).await?;
            return Ok(());
        }
    }

    // No retry available, mark as failed
    let failed_task = twerk_core::task::Task {
        state: twerk_core::task::TASK_STATE_FAILED.to_string(),
        ..task
    };
    broker.publish_task(QUEUE_FAILED.to_string(), &failed_task).await?;
    fire_task_webhooks(ds, &failed_task, "task.Error").await;
    Ok(())
}

/// Handles node heartbeat.
pub async fn handle_heartbeat(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    node: twerk_core::node::Node,
) -> Result<()> {
    debug!("Received heartbeat from node {}", node.name.as_deref().unwrap_or("unknown"));
    
    match &node.id {
        Some(node_id) => {
            let node_id_str = node_id.to_string();
            ds.update_node(&node_id_str, Box::new(move |mut u| {
                u.last_heartbeat_at = node.last_heartbeat_at;
                u.cpu_percent = node.cpu_percent;
                u.task_count = node.task_count;
                u.status = Some(NodeStatus::UP);
                Ok(u)
            })).await.map_err(|e| anyhow::anyhow!("failed to update node: {e}"))?;
        }
        None => {
            warn!("Received heartbeat from node without ID, creating new node");
            let new_node = twerk_core::node::Node {
                status: Some(NodeStatus::UP),
                last_heartbeat_at: Some(time::OffsetDateTime::now_utc()),
                ..node
            };
            ds.create_node(&new_node).await
                .map_err(|e| anyhow::anyhow!("failed to create node: {e}"))?;
        }
    }
    Ok(())
}

/// Cancels the parent job of a cancelled subjob.
async fn cancel_parent_job(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    parent_id: &str,
) -> Result<(), JobHandlerError> {
    let parent_task = ds.get_task_by_id(parent_id).await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    let parent_job_id = parent_task.job_id.clone().unwrap_or_default();
    let parent_job = ds.get_job_by_id(&parent_job_id).await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    let cancelled_job = twerk_core::job::Job {
        state: JOB_STATE_CANCELLED.to_string(),
        ..parent_job
    };
    broker.publish_job(&cancelled_job).await
        .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
    Ok(())
}

/// Cancels the affinity (subjob or node) associated with a task.
async fn cancel_task_affinity(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    task: &twerk_core::task::Task,
) -> Result<(), JobHandlerError> {
    match &task.subjob {
        Some(subjob) => {
            if let Some(ref subjob_id) = subjob.id {
                let job_to_cancel = ds.get_job_by_id(subjob_id).await
                    .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
                let cancelled_job = twerk_core::job::Job {
                    state: JOB_STATE_CANCELLED.to_string(),
                    ..job_to_cancel
                };
                broker.publish_job(&cancelled_job).await
                    .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
            }
        }
        None => {
            if let Some(ref node_id) = task.node_id {
                let node = ds.get_node_by_id(node_id).await
                    .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
                let queue = node.queue.unwrap_or_else(|| QUEUE_PENDING.to_string());
                broker.publish_task(queue, task).await
                    .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
            }
        }
    }
    Ok(())
}

/// Cancels all active tasks for a job.
async fn cancel_active_tasks(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: &str,
) -> Result<(), JobHandlerError> {
    let tasks = ds.get_active_tasks(job_id).await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;

    for task in tasks {
        let task_id = task.id.clone().unwrap_or_default();
        ds.update_task(&task_id, Box::new(|mut u| {
            u.state = TASK_STATE_CANCELLED.to_string();
            Ok(u)
        })).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;

        cancel_task_affinity(ds, broker, &task).await?;
    }

    Ok(())
}
