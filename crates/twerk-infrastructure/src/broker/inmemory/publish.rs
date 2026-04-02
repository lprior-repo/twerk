//! Publishing logic for the in-memory broker.

use super::super::BoxedFuture;
use super::InMemoryBroker;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};
use twerk_common::constants::DEFAULT_TASK_NAME;
use twerk_common::wildcard::wildcard_match;
use twerk_core::job::Job;
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

// ── Shared handler spawn helper ───────────────────────────────────────────────────

/// Spawns a handler call, logging any errors with the given message.
///
/// This eliminates the repeated `tokio::spawn(async move { if let Err(e) = handler(...).await {...} })`
/// pattern across all publish functions.
fn spawn_handler<T: Send + 'static>(
    handler: Arc<dyn Fn(T) -> BoxedFuture<()> + Send + Sync>,
    msg: T,
    error_msg: &'static str,
) {
    tokio::spawn(async move {
        if handler(msg).await.is_err() {
            warn!(error_msg);
        }
    });
}

/// Publish a task to a queue.
pub(crate) fn task(broker: &InMemoryBroker, qname: &str, task: &Task) -> BoxedFuture<()> {
    let task_arc = Arc::new(task.clone());

    // Store the task
    broker
        .tasks
        .entry(qname.to_string())
        .or_default()
        .push(Arc::clone(&task_arc));

    // Collect handlers for this queue before spawning tasks
    let handlers: Vec<super::TaskHandler> = broker
        .handlers
        .get(qname)
        .map(|entry| entry.value().clone())
        .unwrap_or_default();

    // Invoke all registered handlers for this queue
    for handler in handlers {
        let task_clone = Arc::clone(&task_arc);
        spawn_handler(handler, task_clone, "task handler failed");
    }

    Box::pin(async { Ok(()) })
}

/// Publish multiple tasks to a queue.
pub(crate) fn tasks(
    broker: &InMemoryBroker,
    qname: &str,
    tasks: &[Task],
) -> super::super::BoxedFuture<()> {
    let task_arcs: Vec<Arc<Task>> = tasks.iter().map(|t| Arc::new(t.clone())).collect();

    // Store the tasks in one go
    broker
        .tasks
        .entry(qname.to_string())
        .or_default()
        .extend(task_arcs.clone());

    // Collect handlers for this queue before spawning tasks
    let handlers: Vec<super::TaskHandler> = broker
        .handlers
        .get(qname)
        .map(|entry| entry.value().clone())
        .unwrap_or_default();

    // Invoke all registered handlers for each task
    if !handlers.is_empty() && !task_arcs.is_empty() {
        let mut jobs = Vec::with_capacity(task_arcs.len() * handlers.len());
        for task_arc in &task_arcs {
            for handler in &handlers {
                jobs.push((handler.clone(), Arc::clone(task_arc)));
            }
        }
        tokio::spawn(async move {
            use futures_util::StreamExt;
            futures_util::stream::iter(jobs)
                .for_each_concurrent(100, |(handler, task)| async move {
                    if handler(task).await.is_err() {
                        warn!("batch task handler failed");
                    }
                })
                .await;
        });
    }

    Box::pin(async { Ok(()) })
}

/// Publish task progress.
pub(crate) fn task_progress(broker: &InMemoryBroker, task: &Task) -> BoxedFuture<()> {
    let task = task.clone();
    let handlers = broker.progress_handlers.clone();
    Box::pin(async move {
        let handlers = handlers.read().await;
        for handler in handlers.iter() {
            let task_clone = task.clone();
            spawn_handler(handler.clone(), task_clone, "progress handler failed");
        }
        Ok(())
    })
}

/// Publish a heartbeat.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn heartbeat(broker: &InMemoryBroker, node: Node) -> BoxedFuture<()> {
    let node = node.clone();
    let handlers = broker.heartbeat_handlers.clone();
    let heartbeats = broker.heartbeats.clone();
    Box::pin(async move {
        if let Some(ref node_id) = node.id {
            // Use RwLock to safely write to heartbeats
            heartbeats
                .write()
                .await
                .insert(node_id.to_string(), node.clone());
        }
        let handlers = handlers.read().await;
        for handler in handlers.iter() {
            let node_clone = node.clone();
            spawn_handler(handler.clone(), node_clone, "heartbeat handler failed");
        }
        Ok(())
    })
}

/// Publish a job.
pub(crate) fn job(broker: &InMemoryBroker, job: &Job) -> BoxedFuture<()> {
    let job = job.clone();
    let handlers = broker.job_handlers.clone();
    Box::pin(async move {
        let handlers = handlers.read().await;
        debug!(
            "Publishing job {} to {} handlers",
            job.id.as_deref().unwrap_or(DEFAULT_TASK_NAME),
            handlers.len()
        );
        for handler in handlers.iter() {
            let job_clone = job.clone();
            spawn_handler(handler.clone(), job_clone, "job handler failed");
        }
        Ok(())
    })
}

/// Publish an event to a topic.
pub(crate) fn event(broker: &InMemoryBroker, topic: String, event: Value) -> BoxedFuture<()> {
    let handlers = broker.event_handlers.clone();
    Box::pin(async move {
        for entry in handlers.iter() {
            let pattern = entry.key();
            if wildcard_match(pattern, &topic) {
                let event_clone = event.clone();
                let topic_handlers = entry.value().clone();
                for handler in topic_handlers {
                    let ev_clone = event_clone.clone();
                    spawn_handler(handler.clone(), ev_clone, "event handler failed");
                }
            }
        }
        Ok(())
    })
}

/// Publish a task log part.
pub(crate) fn task_log_part(broker: &InMemoryBroker, part: &TaskLogPart) -> BoxedFuture<()> {
    let part = part.clone();
    let handlers = broker.task_log_part_handlers.clone();
    let task_log_parts = broker.task_log_parts.clone();
    Box::pin(async move {
        if let Some(task_id) = &part.task_id {
            let task_id_str = task_id.to_string();
            // Use RwLock to safely write to task_log_parts
            let task_log_parts_guard = task_log_parts.write().await;
            let mut entry = task_log_parts_guard.entry(task_id_str).or_default();
            // Entry::Ref supports mutable access via DerefMut - push takes &mut self
            entry.push(part.clone());
        }
        let handlers = handlers.read().await;
        for handler in handlers.iter() {
            let part_clone = part.clone();
            spawn_handler(handler.clone(), part_clone, "task log part handler failed");
        }
        Ok(())
    })
}
