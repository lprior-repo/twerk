//! Task retry logic

use crate::engine::coordinator::handlers::util::build_job_context;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use twerk_core::task::TaskState;
use twerk_core::uuid::new_short_uuid;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

/// Creates a retry task when retry is available.
pub(crate) async fn create_retry_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
    job: &twerk_core::job::Job,
    now: time::OffsetDateTime,
) -> Result<()> {
    let retry_config = task
        .retry
        .clone()
        .ok_or_else(|| anyhow!("task has no retry config"))?;

    let mut retry_task = task;
    retry_task.id = Some(new_short_uuid().into());
    retry_task.created_at = Some(now);
    retry_task.state = TaskState::Pending;
    retry_task.error = None;
    retry_task.failed_at = None;
    retry_task.retry = Some(twerk_core::task::TaskRetry {
        attempts: retry_config.attempts + 1,
        limit: retry_config.limit,
    });

    let job_ctx = build_job_context(job);
    let final_task =
        twerk_core::eval::evaluate_task(&retry_task, &job_ctx).map_err(|e| anyhow!("{e}"))?;

    ds.create_task(&final_task).await?;
    broker
        .publish_task(QUEUE_PENDING.to_string(), &final_task)
        .await
}
