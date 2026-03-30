//! Event handlers for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use tracing::{debug, error, warn};
use twerk_infrastructure::broker::queue::{QUEUE_COMPLETED, QUEUE_FAILED, QUEUE_PENDING};
use crate::engine::{TOPIC_JOB_COMPLETED, TOPIC_JOB_FAILED};
use crate::engine::coordinator::scheduler::Scheduler;
use crate::engine::coordinator::webhook::{fire_job_webhooks, fire_task_webhooks};
use twerk_core::eval::evaluate_task;
use twerk_core::job::{JOB_STATE_CANCELLED, JOB_STATE_FAILED, JOB_STATE_RUNNING, JOB_STATE_PENDING, JOB_STATE_COMPLETED, JOB_STATE_RESTART, JOB_STATE_SCHEDULED};
use twerk_core::task::{TaskLogPart, TASK_STATE_CANCELLED, TASK_STATE_FAILED, TASK_STATE_RUNNING, TASK_STATE_SKIPPED, TASK_STATE_PENDING, TASK_STATE_COMPLETED};
use twerk_core::node::NodeStatus;
use crate::engine::types::JobHandlerError;

// ── Calculations (Pure) ────────────────────────────────────────

/// Extracts job ID string safely.
fn job_id_str(job: &twerk_core::job::Job) -> &str {
    job.id.as_deref().map_or("unknown", |id| id)
}

/// Extracts task ID string safely.
fn task_id_str(task: &twerk_core::task::Task) -> &str {
    task.id.as_deref().map_or("unknown", |id| id)
}

/// Builds job context from job, merging inputs.
fn build_job_context(job: &twerk_core::job::Job) -> std::collections::HashMap<String, serde_json::Value> {
    job.context.as_ref()
        .map(twerk_core::job::JobContext::as_map)
        .map(|mut ctx| {
            job.context.as_ref()
                .and_then(|c| c.inputs.as_ref())
                .into_iter()
                .flatten()
                .for_each(|(k, v)| {
                    ctx.insert(k.clone(), serde_json::Value::String(v.clone()));
                });
            ctx
        })
        .unwrap_or_default()
}

/// Checks if job is in an active state.
fn is_job_active(job_state: &str) -> bool {
    matches!(job_state, JOB_STATE_RUNNING | JOB_STATE_SCHEDULED)
}

/// Checks if retry is available.
fn can_retry(retry: &twerk_core::task::TaskRetry) -> bool {
    retry.attempts < retry.limit
}

/// Checks if task should be skipped.
fn should_skip_task(task: &twerk_core::task::Task) -> bool {
    task.r#if
        .as_ref()
        .is_some_and(|s| s.trim() == "false")
}

// ── Actions ────────────────────────────────────────────────────

/// Skips a task by marking it as SKIPPED.
async fn skip_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let now = time::OffsetDateTime::now_utc();
    let task_id = task.id.as_deref().ok_or_else(|| anyhow!("task has no id"))?;
    
    ds.update_task(task_id, Box::new(move |mut u| {
        u.state = TASK_STATE_SKIPPED.to_string();
        u.scheduled_at = Some(now);
        u.started_at = Some(now);
        u.completed_at = Some(now);
        Ok(u)
    })).await?;
    
    let mut skipped_task = task;
    skipped_task.state = TASK_STATE_SKIPPED.to_string();
    skipped_task.scheduled_at = Some(now);
    skipped_task.started_at = Some(now);
    skipped_task.completed_at = Some(now);
    
    broker.publish_task(QUEUE_COMPLETED.to_string(), &skipped_task).await
}

/// Handles job events from the broker.
///
/// # Errors
/// Returns error if job handling logic fails.
pub async fn handle_job_event(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    debug!(job_id = job_id_str(&job), state = %job.state, "Handling job event");
    
    let res = match job.state.as_str() {
        JOB_STATE_PENDING => start_job(ds, broker, job).await,
        JOB_STATE_COMPLETED => complete_job(ds, broker, job).await,
        JOB_STATE_RESTART => restart_job(ds, broker, job).await,
        JOB_STATE_CANCELLED => handle_cancel(ds, broker, job).await.map_err(|e| anyhow!("{e}")),
        JOB_STATE_FAILED => fail_job(ds, broker, job).await.map_err(|e| anyhow!("{e}")),
        JOB_STATE_RUNNING => mark_job_as_running(ds, broker, job).await.map_err(|e| anyhow!("{e}")),
        _ => Ok(()),
    };

    if let Err(ref e) = res {
        error!(error = %e, "failed to handle job event");
    }
    res
}

/// Handles task progress updates.
///
/// # Errors
/// Returns error if task update fails.
pub async fn handle_task_progress(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    match task.state.as_str() {
        TASK_STATE_PENDING => handle_pending_task(ds, broker, task).await,
        TASK_STATE_COMPLETED => handle_task_completed(ds, broker, task).await,
        TASK_STATE_FAILED => handle_error(ds, broker, task).await,
        _ => {
            let task_id = task.id.as_deref().ok_or_else(|| anyhow!("task has no id"))?;
            ds.update_task(task_id, Box::new(move |mut u| {
                u.state.clone_from(&task.state);
                u.started_at = task.started_at;
                u.completed_at = task.completed_at;
                u.failed_at = task.failed_at;
                u.result.clone_from(&task.result);
                u.error.clone_from(&task.error);
                Ok(u)
            })).await.map_err(anyhow::Error::from)
        }
    }
}

/// Handles pending task by scheduling it.
///
/// # Errors
/// Returns error if scheduling fails.
pub async fn handle_pending_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    if should_skip_task(&task) {
        skip_task(ds, broker, task).await
    } else {
        Scheduler::new(ds, broker).schedule_task(task).await
    }
}

/// Handles redelivered tasks.
///
/// # Errors
/// Returns error if task publishing fails.
pub async fn handle_redelivered(
    _ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    mut task: twerk_core::task::Task,
) -> Result<()> {
    task.redelivered += 1;
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await
}

/// Handles task started event.
///
/// # Errors
/// Returns error if datastore update fails.
pub async fn handle_started(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let task_id = task.id.as_deref().ok_or_else(|| anyhow!("task has no id"))?;
    let now = time::OffsetDateTime::now_utc();
    
    ds.update_task(task_id, Box::new(move |mut u| {
        u.state = TASK_STATE_RUNNING.to_string();
        u.started_at = Some(now);
        Ok(u)
    })).await?;
    
    let _ = fire_task_webhooks(ds, &task, "task.Started").await;
    Ok(())
}

/// Handles task log part.
///
/// # Errors
/// Returns error if log creation fails.
pub async fn handle_log_part(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    part: TaskLogPart,
) -> Result<()> {
    ds.create_task_log_part(&part).await.map_err(anyhow::Error::from)
}

/// Handles job cancellation.
///
/// # Errors
/// Returns error if cancellation logic fails.
pub async fn handle_cancel(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<(), JobHandlerError> {
    let job_id = job.id.as_deref().ok_or_else(|| JobHandlerError::Handler("job has no id".to_string()))?;

    if is_job_active(&job.state) {
        ds.update_job(job_id, Box::new(|mut u| {
            u.state = JOB_STATE_CANCELLED.to_string();
            Ok(u)
        })).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    }

    if let Some(ref parent_id) = job.parent_id {
        cancel_parent_job(&ds, &broker, parent_id).await?;
    }

    cancel_active_tasks(&ds, &broker, job_id).await
}

// ── Private Actions ───────────────────────────────────────────

async fn start_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    let tasks = job.tasks.as_ref().ok_or_else(|| anyhow!("job has no tasks"))?;
    let base_task = tasks.first().ok_or_else(|| anyhow!("job has no tasks"))?;
    
    let now = time::OffsetDateTime::now_utc();
    let job_ctx = build_job_context(&job);
    let job_id = job.id.as_ref().ok_or_else(|| anyhow!("job has no id"))?;
    
    let mut task = evaluate_task(base_task, &job_ctx).map_err(|e| anyhow!("{e}"))?;
    task.id = Some(uuid::Uuid::new_v4().to_string().into());
    task.job_id = Some(job_id.clone());
    task.state = TASK_STATE_PENDING.to_string();
    task.position = 1;
    task.created_at = Some(now);
    
    ds.create_task(&task).await?;
    
    ds.update_job(job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_SCHEDULED.to_string();
        u.started_at = Some(now);
        u.position = 1;
        Ok(u)
    })).await?;

    fire_job_webhooks(&job, "job.Scheduled");
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await
}

async fn restart_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    let job_id = job.id.as_ref().ok_or_else(|| anyhow!("job has no id"))?;
    let now = time::OffsetDateTime::now_utc();

    ds.update_job(job_id, Box::new(move |mut u| {
        u.state = JOB_STATE_RUNNING.to_string();
        u.failed_at = None;
        Ok(u)
    })).await?;

    let tasks = job.tasks.as_ref().ok_or_else(|| anyhow!("job has no tasks"))?;
    let task_index = (job.position - 1) as usize;
    let base_task = tasks.get(task_index).ok_or_else(|| anyhow!("job position out of bounds"))?;
    
    let mut task = evaluate_task(base_task, &build_job_context(&job)).map_err(|e| anyhow!("{e}"))?;
    task.id = Some(uuid::Uuid::new_v4().to_string().into());
    task.job_id = Some(job_id.clone());
    task.state = TASK_STATE_PENDING.to_string();
    task.position = job.position;
    task.created_at = Some(now);

    ds.create_task(&task).await?;
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await
}

async fn complete_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    let now = time::OffsetDateTime::now_utc();
    let job_id = job.id.as_ref().ok_or_else(|| anyhow!("job has no id"))?;
    
    ds.update_job(job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_COMPLETED.to_string();
        u.completed_at = Some(now);
        Ok(u)
    })).await?;

    let updated_job = ds.get_job_by_id(job_id).await?;
    fire_job_webhooks(&updated_job, "job.Completed");

    match &job.parent_id {
        Some(parent_id) => {
            let mut parent = ds.get_task_by_id(parent_id).await?;
            parent.state = TASK_STATE_COMPLETED.to_string();
            parent.completed_at = Some(now);
            broker.publish_task(QUEUE_COMPLETED.to_string(), &parent).await
        }
        None => {
            broker.publish_event(TOPIC_JOB_COMPLETED.to_string(), serde_json::to_value(&job)?).await
        }
    }
}

async fn mark_job_as_running(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<(), JobHandlerError> {
    let job_id = job.id.as_deref().ok_or_else(|| JobHandlerError::Handler("job has no id".to_string()))?;
    
    ds.update_job(job_id, Box::new(move |mut u| {
        if u.state == twerk_core::job::JOB_STATE_SCHEDULED {
            u.state = JOB_STATE_RUNNING.to_string();
            u.failed_at = None;
        }
        Ok(u)
    })).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))
}

async fn fail_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<(), JobHandlerError> {
    let job_id = job.id.as_deref().ok_or_else(|| JobHandlerError::Handler("job has no id".to_string()))?;
    let failed_at = job.failed_at;
    
    ds.update_job(job_id, Box::new(move |mut u| {
        if is_job_active(&u.state) {
            u.state = JOB_STATE_FAILED.to_string();
            u.failed_at = failed_at;
        }
        Ok(u)
    })).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    
    if let Some(ref parent_id) = job.parent_id {
        let mut parent = ds.get_task_by_id(parent_id).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
        parent.state = TASK_STATE_FAILED.to_string();
        parent.failed_at = failed_at;
        parent.error.clone_from(&job.error);
        broker.publish_task(QUEUE_FAILED.to_string(), &parent).await.map_err(|e| JobHandlerError::Handler(e.to_string()))?;
    }
    
    cancel_active_tasks(&ds, &broker, job_id).await?;
    
    let updated_job = ds.get_job_by_id(job_id).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    if updated_job.state == JOB_STATE_FAILED {
        broker.publish_event(TOPIC_JOB_FAILED.to_string(), serde_json::to_value(&updated_job).map_err(|e| JobHandlerError::Handler(e.to_string()))?).await
            .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
    }
    
    Ok(())
}

/// Handles task completion.
///
/// # Errors
/// Returns error if task update or next step scheduling fails.
pub async fn handle_task_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let task_id = task.id.as_deref().ok_or_else(|| anyhow!("task has no id"))?;
    let completed_at = task.completed_at;
    let result = task.result.clone();
    
    ds.update_task(task_id, Box::new(move |mut u| {
        u.state = TASK_STATE_COMPLETED.to_string();
        u.completed_at = completed_at;
        u.result = result;
        Ok(u)
    })).await?;
    
    let _ = fire_task_webhooks(ds.clone(), &task, "task.Completed").await;

    if let Some(pid) = task.parent_id.clone() {
        handle_subtask_completed(ds, broker, task, pid.as_str()).await
    } else {
        let job_id = task.job_id.as_deref().ok_or_else(|| anyhow!("task has no job_id"))?;
        handle_top_level_task_completed(ds, broker, job_id.to_string()).await
    }
}

async fn handle_top_level_task_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: String,
) -> Result<()> {
    let job = ds.get_job_by_id(&job_id).await?;
    let next_position = job.position + 1;
    let now = time::OffsetDateTime::now_utc();
    
    let tasks = job.tasks.as_ref().ok_or_else(|| anyhow!("job has no tasks"))?;
    
    if next_position <= tasks.len() as i64 {
        let base_task = tasks.get((next_position - 1) as usize).ok_or_else(|| anyhow!("task out of bounds"))?;
        let mut task = base_task.clone();
        task.id = Some(uuid::Uuid::new_v4().to_string().into());
        task.job_id = Some(job_id.clone().into());
        task.state = TASK_STATE_PENDING.to_string();
        task.position = next_position;
        task.created_at = Some(now);
        
        ds.create_task(&task).await?;
        ds.update_job(&job_id, Box::new(move |mut u| {
            u.position = next_position;
            Ok(u)
        })).await?;
        broker.publish_task(QUEUE_PENDING.to_string(), &task).await
    } else {
        let mut completed_job = job;
        completed_job.state = JOB_STATE_COMPLETED.to_string();
        broker.publish_job(&completed_job).await
    }
}

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

async fn handle_parallel_subtask_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    _task: twerk_core::task::Task,
    parent: twerk_core::task::Task,
) -> Result<()> {
    let parent_id = parent.id.as_deref().ok_or_else(|| anyhow!("parent task has no id"))?;
    let is_last = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let is_last_clone = is_last.clone();
    
    ds.update_task(parent_id, Box::new(move |mut u| {
        if let Some(ref mut p) = u.parallel {
            p.completions += 1;
            if let Some(ref tasks) = p.tasks {
                is_last_clone.store(p.completions >= tasks.len() as i64, std::sync::atomic::Ordering::SeqCst);
            }
        }
        Ok(u)
    })).await?;
    
    if is_last.load(std::sync::atomic::Ordering::SeqCst) {
        let mut completed_parent = parent;
        completed_parent.state = TASK_STATE_COMPLETED.to_string();
        completed_parent.completed_at = Some(time::OffsetDateTime::now_utc());
        broker.publish_task(QUEUE_COMPLETED.to_string(), &completed_parent).await
    } else {
        Ok(())
    }
}

async fn handle_each_subtask_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    _task: twerk_core::task::Task,
    parent: twerk_core::task::Task,
) -> Result<()> {
    let parent_id = parent.id.as_deref().ok_or_else(|| anyhow!("parent task has no id"))?;
    let is_last = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let is_last_clone = is_last.clone();
    
    ds.update_task(parent_id, Box::new(move |mut u| {
        if let Some(ref mut e) = u.each {
            e.completions += 1;
            is_last_clone.store(e.completions >= e.size, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(u)
    })).await?;
    
    if is_last.load(std::sync::atomic::Ordering::SeqCst) {
        let mut completed_parent = parent;
        completed_parent.state = TASK_STATE_COMPLETED.to_string();
        completed_parent.completed_at = Some(time::OffsetDateTime::now_utc());
        broker.publish_task(QUEUE_COMPLETED.to_string(), &completed_parent).await
    } else {
        Ok(())
    }
}

async fn create_retry_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
    job: &twerk_core::job::Job,
    now: time::OffsetDateTime,
) -> Result<()> {
    let retry_config = task.retry.clone().ok_or_else(|| anyhow!("task has no retry config"))?;
    
    let mut retry_task = task;
    retry_task.id = Some(uuid::Uuid::new_v4().to_string().into());
    retry_task.created_at = Some(now);
    retry_task.state = TASK_STATE_PENDING.to_string();
    retry_task.error = None;
    retry_task.failed_at = None;
    retry_task.retry = Some(twerk_core::task::TaskRetry {
        attempts: retry_config.attempts + 1,
        limit: retry_config.limit,
    });
    
    let job_ctx = build_job_context(job);
    let final_task = evaluate_task(&retry_task, &job_ctx).map_err(|e| anyhow!("{e}"))?;
    
    ds.create_task(&final_task).await?;
    broker.publish_task(QUEUE_PENDING.to_string(), &final_task).await
}

fn handle_subtask_failed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
    parent_id: String,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
    Box::pin(async move {
        let parent = ds.get_task_by_id(&parent_id).await?;
        
        if parent.parallel.is_some() || parent.each.is_some() {
            let mut failed_parent = parent;
            failed_parent.state = TASK_STATE_FAILED.to_string();
            failed_parent.failed_at = Some(time::OffsetDateTime::now_utc());
            failed_parent.error = task.error.clone();
            
            // When a subtask fails, we fail the parent immediately
            handle_task_failed(ds, broker, failed_parent).await
        } else {
            Ok(())
        }
    })
}

/// Handles task failure.
///
/// # Errors
/// Returns error if task update or next step fail.
pub async fn handle_task_failed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let task_id = task.id.as_deref().ok_or_else(|| anyhow!("task has no id"))?;
    let failed_at = task.failed_at;
    let error = task.error.clone();
    
    ds.update_task(task_id, Box::new(move |mut u| {
        u.state = TASK_STATE_FAILED.to_string();
        u.failed_at = failed_at;
        u.error = error;
        Ok(u)
    })).await?;
    
    let _ = fire_task_webhooks(ds.clone(), &task, "task.Failed").await;

    if let Some(pid) = task.parent_id.clone() {
        handle_subtask_failed(ds, broker, task, pid.to_string()).await
    } else {
        let job_id = task.job_id.as_deref().ok_or_else(|| anyhow!("task has no job_id"))?;
        handle_top_level_task_failed(ds, broker, job_id.to_string(), task.error.clone()).await
    }
}

async fn handle_top_level_task_failed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: String,
    task_error: Option<String>,
) -> Result<()> {
    let mut job = ds.get_job_by_id(&job_id).await?;
    job.state = JOB_STATE_FAILED.to_string();
    job.failed_at = Some(time::OffsetDateTime::now_utc());
    job.error = task_error;
    
    broker.publish_job(&job).await
}

/// Handles task error event.
///
/// # Errors
/// Returns error if task update or retry logic fails.
pub async fn handle_error(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    error!(task_id = task_id_str(&task), error = task.error.as_deref().unwrap_or("unknown error"), "Task failed");
    let task_id = task.id.as_deref().ok_or_else(|| anyhow!("task has no id"))?;
    let job_id = task.job_id.as_deref().ok_or_else(|| anyhow!("task has no job_id"))?;
    let now = time::OffsetDateTime::now_utc();
    let task_error = task.error.clone();
    let task_result = task.result.clone();
    
    ds.update_task(task_id, Box::new(move |mut u| {
        u.state = TASK_STATE_FAILED.to_string();
        u.failed_at = Some(now);
        u.error.clone_from(&task_error);
        u.result.clone_from(&task_result);
        Ok(u)
    })).await?;

    let job = ds.get_job_by_id(job_id).await?;

    if !is_job_active(&job.state) {
        let _ = fire_task_webhooks(ds, &task, "task.Error").await;
        return Ok(());
    }

    if let Some(ref retry) = task.retry {
        if can_retry(retry) {
            return create_retry_task(ds, broker, task, &job, now).await;
        }
    }

    let mut failed_task = task.clone();
    failed_task.state = TASK_STATE_FAILED.to_string();
    failed_task.failed_at = Some(now);
    broker.publish_task(QUEUE_FAILED.to_string(), &failed_task).await?;

    handle_task_failed(ds, broker, failed_task).await
}

/// Handles node heartbeat.
///
/// # Errors
/// Returns error if node update fails.
pub async fn handle_heartbeat(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    node: twerk_core::node::Node,
) -> Result<()> {
    if let Some(node_id) = &node.id {
        let node_id_str = node_id.to_string();
        ds.update_node(&node_id_str, Box::new(move |mut u: twerk_core::node::Node| {
            u.last_heartbeat_at = node.last_heartbeat_at;
            u.cpu_percent = node.cpu_percent;
            u.task_count = node.task_count;
            u.status = Some(NodeStatus::UP);
            Ok(u)
        }))
        .await
        .map_err(anyhow::Error::from)
    } else {
        warn!("Received heartbeat from node without ID, creating new node");
        let mut new_node = node;
        new_node.status = Some(NodeStatus::UP);
        new_node.last_heartbeat_at = Some(time::OffsetDateTime::now_utc());
        ds.create_node(&new_node).await.map_err(anyhow::Error::from)
    }
}

async fn cancel_parent_job(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    parent_id: &str,
) -> Result<(), JobHandlerError> {
    let parent_task = ds.get_task_by_id(parent_id).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    let parent_job_id = parent_task.job_id.as_deref().ok_or_else(|| JobHandlerError::Handler("parent task has no job_id".to_string()))?;
    let parent_job = ds.get_job_by_id(parent_job_id).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    let mut cancelled_job = parent_job;
    cancelled_job.state = JOB_STATE_CANCELLED.to_string();
    broker.publish_job(&cancelled_job).await.map_err(|e| JobHandlerError::Handler(e.to_string()))
}

async fn cancel_task_affinity(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    task: &twerk_core::task::Task,
) -> Result<(), JobHandlerError> {
    match &task.subjob {
        Some(subjob) => {
            let subjob_id = subjob.id.as_deref().ok_or_else(|| JobHandlerError::Handler("subjob has no id".to_string()))?;
            let job_to_cancel = ds.get_job_by_id(subjob_id).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
            let mut cancelled_job = job_to_cancel;
            cancelled_job.state = JOB_STATE_CANCELLED.to_string();
            broker.publish_job(&cancelled_job).await.map_err(|e| JobHandlerError::Handler(e.to_string()))
        }
        None => {
            if let Some(ref node_id) = task.node_id {
                let node = ds.get_node_by_id(node_id).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
                let queue = node.queue.unwrap_or_else(|| QUEUE_PENDING.to_string());
                broker.publish_task(queue, task).await.map_err(|e| JobHandlerError::Handler(e.to_string()))
            } else {
                Ok(())
            }
        }
    }
}

async fn cancel_active_tasks(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: &str,
) -> Result<(), JobHandlerError> {
    let tasks = ds.get_active_tasks(job_id).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;

    for task in tasks {
        let task_id = task.id.as_deref().ok_or_else(|| JobHandlerError::Handler("task has no id".to_string()))?;
        ds.update_task(task_id, Box::new(|mut u| {
            u.state = TASK_STATE_CANCELLED.to_string();
            Ok(u)
        })).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;

        cancel_task_affinity(ds, broker, &task).await?;
    }

    Ok(())
}
