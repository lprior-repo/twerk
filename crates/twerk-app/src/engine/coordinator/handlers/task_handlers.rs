//! Task-related event handlers

use super::task_workflow::{handle_top_level_task_completed, handle_top_level_task_failed};
use super::HandlerError;
use crate::engine::coordinator::handlers::retry::create_retry_task;
use crate::engine::coordinator::handlers::subtask_handlers::{
    handle_subtask_completed, handle_subtask_failed,
};
use crate::engine::coordinator::handlers::util::{
    can_retry, is_job_active, should_skip_task, skip_task, task_id_str,
};
use crate::engine::coordinator::scheduler::Scheduler;
use crate::engine::coordinator::webhook::fire_task_webhooks;
use anyhow::Result;
use std::sync::Arc;
use tracing::{error, instrument, warn};
use twerk_core::task::{TaskLogPart, TaskState};
use twerk_infrastructure::broker::queue::QUEUE_FAILED;

// ── Public Task Handlers ────────────────────────────────────────

/// Handles task progress updates.
///
/// # Errors
/// Returns error if task update fails.
#[instrument(name = "handle_task_progress", skip_all, fields(task_id = %task.id.as_deref().unwrap_or("unknown"), state = %task.state))]
pub async fn handle_task_progress(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    match task.state {
        TaskState::Pending => handle_pending_task(ds, broker, task).await,
        TaskState::Completed => handle_task_completed(ds, broker, task).await,
        TaskState::Failed => handle_error(ds, broker, task).await,
        TaskState::Created
        | TaskState::Scheduled
        | TaskState::Running
        | TaskState::Cancelled
        | TaskState::Stopped
        | TaskState::Skipped => persist_task_progress(ds, task).await,
    }
}

/// Handles pending task by scheduling it.
///
/// # Errors
/// Returns error if scheduling fails.
#[instrument(name = "handle_pending_task", skip_all)]
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
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    mut task: twerk_core::task::Task,
) -> Result<()> {
    let task_id = task
        .id
        .as_deref()
        .ok_or(HandlerError::RedeliveredMissingTaskId)?;
    let persisted = ds.get_task_by_id(task_id).await?;
    if matches!(
        persisted.state,
        TaskState::Completed | TaskState::Failed | TaskState::Cancelled
    ) {
        return Ok(());
    }
    task.redelivered += 1;
    ds.update_task(
        task_id,
        Box::new(move |mut current| {
            current.redelivered = task.redelivered;
            Ok(current)
        }),
    )
    .await?;

    let queue = persisted.queue.unwrap_or_else(|| "default".to_string());
    broker.publish_task(queue, &task).await
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
    let task_id = task.id.as_deref().ok_or(HandlerError::MissingTaskId)?;
    let now = time::OffsetDateTime::now_utc();

    ds.update_task(
        task_id,
        Box::new(move |mut u| {
            u.state = TaskState::Running;
            u.started_at = Some(now);
            Ok(u)
        }),
    )
    .await?;

    if let Err(e) = fire_task_webhooks(ds, &task, "task.Started").await {
        warn!(error = %e, "failed to fire task.Started webhook");
    }
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
    ds.create_task_log_part(&part)
        .await
        .map_err(anyhow::Error::from)
}

/// Handles task completion.
///
/// # Errors
/// Returns error if task update or next step scheduling fails.
#[instrument(name = "handle_task_completed", skip_all, fields(task_id = %task.id.as_deref().unwrap_or("unknown")))]
pub async fn handle_task_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    persist_completed_task(ds.clone(), &task).await?;

    if let Err(e) = fire_task_webhooks(ds.clone(), &task, "task.Completed").await {
        warn!(error = %e, "failed to fire task.Completed webhook");
    }

    route_completed_task(ds, broker, task).await
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
    persist_failed_task(ds.clone(), &task).await?;

    if let Err(e) = fire_task_webhooks(ds.clone(), &task, "task.Failed").await {
        warn!(error = %e, "failed to fire task.Failed webhook");
    }

    route_failed_task(ds, broker, task).await
}

/// Handles task error event.
///
/// # Errors
/// Returns error if task update or retry logic fails.
#[instrument(name = "handle_error", skip_all, fields(task_id = %task.id.as_deref().unwrap_or("unknown")))]
pub async fn handle_error(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    error!(
        task_id = task_id_str(&task),
        error = task.error.as_deref().unwrap_or("unknown error"),
        "Task failed"
    );
    let task_id = task.id.as_deref().ok_or(HandlerError::MissingTaskId)?;
    let job_id = task.job_id.as_deref().ok_or(HandlerError::MissingJobId)?;
    let now = time::OffsetDateTime::now_utc();
    persist_task_error(ds.clone(), task_id, &task, now).await?;

    let job = ds.get_job_by_id(job_id).await?;

    if !is_job_active(job.state) {
        if let Err(e) = fire_task_webhooks(ds, &task, "task.Error").await {
            warn!(error = %e, "failed to fire task.Error webhook");
        }
        return Ok(());
    }

    if task.retry.as_ref().is_some_and(can_retry) {
        return create_retry_task(ds, broker, task, &job, now).await;
    }

    let mut failed_task = task.clone();
    failed_task.state = TaskState::Failed;
    failed_task.failed_at = Some(now);
    broker
        .publish_task(QUEUE_FAILED.to_string(), &failed_task)
        .await?;

    handle_task_failed(ds, broker, failed_task).await
}

async fn persist_task_progress(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let task_id = task.id.as_deref().ok_or(HandlerError::MissingTaskId)?;

    ds.update_task(
        task_id,
        Box::new(move |mut current| {
            current.state = task.state;
            current.started_at = task.started_at;
            current.completed_at = task.completed_at;
            current.failed_at = task.failed_at;
            current.result.clone_from(&task.result);
            current.error.clone_from(&task.error);
            Ok(current)
        }),
    )
    .await
    .map_err(anyhow::Error::from)
}

async fn persist_completed_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    task: &twerk_core::task::Task,
) -> Result<()> {
    let task_id = task.id.as_deref().ok_or(HandlerError::MissingTaskId)?;
    let completed_at = task.completed_at;
    let result = task.result.clone();

    ds.update_task(
        task_id,
        Box::new(move |mut current| {
            current.state = TaskState::Completed;
            current.completed_at = completed_at;
            current.result = result;
            Ok(current)
        }),
    )
    .await
    .map_err(anyhow::Error::from)
}

async fn route_completed_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    if let Some(parent_id) = task.parent_id.clone() {
        handle_subtask_completed(ds, broker, task, parent_id.as_str()).await
    } else {
        let job_id = task.job_id.as_deref().ok_or(HandlerError::MissingJobId)?;
        handle_top_level_task_completed(ds, broker, job_id.to_string()).await
    }
}

async fn persist_failed_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    task: &twerk_core::task::Task,
) -> Result<()> {
    let task_id = task.id.as_deref().ok_or(HandlerError::MissingTaskId)?;
    let failed_at = task.failed_at;
    let task_error = task.error.clone();

    ds.update_task(
        task_id,
        Box::new(move |mut current| {
            current.state = TaskState::Failed;
            current.failed_at = failed_at;
            current.error = task_error;
            Ok(current)
        }),
    )
    .await
    .map_err(anyhow::Error::from)
}

async fn route_failed_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    if let Some(parent_id) = task.parent_id.clone() {
        handle_subtask_failed(ds, broker, task, parent_id.to_string()).await
    } else {
        let job_id = task.job_id.as_deref().ok_or(HandlerError::MissingJobId)?;
        handle_top_level_task_failed(ds, broker, job_id.to_string(), task.error.clone()).await
    }
}

async fn persist_task_error(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    task_id: &str,
    task: &twerk_core::task::Task,
    now: time::OffsetDateTime,
) -> Result<()> {
    let task_error = task.error.clone();
    let task_result = task.result.clone();

    ds.update_task(
        task_id,
        Box::new(move |mut current| {
            current.state = TaskState::Failed;
            current.failed_at = Some(now);
            current.error.clone_from(&task_error);
            current.result.clone_from(&task_result);
            Ok(current)
        }),
    )
    .await
    .map_err(anyhow::Error::from)
}
