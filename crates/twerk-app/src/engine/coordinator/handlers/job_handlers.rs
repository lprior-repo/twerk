//! Job-related event handlers

use crate::engine::coordinator::handlers::cancellation::{cancel_active_tasks, cancel_parent_job};
use crate::engine::coordinator::handlers::util::{build_job_context, is_job_active, job_id_str};
use crate::engine::coordinator::webhook::fire_job_webhooks;
use crate::engine::types::JobHandlerError;
use crate::engine::{TOPIC_JOB_COMPLETED, TOPIC_JOB_FAILED};
use anyhow::{anyhow, Result};
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
            .map_err(|e| anyhow!("{e}")),
        JobState::Failed => fail_job(ds, broker, job).await.map_err(|e| anyhow!("{e}")),
        JobState::Running => mark_job_as_running(ds, broker, job)
            .await
            .map_err(|e| anyhow!("{e}")),
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
    let tasks = job
        .tasks
        .as_ref()
        .ok_or_else(|| anyhow!("job has no tasks"))?;
    let base_task = tasks.first().ok_or_else(|| anyhow!("job has no tasks"))?;

    let now = time::OffsetDateTime::now_utc();
    let job_ctx = build_job_context(&job);
    let job_id = job.id.as_ref().ok_or_else(|| anyhow!("job has no id"))?;

    let mut task =
        twerk_core::eval::evaluate_task(base_task, &job_ctx).map_err(|e| anyhow!("{e}"))?;
    task.id = Some(new_short_uuid().into());
    task.job_id = Some(job_id.clone());
    task.state = TaskState::Pending;
    task.position = 1;
    task.created_at = Some(now);

    ds.create_task(&task).await?;

    ds.update_job(
        job_id,
        Box::new(move |mut u| {
            u.state = JobState::Scheduled;
            u.started_at = Some(now);
            u.position = 1;
            Ok(u)
        }),
    )
    .await?;

    fire_job_webhooks(&job, "job.Scheduled").await;
    broker.publish_task(QUEUE_PENDING.to_string(), &task).await
}

async fn restart_job(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job: twerk_core::job::Job,
) -> Result<()> {
    let job_id = job.id.as_ref().ok_or_else(|| anyhow!("job has no id"))?;
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

    let tasks = job
        .tasks
        .as_ref()
        .ok_or_else(|| anyhow!("job has no tasks"))?;
    let task_index = (job.position - 1) as usize;
    let base_task = tasks
        .get(task_index)
        .ok_or_else(|| anyhow!("job position out of bounds"))?;

    let mut task = twerk_core::eval::evaluate_task(base_task, &build_job_context(&job))
        .map_err(|e| anyhow!("{e}"))?;
    task.id = Some(new_short_uuid().into());
    task.job_id = Some(job_id.clone());
    task.state = TaskState::Pending;
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
        Some(parent_id) => {
            let mut parent = ds.get_task_by_id(parent_id).await?;
            parent.state = TaskState::Completed;
            parent.completed_at = Some(now);
            broker
                .publish_task(QUEUE_COMPLETED.to_string(), &parent)
                .await
        }
        None => {
            broker
                .publish_event(TOPIC_JOB_COMPLETED.to_string(), serde_json::to_value(&job)?)
                .await
        }
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

    if let Some(ref parent_id) = job.parent_id {
        let mut parent = ds
            .get_task_by_id(parent_id)
            .await
            .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
        parent.state = TaskState::Failed;
        parent.failed_at = failed_at;
        parent.error.clone_from(&job.error);
        broker
            .publish_task(QUEUE_FAILED.to_string(), &parent)
            .await
            .map_err(|e| JobHandlerError::Handler(e.to_string()))?;
    }

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
