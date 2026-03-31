//! Job cancellation helpers

use crate::engine::types::JobHandlerError;
use std::sync::Arc;
use twerk_core::job::JOB_STATE_CANCELLED;
use twerk_core::task::TASK_STATE_CANCELLED;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

/// Cancels the parent job when a subjob is cancelled.
pub(crate) async fn cancel_parent_job(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    parent_id: &str,
) -> Result<(), JobHandlerError> {
    let parent_task = ds
        .get_task_by_id(parent_id)
        .await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    let parent_job_id = parent_task
        .job_id
        .as_deref()
        .ok_or_else(|| JobHandlerError::Handler("parent task has no job_id".to_string()))?;
    let parent_job = ds
        .get_job_by_id(parent_job_id)
        .await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
    let mut cancelled_job = parent_job;
    cancelled_job.state = JOB_STATE_CANCELLED.to_string();
    broker
        .publish_job(&cancelled_job)
        .await
        .map_err(|e| JobHandlerError::Handler(e.to_string()))
}

/// Cancels task affinity (subjob or node queue).
pub(crate) async fn cancel_task_affinity(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    task: &twerk_core::task::Task,
) -> Result<(), JobHandlerError> {
    match &task.subjob {
        Some(subjob) => {
            let subjob_id = subjob
                .id
                .as_deref()
                .ok_or_else(|| JobHandlerError::Handler("subjob has no id".to_string()))?;
            let job_to_cancel = ds
                .get_job_by_id(subjob_id)
                .await
                .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
            let mut cancelled_job = job_to_cancel;
            cancelled_job.state = JOB_STATE_CANCELLED.to_string();
            broker
                .publish_job(&cancelled_job)
                .await
                .map_err(|e| JobHandlerError::Handler(e.to_string()))
        }
        None => {
            if let Some(ref node_id) = task.node_id {
                let node = ds
                    .get_node_by_id(node_id)
                    .await
                    .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;
                let queue = node.queue.unwrap_or_else(|| QUEUE_PENDING.to_string());
                broker
                    .publish_task(queue, task)
                    .await
                    .map_err(|e| JobHandlerError::Handler(e.to_string()))
            } else {
                Ok(())
            }
        }
    }
}

/// Cancels all active tasks for a job.
pub(crate) async fn cancel_active_tasks(
    ds: &Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: &Arc<dyn twerk_infrastructure::broker::Broker>,
    job_id: &str,
) -> Result<(), JobHandlerError> {
    let tasks = ds
        .get_active_tasks(job_id)
        .await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;

    for task in tasks {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| JobHandlerError::Handler("task has no id".to_string()))?;
        ds.update_task(
            task_id,
            Box::new(|mut u| {
                u.state = TASK_STATE_CANCELLED.to_string();
                Ok(u)
            }),
        )
        .await
        .map_err(|e| JobHandlerError::Datastore(e.to_string()))?;

        cancel_task_affinity(ds, broker, &task).await?;
    }

    Ok(())
}
