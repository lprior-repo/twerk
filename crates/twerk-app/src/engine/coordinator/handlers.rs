//! Event handlers for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, error, info};
use twerk_infrastructure::broker::queue::{QUEUE_COMPLETED, QUEUE_PENDING};
use crate::engine::TOPIC_JOB_COMPLETED;
use crate::engine::coordinator::scheduler::Scheduler;
use twerk_core::eval::evaluate_task;

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
    
    let mut job_ctx = job.context.as_ref().map(|c| c.as_map()).unwrap_or_default();
    if let Some(inputs) = job.context.as_ref().and_then(|c| c.inputs.as_ref()) {
        for (k, v) in inputs {
            job_ctx.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
    }
    
    let mut first_task = tasks[0].clone();
    first_task = evaluate_task(&first_task, &job_ctx)
        .map_err(|e| anyhow::anyhow!("failed to evaluate task: {}", e))?;

    first_task.id = Some(uuid::Uuid::new_v4().to_string().into());
    first_task.job_id = job.id.clone();
    first_task.state = twerk_core::task::TASK_STATE_PENDING.to_string();
    first_task.position = 1;
    first_task.created_at = Some(now);
    
    ds.create_task(&first_task).await
        .map_err(|e| anyhow::anyhow!("failed to create task: {}", e))?;
    
    let job_id = job.id.clone().unwrap_or_default();
    debug!("Created first task {} for job {}", first_task.id.as_deref().unwrap_or("unknown"), job_id);
    ds.update_job(&job_id, Box::new(move |mut u| {
        u.state = twerk_core::job::JOB_STATE_SCHEDULED.to_string();
        u.started_at = Some(now);
        u.position = 1;
        Ok(u)
    })).await.map_err(|e| anyhow::anyhow!("failed to update job: {}", e))?;
    
    broker.publish_task(QUEUE_PENDING.to_string(), &first_task).await?;
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
    })).await.map_err(|e| anyhow::anyhow!("failed to update job: {}", e))?;
    
    if let Some(parent_id) = &job.parent_id {
        let mut parent = ds.get_task_by_id(parent_id).await
            .map_err(|e| anyhow::anyhow!("failed to get parent task: {}", e))?;
        parent.state = twerk_core::task::TASK_STATE_COMPLETED.to_string();
        parent.completed_at = Some(now);
        broker.publish_task(QUEUE_COMPLETED.to_string(), &parent).await?;
    } else {
        broker.publish_event(TOPIC_JOB_COMPLETED.to_string(), serde_json::to_value(&job)?).await?;
    }
    Ok(())
}

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
                u.state = task.state.clone();
                u.started_at = task.started_at;
                u.completed_at = task.completed_at;
                u.failed_at = task.failed_at;
                u.result = task.result.clone();
                u.error = task.error.clone();
                Ok(u)
            })).await.map_err(|e| anyhow::anyhow!("failed to update task: {}", e))?;
            Ok(())
        }
    }
}

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
    })).await.map_err(|e| anyhow::anyhow!("failed to update task: {}", e))?;
    
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
        .map_err(|e| anyhow::anyhow!("failed to get job: {}", e))?;
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
