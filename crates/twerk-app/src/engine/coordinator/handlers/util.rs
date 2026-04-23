//! Shared utility functions for handlers (pure calculations)

use super::HandlerError;
use anyhow::Result;
use std::sync::Arc;
use twerk_core::job::JobState;
use twerk_core::task::TaskState;
use twerk_infrastructure::broker::queue::QUEUE_COMPLETED;

// ── Calculations (Pure) ────────────────────────────────────────

/// Extracts job ID string safely.
pub fn job_id_str(job: &twerk_core::job::Job) -> &str {
    job.id.as_deref().map_or("unknown", |id| id)
}

/// Extracts task ID string safely.
pub fn task_id_str(task: &twerk_core::task::Task) -> &str {
    task.id.as_deref().map_or("unknown", |id| id)
}

/// Builds job context from job, merging inputs.
pub fn build_job_context(
    job: &twerk_core::job::Job,
) -> std::collections::HashMap<String, serde_json::Value> {
    job.context
        .as_ref()
        .map_or_else(std::collections::HashMap::new, |ctx| {
            let mut merged: std::collections::HashMap<_, _> = ctx
                .as_map()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            ctx.inputs
                .as_ref()
                .into_iter()
                .flatten()
                .for_each(|(k, v)| {
                    merged.insert(k.clone(), serde_json::Value::String(v.clone()));
                });
            merged
        })
}

/// Checks if job is in an active state.
pub fn is_job_active(job_state: JobState) -> bool {
    matches!(
        job_state,
        JobState::Running | JobState::Scheduled | JobState::Pending
    )
}

/// Checks if retry is available.
pub fn can_retry(retry: &twerk_core::task::TaskRetry) -> bool {
    retry.attempts < retry.limit
}

/// Checks if task should be skipped.
pub fn should_skip_task(task: &twerk_core::task::Task) -> bool {
    task.r#if.as_ref().is_some_and(|s| s.trim() == "false")
}

// ── Actions ────────────────────────────────────────────────────

/// Skips a task by marking it as SKIPPED.
pub async fn skip_task(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
) -> Result<()> {
    let now = time::OffsetDateTime::now_utc();
    let task_id = task.id.as_deref().ok_or(HandlerError::MissingTaskId)?;

    ds.update_task(
        task_id,
        Box::new(move |mut u| {
            u.state = TaskState::Skipped;
            u.scheduled_at = Some(now);
            u.started_at = Some(now);
            u.completed_at = Some(now);
            Ok(u)
        }),
    )
    .await?;

    let mut skipped_task = task;
    skipped_task.state = TaskState::Skipped;
    skipped_task.scheduled_at = Some(now);
    skipped_task.started_at = Some(now);
    skipped_task.completed_at = Some(now);

    broker
        .publish_task(QUEUE_COMPLETED.to_string(), &skipped_task)
        .await
}
