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
use twerk_core::job::{JOB_STATE_CANCELLED, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED};
use twerk_core::task::{TaskLogPart, TASK_STATE_CANCELLED, TASK_STATE_RUNNING};
use twerk_core::node::NodeStatus;
use crate::engine::types::JobHandlerError;

/// Handles job events from the broker.
/// # Errors
/// Returns error if task scheduling or job completion fails.
pub async fn handle_job_event(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    debug!("Handling job event for job {} (state={})", job.id.as_deref().unwrap_or("unknown"), job.state);
    let result = match job.state.as_str() {
        twerk_core::job::JOB_STATE_PENDING => {
            start_job(ds, broker, job).await
        }
        twerk_core::job::JOB_STATE_COMPLETED => {
            complete_job(ds, broker, job).await
        }
        twerk_core::job::JOB_STATE_RESTART => {
            restart_job(ds, broker, job).await
        }
        JOB_STATE_CANCELLED => {
            handle_cancel(ds, broker, job).await.map_err(|e| anyhow::anyhow!("{}", e))
        }
        _ => Ok(())
    };
    if let Err(ref e) = result {
        error!("failed to handle job event: {}", e);
    }
    result
}

async fn start_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    mut job: twerk_core::job::Job,
) -> Result<()> {
    info!("Starting job {}", job.id.as_deref().unwrap_or("unknown"));
    let now = time::OffsetDateTime::now_utc();
    let tasks = job.tasks.as_mut().ok_or_else(|| anyhow::anyhow!("job has no tasks"))?;
    if tasks.is_empty() {
        return Err(anyhow::anyhow!("job has no tasks"));
    }
    
    let mut job_ctx = job.context.as_ref().map(twerk_core::job::JobContext::as_map).unwrap_or_default();
    if let Some(inputs) = job.context.as_ref().and_then(|c| c.inputs.as_ref()) {
        for (k, v) in inputs {
            job_ctx.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
    }
    
    let mut first_task = tasks[0].clone();
    first_task = evaluate_task(&first_task, &job_ctx)
        .map_err(|e| anyhow::anyhow!("failed to evaluate task: {e}"))?;

    first_task.id = Some(uuid::Uuid::new_v4().to_string().into());
    first_task.job_id = job.id.clone();
    first_task.state = twerk_core::task::TASK_STATE_PENDING.to_string();
    first_task.position = 1;
    first_task.created_at = Some(now);
    
    ds.create_task(&first_task).await
        .map_err(|e| anyhow::anyhow!("failed to create task: {e}"))?;
    
    let job_id = job.id.clone().unwrap_or_default();
    debug!("Created first task {} for job {}", first_task.id.as_deref().unwrap_or("unknown"), job_id);
    ds.update_job(&job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_SCHEDULED.to_string();
        u.started_at = Some(now);
        u.position = 1;
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update job: {e}"))?;

    fire_job_webhooks(&job, "job.Scheduled");
    broker.publish_task(QUEUE_PENDING.to_string(), &first_task).await?;
    Ok(())
}

async fn restart_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    info!("Restarting job {}", job.id.as_deref().unwrap_or("unknown"));
    let job_id = job.id.clone().unwrap_or_default();
    let job_position = job.position;
    let now = time::OffsetDateTime::now_utc();

    // Update job state to RUNNING and clear failed_at
    ds.update_job(&job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_RUNNING.to_string();
        u.failed_at = None;
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update job: {e}"))?;

    // Get task at original position
    let tasks = job.tasks.as_ref().ok_or_else(|| anyhow::anyhow!("job has no tasks"))?;
    if tasks.is_empty() {
        return Err(anyhow::anyhow!("job has no tasks"));
    }
    let task_index = (job_position - 1) as usize;
    if task_index >= tasks.len() {
        return Err(anyhow::anyhow!("job position {} out of bounds", job_position));
    }

    let mut task = tasks[task_index].clone();
    
    // Build job context
    let mut job_ctx = job.context.as_ref().map(twerk_core::job::JobContext::as_map).unwrap_or_default();
    if let Some(inputs) = job.context.as_ref().and_then(|c| c.inputs.as_ref()) {
        for (k, v) in inputs {
            job_ctx.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
    }
    
    // Re-evaluate task with job context
    task = evaluate_task(&task, &job_ctx)
        .map_err(|e| anyhow::anyhow!("failed to evaluate task: {e}"))?;

    // Reset task fields for restart
    task.id = Some(uuid::Uuid::new_v4().to_string().into());
    task.job_id = job.id.clone();
    task.state = twerk_core::task::TASK_STATE_PENDING.to_string();
    task.position = job_position;
    task.created_at = Some(now);
    task.started_at = None;
    task.completed_at = None;
    task.failed_at = None;
    task.result = None;
    task.error = None;

    // Create task in datastore
    ds.create_task(&task).await
        .map_err(|e| anyhow::anyhow!("failed to create task: {e}"))?;

    debug!("Restarted task {} for job {} at position {}", task.id.as_deref().unwrap_or("unknown"), job_id, job_position);
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await?;
    Ok(())
}

async fn complete_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    info!("Completing job {}", job.id.as_deref().unwrap_or("unknown"));
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

    if let Some(parent_id) = &job.parent_id {
        let mut parent = ds.get_task_by_id(parent_id).await
            .map_err(|e| anyhow::anyhow!("failed to get parent task: {e}"))?;
        parent.state = twerk_core::task::TASK_STATE_COMPLETED.to_string();
        parent.completed_at = Some(now);
        broker.publish_task(QUEUE_COMPLETED.to_string(), &parent).await?;
    } else {
        broker.publish_event(TOPIC_JOB_COMPLETED.to_string(), serde_json::to_value(&job)?).await?;
    }
    Ok(())
}

/// Handles task progress updates from workers.
/// # Errors
/// Returns error if task update or scheduling fails.
pub async fn handle_task_progress(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    match task.state.as_str() {
        twerk_core::task::TASK_STATE_PENDING => {
            handle_pending_task(ds, broker, task).await
        }
        twerk_core::task::TASK_STATE_COMPLETED => {
            handle_task_completed(ds, broker, task).await
        }
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
/// # Errors
/// Returns error if scheduling fails.
pub async fn handle_pending_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let scheduler = Scheduler::new(ds, broker);
    if let Err(e) = scheduler.schedule_task(task).await {
        error!("failed to schedule task: {}", e);
        return Err(e);
    }
    Ok(())
}

/// Handles task completion and triggers next task or job completion.
/// # Errors
/// Returns error if task update or next task scheduling fails.
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
    fire_task_webhooks(&task, "task.Completed");
    
    if let Some(pid) = parent_id {
        handle_subtask_completed(ds, broker, task, &pid).await
    } else {
        handle_top_level_task_completed(ds, broker, job_id.to_string()).await
    }
}

async fn handle_top_level_task_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: String,
) -> Result<()> {
    let mut job = ds.get_job_by_id(&job_id).await
        .map_err(|e| anyhow::anyhow!("failed to get job: {e}"))?;
    job.position += 1;
    let now = time::OffsetDateTime::now_utc();
    
    if let Some(tasks) = &job.tasks {
        if job.position <= tasks.len() as i64 {
            let mut next_task = tasks[(job.position - 1) as usize].clone();
            next_task.id = Some(uuid::Uuid::new_v4().to_string().into());
            next_task.job_id = Some(job_id.clone().into());
            next_task.state = twerk_core::task::TASK_STATE_PENDING.to_string();
            next_task.position = job.position;
            next_task.created_at = Some(now);
            ds.create_task(&next_task).await?;
            ds.update_job(&job_id, Box::new(move |mut u| {
                u.position = job.position;
                Ok(u)
            })).await?;
            broker.publish_task(QUEUE_PENDING.to_string(), &next_task).await?;
        } else {
            job.state = twerk_core::job::JOB_STATE_COMPLETED.to_string();
            broker.publish_job(&job).await?;
        }
    }
    Ok(())
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
    mut parent: twerk_core::task::Task,
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
        parent.state = twerk_core::task::TASK_STATE_COMPLETED.to_string();
        parent.completed_at = Some(time::OffsetDateTime::now_utc());
        broker.publish_task(QUEUE_COMPLETED.to_string(), &parent).await?;
    }
    Ok(())
}

async fn handle_each_subtask_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    _task: twerk_core::task::Task,
    mut parent: twerk_core::task::Task,
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
        parent.state = twerk_core::task::TASK_STATE_COMPLETED.to_string();
        parent.completed_at = Some(time::OffsetDateTime::now_utc());
        broker.publish_task(QUEUE_COMPLETED.to_string(), &parent).await?;
    }
    Ok(())
}

/// Handles redelivered tasks.
/// # Errors
/// Returns an error if publishing to the broker fails.
pub async fn handle_redelivered(
    _ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    debug!("Handling redelivered task {} (redelivered={})", task.id.as_deref().unwrap_or("unknown"), task.redelivered);
    let mut task = task;
    task.redelivered += 1;
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await?;
    Ok(())
}

/// Handles task started event.
/// # Errors
/// Returns an error if updating the task in the datastore fails.
pub async fn handle_started(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    info!("Task {} started", task.id.as_deref().unwrap_or("unknown"));
    let task_id = task.id.clone().unwrap_or_default();
    let now = time::OffsetDateTime::now_utc();
    ds.update_task(&task_id, Box::new(move |mut u| {
        u.state = TASK_STATE_RUNNING.to_string();
        u.started_at = Some(now);
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update task: {e}"))?;
    fire_task_webhooks(&task, "task.Started");
    Ok(())
}

/// Handles task error event.
/// # Errors
/// Returns an error if updating the task or publishing to the broker fails.
pub async fn handle_error(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    mut task: twerk_core::task::Task,
) -> Result<()> {
    error!("Task {} failed: {}", task.id.as_deref().unwrap_or("unknown"), task.error.as_deref().unwrap_or("unknown error"));
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

    let job_state = job.state.as_str();
    let is_job_active = job_state == twerk_core::job::JOB_STATE_RUNNING || job_state == twerk_core::job::JOB_STATE_SCHEDULED;

    if is_job_active {
        if let Some(ref retry) = task.retry {
            if retry.attempts < retry.limit {
                let mut retry_task = task.clone();
                retry_task.id = Some(uuid::Uuid::new_v4().to_string().into());
                retry_task.created_at = Some(now);
                if let Some(ref mut r) = retry_task.retry {
                    r.attempts += 1;
                }
                retry_task.state = twerk_core::task::TASK_STATE_PENDING.to_string();
                retry_task.error = None;
                retry_task.failed_at = None;

                let mut job_ctx = job.context.as_ref().map(twerk_core::job::JobContext::as_map).unwrap_or_default();
                if let Some(inputs) = job.context.as_ref().and_then(|c| c.inputs.as_ref()) {
                    for (k, v) in inputs {
                        job_ctx.insert(k.clone(), serde_json::Value::String(v.clone()));
                    }
                }

                retry_task = evaluate_task(&retry_task, &job_ctx)
                    .map_err(|e| anyhow::anyhow!("failed to evaluate retry task: {e}"))?;

                ds.create_task(&retry_task).await
                    .map_err(|e| anyhow::anyhow!("failed to create retry task: {e}"))?;

                broker.publish_task(QUEUE_PENDING.to_string(), &retry_task).await?;
                return Ok(());
            }
        }
    }

    task.state = twerk_core::task::TASK_STATE_FAILED.to_string();
    broker.publish_task(QUEUE_FAILED.to_string(), &task).await?;
    fire_task_webhooks(&task, "task.Error");
    Ok(())
}

/// Handles node heartbeat.
/// # Errors
/// Returns an error if updating or creating the node in the datastore fails.
pub async fn handle_heartbeat(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    node: twerk_core::node::Node,
) -> Result<()> {
    debug!("Received heartbeat from node {}", node.name.as_deref().unwrap_or("unknown"));
    if let Some(node_id) = &node.id {
        let node_id_str = node_id.to_string();
        ds.update_node(&node_id_str, Box::new(move |mut u| {
            u.last_heartbeat_at = node.last_heartbeat_at;
            u.cpu_percent = node.cpu_percent;
            u.task_count = node.task_count;
            u.status = Some(NodeStatus::UP);
            Ok(u)
        })).await.map_err(|e| anyhow::anyhow!("failed to update node: {e}"))?;
    } else {
        warn!("Received heartbeat from node without ID, creating new node");
        let mut new_node = node;
        new_node.status = Some(NodeStatus::UP);
        new_node.last_heartbeat_at = Some(time::OffsetDateTime::now_utc());
        ds.create_node(&new_node).await.map_err(|e| anyhow::anyhow!("failed to create node: {e}"))?;
    }
    Ok(())
}

/// Handles task log part.
/// # Errors
/// Returns an error if storing the log part in the datastore fails.
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
/// # Errors
/// Returns error if job update or task cancellation fails.
pub async fn handle_cancel(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<(), JobHandlerError> {
    let job_id = job.id.clone().unwrap_or_default();
    let job_state = job.state.clone();

    if job_state.as_str() == JOB_STATE_RUNNING || job_state.as_str() == JOB_STATE_SCHEDULED {
        ds.update_job(&job_id, Box::new(|mut u| {
            u.state = JOB_STATE_CANCELLED.to_string();
            Ok(u)
        })).await.map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    }

    if let Some(ref parent_id) = job.parent_id {
        let parent_task = ds.get_task_by_id(parent_id).await
            .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
        let parent_job_id = parent_task.job_id.clone().unwrap_or_default();
        let mut parent_job = ds.get_job_by_id(&parent_job_id).await
            .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
        parent_job.state = JOB_STATE_CANCELLED.to_string();
        broker.publish_job(&parent_job).await
            .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
    }

    cancel_active_tasks(&ds, &broker, &job_id).await?;

    Ok(())
}

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

        if let Some(ref subjob) = task.subjob {
            if let Some(ref subjob_id) = subjob.id {
                let mut sub_job = ds.get_job_by_id(subjob_id).await
                    .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
                sub_job.state = JOB_STATE_CANCELLED.to_string();
                broker.publish_job(&sub_job).await
                    .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
            }
        } else if let Some(ref node_id) = task.node_id {
            let node = ds.get_node_by_id(node_id).await
                .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
            let queue = node.queue.unwrap_or_else(|| QUEUE_PENDING.to_string());
            broker.publish_task(queue, &task).await
                .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
        }
    }

    Ok(())
}
