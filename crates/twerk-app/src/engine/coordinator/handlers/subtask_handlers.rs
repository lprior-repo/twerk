//! Subtask-related event handlers (parallel/each patterns)

use super::task_handlers::handle_task_failed;
use super::HandlerError;
use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use twerk_core::task::TaskState;
use twerk_infrastructure::broker::queue::QUEUE_COMPLETED;

// ── Subtask Completion Handlers ──────────────────────────────────

/// Handles subtask completion by delegating to appropriate handler.
pub(crate) async fn handle_subtask_completed(
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
    parent: twerk_core::task::Task,
) -> Result<()> {
    let parent_id = parent
        .id
        .as_deref()
        .ok_or(HandlerError::MissingParentTaskId)?;
    let is_last = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let is_last_clone = is_last.clone();

    ds.update_task(
        parent_id,
        Box::new(move |mut u| {
            if let Some(ref mut p) = u.parallel {
                p.completions += 1;
                if let Some(ref tasks) = p.tasks {
                    is_last_clone.store(
                        p.completions >= tasks.len() as i64,
                        std::sync::atomic::Ordering::SeqCst,
                    );
                }
            }
            Ok(u)
        }),
    )
    .await?;

    if is_last.load(std::sync::atomic::Ordering::SeqCst) {
        let mut completed_parent = parent;
        completed_parent.state = TaskState::Completed;
        completed_parent.completed_at = Some(time::OffsetDateTime::now_utc());
        broker
            .publish_task(QUEUE_COMPLETED.to_string(), &completed_parent)
            .await
    } else {
        Ok(())
    }
}

async fn handle_each_subtask_completed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    _task: twerk_core::task::Task,
    parent: twerk_core::task::Task,
) -> Result<()> {
    let parent_id = parent
        .id
        .as_deref()
        .ok_or(HandlerError::MissingParentTaskId)?;
    let is_last = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let is_last_clone = is_last.clone();

    ds.update_task(
        parent_id,
        Box::new(move |mut u| {
            if let Some(ref mut e) = u.each {
                e.completions += 1;
                is_last_clone.store(e.completions >= e.size, std::sync::atomic::Ordering::SeqCst);
            }
            Ok(u)
        }),
    )
    .await?;

    if is_last.load(std::sync::atomic::Ordering::SeqCst) {
        let mut completed_parent = parent;
        completed_parent.state = TaskState::Completed;
        completed_parent.completed_at = Some(time::OffsetDateTime::now_utc());
        broker
            .publish_task(QUEUE_COMPLETED.to_string(), &completed_parent)
            .await
    } else {
        Ok(())
    }
}

// ── Subtask Failure Handlers ─────────────────────────────────────

/// Handles subtask failure by failing the parent task.
pub(crate) fn handle_subtask_failed(
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    task: twerk_core::task::Task,
    parent_id: String,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
    Box::pin(async move {
        let parent = ds.get_task_by_id(&parent_id).await?;

        if parent.parallel.is_some() || parent.each.is_some() {
            let mut failed_parent = parent;
            failed_parent.state = TaskState::Failed;
            failed_parent.failed_at = Some(time::OffsetDateTime::now_utc());
            failed_parent.error = task.error.clone();

            // When a subtask fails, we fail the parent immediately
            handle_task_failed(ds, broker, failed_parent).await
        } else {
            Ok(())
        }
    })
}
