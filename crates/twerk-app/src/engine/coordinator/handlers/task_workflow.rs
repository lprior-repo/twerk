//! Top-level task workflow handlers (job progression logic)

use super::HandlerError;
use crate::engine::coordinator::webhook::fire_job_webhooks;
use crate::engine::{TOPIC_JOB_COMPLETED, TOPIC_JOB_FAILED};
use anyhow::Result;
use std::sync::Arc;
use tracing::instrument;
use twerk_core::job::JobState;
use twerk_core::task::TaskState;
use twerk_core::uuid::new_short_uuid;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

/// Handles completion of a top-level task and progresses the job.
///
/// If there are more tasks in the job, schedules the next one.
/// If all tasks are complete, marks the job as completed.
///
/// # Errors
/// Returns error if task creation or job update fails.
#[instrument(name = "handle_top_level_task_completed", skip_all, fields(job_id = %job_id))]
pub async fn handle_top_level_task_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: String,
) -> Result<()> {
    let job = ds.get_job_by_id(&job_id).await?;
    let next_position = job.position + 1;
    let now = time::OffsetDateTime::now_utc();

    let tasks = job.tasks.as_ref().ok_or(HandlerError::JobHasNoTasks)?;

    if next_position <= tasks.len() as i64 {
        let base_task = tasks
            .get((next_position - 1) as usize)
            .ok_or(HandlerError::TaskOutOfBounds)?;
        let mut task = base_task.clone();
        task.id = Some(new_short_uuid().into());
        task.job_id = Some(twerk_core::id::JobId::new(job_id.clone())?);
        task.state = TaskState::Pending;
        task.position = next_position;
        task.created_at = Some(now);

        ds.create_task(&task).await?;

        ds.update_job(
            &job_id,
            Box::new(move |mut u| {
                u.position = next_position;
                Ok(u)
            }),
        )
        .await?;

        broker.publish_task(QUEUE_PENDING.to_string(), &task).await
    } else {
        let job_id_str = job
            .id
            .as_ref()
            .map(|id| id.as_str().to_string())
            .unwrap_or_default();
        ds.update_job(
            &job_id_str,
            Box::new(move |mut u| {
                u.state = JobState::Completed;
                u.completed_at = Some(now);
                Ok(u)
            }),
        )
        .await?;

        let updated_job = ds.get_job_by_id(&job_id_str).await?;

        fire_job_webhooks(&updated_job, "job.Completed").await;

        broker
            .publish_event(
                TOPIC_JOB_COMPLETED.to_string(),
                serde_json::to_value(&updated_job)
                    .map_err(|e| anyhow::anyhow!("failed to serialize completed job: {e}"))?,
            )
            .await
    }
}

/// Handles failure of a top-level task by failing the job.
///
/// # Errors
/// Returns error if job publishing fails.
#[instrument(name = "handle_top_level_task_failed", skip_all, fields(job_id = %job_id))]
pub async fn handle_top_level_task_failed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: String,
    task_error: Option<String>,
) -> Result<()> {
    let now = time::OffsetDateTime::now_utc();

    ds.update_job(
        &job_id,
        Box::new(move |mut u| {
            u.state = JobState::Failed;
            u.failed_at = Some(now);
            u.error.clone_from(&task_error);
            Ok(u)
        }),
    )
    .await?;

    let updated_job = ds.get_job_by_id(&job_id).await?;

    fire_job_webhooks(&updated_job, "job.Failed").await;

    broker
        .publish_event(
            TOPIC_JOB_FAILED.to_string(),
            serde_json::to_value(&updated_job)
                .map_err(|e| anyhow::anyhow!("failed to serialize failed job: {e}"))?,
        )
        .await
}
