//! Job-related event handlers

use super::HandlerError;
use crate::engine::coordinator::handlers::cancellation::{cancel_active_tasks, cancel_parent_job};
use crate::engine::coordinator::handlers::task_handlers::handle_pending_task;
use crate::engine::coordinator::handlers::util::{build_job_context, is_job_active, job_id_str};
use crate::engine::coordinator::webhook::fire_job_webhooks;
use crate::engine::types::JobHandlerError;
use crate::engine::{TOPIC_JOB_COMPLETED, TOPIC_JOB_FAILED};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, error, instrument};
use twerk_core::job::JobState;
use twerk_core::task::TaskState;
use twerk_core::uuid::new_short_uuid;
use twerk_infrastructure::broker::queue::{QUEUE_COMPLETED, QUEUE_FAILED, QUEUE_PENDING};

// ── Public Job Handlers ─────────────────────────────────────────

/// Handles job events from the broker.
///
/// # Errors
/// Returns error if job handling logic fails.
#[instrument(name = "handle_job_event", skip_all, fields(job_id = %job.id.as_deref().unwrap_or("unknown"), state = %job.state))]
pub async fn handle_job_event(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    debug!(job_id = job_id_str(&job), state = %job.state, "Handling job event");
    let res = match job.state {
        JobState::Pending => start_job(ds, broker, job).await,
        JobState::Completed => complete_job(ds, broker, job).await,
        JobState::Restart => restart_job(ds, broker, job).await,
        JobState::Cancelled => handle_cancel(ds, broker, job)
            .await
            .map_err(|e| anyhow::anyhow!("{e}")),
        JobState::Failed => fail_job(ds, broker, job)
            .await
            .map_err(|e| anyhow::anyhow!("{e}")),
        JobState::Running => mark_job_as_running(ds, broker, job)
            .await
            .map_err(|e| anyhow::anyhow!("{e}")),
        JobState::Scheduled => Ok(()),
    };

    if let Err(ref e) = res {
        error!(error = %e, "failed to handle job event");
    }
    res
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
    let job_id = job
        .id
        .as_deref()
        .ok_or_else(|| JobHandlerError::Handler("job has no id".to_string()))?;

    if is_job_active(job.state) {
        ds.update_job(
            job_id,
            Box::new(|mut u| {
                u.state = JobState::Cancelled;
                Ok(u)
            }),
        )
        .await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    }

    if let Some(ref parent_id) = job.parent_id {
        cancel_parent_job(&ds, &broker, parent_id).await?;
    }

    cancel_active_tasks(&ds, &broker, job_id).await
}

// ── Private Job Handlers ────────────────────────────────────────

#[instrument(name = "start_job", skip_all)]
async fn start_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    let tasks = job.tasks.as_ref().ok_or(HandlerError::JobHasNoTasks)?;
    let base_task = tasks.first().ok_or(HandlerError::JobHasNoTasks)?;

    let now = time::OffsetDateTime::now_utc();
    let job_ctx = build_job_context(&job);
    let job_id = job.id.as_ref().ok_or(HandlerError::MissingJobId)?;

    debug!(job_id = %job_id, "start_job: transitioning job to Scheduled");
    transition_job_to_scheduled(&ds, job_id, now).await?;

    debug!(job_id = %job_id, "start_job: evaluating task");
    let task = build_pending_job_task(base_task, &job_ctx, job_id, 1, now)?;

    debug!(job_id = %job_id, "start_job: creating task in datastore");
    ds.create_task(&task).await?;

    debug!(job_id = %job_id, "start_job: firing webhooks");
    fire_job_webhooks(&job, "job.Scheduled").await;

    debug!(job_id = %job_id, "start_job: handling pending task");
    handle_pending_task(ds, broker, task).await
}

async fn restart_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    let job_id = job.id.as_ref().ok_or(HandlerError::MissingJobId)?;
    let now = time::OffsetDateTime::now_utc();

    ds.update_job(
        job_id,
        Box::new(move |mut u| {
            u.state = JobState::Running;
            u.failed_at = None;
            Ok(u)
        }),
    )
    .await?;

    let tasks = job.tasks.as_ref().ok_or(HandlerError::JobHasNoTasks)?;
    let task_index = (job.position - 1) as usize;
    let base_task = tasks.get(task_index).ok_or(HandlerError::TaskOutOfBounds)?;

    let task = build_pending_job_task(
        base_task,
        &build_job_context(&job),
        job_id,
        job.position,
        now,
    )?;

    ds.create_task(&task).await?;
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await
}

async fn complete_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    let now = time::OffsetDateTime::now_utc();
    let job_id = job.id.as_ref().ok_or(HandlerError::MissingJobId)?;

    ds.update_job(
        job_id,
        Box::new(move |mut u| {
            u.state = JobState::Completed;
            u.completed_at = Some(now);
            Ok(u)
        }),
    )
    .await?;

    let updated_job = ds.get_job_by_id(job_id).await?;
    fire_job_webhooks(&updated_job, "job.Completed").await;

    match &job.parent_id {
        Some(parent_id) => publish_completed_parent_task(&ds, &broker, parent_id, now).await,
        None => publish_job_completed_event(&broker, &job).await,
    }
}

async fn mark_job_as_running(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    _broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<(), JobHandlerError> {
    let job_id = job
        .id
        .as_deref()
        .ok_or_else(|| JobHandlerError::Handler("job has no id".to_string()))?;

    ds.update_job(
        job_id,
        Box::new(move |mut u| {
            if u.state == JobState::Scheduled {
                u.state = JobState::Running;
                u.failed_at = None;
            }
            Ok(u)
        }),
    )
    .await
    .map_err(|e| JobHandlerError::Datastore(e.to_string()))
}

async fn fail_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<(), JobHandlerError> {
    let job_id = job
        .id
        .as_deref()
        .ok_or_else(|| JobHandlerError::Handler("job has no id".to_string()))?;
    let failed_at = job.failed_at;

    ds.update_job(
        job_id,
        Box::new(move |mut u| {
            if is_job_active(u.state) {
                u.state = JobState::Failed;
                u.failed_at = failed_at;
            }
            Ok(u)
        }),
    )
    .await
    .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;

    publish_failed_parent_task_if_present(&ds, &broker, &job, failed_at).await?;

    cancel_active_tasks(&ds, &broker, job_id).await?;

    let updated_job = ds
        .get_job_by_id(job_id)
        .await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    if updated_job.state == JobState::Failed {
        broker
            .publish_event(
                TOPIC_JOB_FAILED.to_string(),
                serde_json::to_value(&updated_job)
                    .map_err(|e| JobHandlerError::Handler(e.to_string()))?,
            )
            .await
            .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
    }

    Ok(())
}

async fn transition_job_to_scheduled(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    job_id: &twerk_core::id::JobId,
    now: time::OffsetDateTime,
) -> Result<()> {
    ds.update_job(
        job_id,
        Box::new(move |mut job| {
            job.state = JobState::Scheduled;
            job.started_at = Some(now);
            job.position = 1;
            Ok(job)
        }),
    )
    .await
    .map_err(anyhow::Error::from)
}

fn build_pending_job_task(
    base_task: &twerk_core::task::Task,
    job_ctx: &std::collections::HashMap<String, serde_json::Value>,
    job_id: &twerk_core::id::JobId,
    position: i64,
    now: time::OffsetDateTime,
) -> Result<twerk_core::task::Task> {
    let evaluated = twerk_core::eval::evaluate_task(base_task, job_ctx)
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    Ok(twerk_core::task::Task {
        id: Some(new_short_uuid().into()),
        job_id: Some(job_id.clone()),
        state: TaskState::Pending,
        position,
        created_at: Some(now),
        ..evaluated
    })
}

async fn publish_completed_parent_task(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    parent_id: &twerk_core::id::JobId,
    now: time::OffsetDateTime,
) -> Result<()> {
    let mut parent = ds.get_task_by_id(parent_id).await?;
    parent.state = TaskState::Completed;
    parent.completed_at = Some(now);
    broker
        .publish_task(QUEUE_COMPLETED.to_string(), &parent)
        .await
}

async fn publish_job_completed_event(
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    job: &twerk_core::job::Job,
) -> Result<()> {
    broker
        .publish_event(TOPIC_JOB_COMPLETED.to_string(), serde_json::to_value(job)?)
        .await
}

async fn publish_failed_parent_task_if_present(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    job: &twerk_core::job::Job,
    failed_at: Option<time::OffsetDateTime>,
) -> Result<(), JobHandlerError> {
    match &job.parent_id {
        Some(parent_id) => {
            let mut parent = ds
                .get_task_by_id(parent_id)
                .await
                .map_err(|error| JobHandlerError::Datastore(error.to_string()))?;
            parent.state = TaskState::Failed;
            parent.failed_at = failed_at;
            parent.error.clone_from(&job.error);
            broker
                .publish_task(QUEUE_FAILED.to_string(), &parent)
                .await
                .map_err(|error| JobHandlerError::Handler(error.to_string()))
        }
        None => Ok(()),
    }
}
