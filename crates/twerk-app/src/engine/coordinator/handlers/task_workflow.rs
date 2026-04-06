//! Top-level task workflow handlers (job progression logic)

use anyhow::{anyhow, Result};
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

    let tasks = job
        .tasks
        .as_ref()
        .ok_or_else(|| anyhow!("job has no tasks"))?;

    if next_position <= tasks.len() as i64 {
        let base_task = tasks
            .get((next_position - 1) as usize)
            .ok_or_else(|| anyhow!("task out of bounds"))?;
        let mut task = base_task.clone();
        task.id = Some(new_short_uuid().into());
        task.job_id = Some(job_id.clone().into());
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
        let mut completed_job = job;
        completed_job.state = JobState::Completed;
        broker.publish_job(&completed_job).await
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
    let mut job = ds.get_job_by_id(&job_id).await?;
    job.state = JobState::Failed;
    job.failed_at = Some(time::OffsetDateTime::now_utc());
    job.error = task_error;

    broker.publish_job(&job).await
}
