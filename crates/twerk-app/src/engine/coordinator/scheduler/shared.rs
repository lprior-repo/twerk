use super::Scheduler;
use super::SchedulerError;
use anyhow::Result;
use std::collections::HashMap;
use twerk_core::task::{Task, TaskState};
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

pub(super) struct SchedulerIds<'a> {
    pub(super) task_id: &'a str,
    pub(super) job_id: &'a str,
}

pub(super) fn scheduler_ids<'a>(task: &'a Task, scheduler: &str) -> Result<SchedulerIds<'a>> {
    let task_id = task
        .id
        .as_deref()
        .ok_or_else(|| SchedulerError::TaskIdRequired {
            scheduler: scheduler.to_string(),
        })?;
    let job_id = task
        .job_id
        .as_deref()
        .ok_or_else(|| SchedulerError::JobIdRequired {
            scheduler: scheduler.to_string(),
        })?;

    Ok(SchedulerIds { task_id, job_id })
}

pub(super) fn job_context_map(job: &twerk_core::job::Job) -> HashMap<String, serde_json::Value> {
    job.context
        .as_ref()
        .map(twerk_core::job::JobContext::as_map)
        .unwrap_or_default()
}

pub(super) async fn mark_task_running(
    scheduler: &Scheduler,
    task_id: &str,
    now: time::OffsetDateTime,
) -> Result<()> {
    scheduler
        .ds
        .update_task(
            task_id,
            Box::new(move |task| {
                Ok(Task {
                    state: TaskState::Running,
                    started_at: Some(now),
                    ..task
                })
            }),
        )
        .await
        .map_err(anyhow::Error::from)
}

pub(super) async fn create_and_publish_subtasks(
    scheduler: &Scheduler,
    subtasks: &[Task],
) -> Result<()> {
    if subtasks.is_empty() {
        return Ok(());
    }

    scheduler.ds.create_tasks(subtasks).await?;

    if let Err(error) = scheduler
        .broker
        .publish_tasks(QUEUE_PENDING.to_string(), subtasks)
        .await
    {
        rollback_failed_publish(scheduler, subtasks, &error).await;
        return Err(error);
    }

    Ok(())
}

async fn rollback_failed_publish(scheduler: &Scheduler, subtasks: &[Task], error: &anyhow::Error) {
    let error_msg = format!("broker publish failed: {error}");
    let compensating: Vec<_> = subtasks
        .iter()
        .filter_map(|task| task.id.as_deref())
        .map(|task_id| {
            let message = error_msg.clone();
            scheduler.ds.update_task(
                task_id,
                Box::new(move |task| {
                    Ok(Task {
                        state: TaskState::Failed,
                        error: Some(message),
                        ..task
                    })
                }),
            )
        })
        .collect();

    let _ = futures_util::future::join_all(compensating).await;
}
