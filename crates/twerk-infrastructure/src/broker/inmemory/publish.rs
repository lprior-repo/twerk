//! Publishing logic for the in-memory broker.

use super::super::BoxedFuture;
use super::InMemoryBroker;
use futures_util::{FutureExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};
use twerk_common::constants::DEFAULT_TASK_NAME;
use twerk_common::wildcard::wildcard_match;
use twerk_core::job::Job;
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

// ── Shared handler invocation helpers ───────────────────────────────────────────────────

/// Threshold above which we spawn tasks for handler invocation instead of calling directly.
/// Below this threshold, direct invocation is faster due to tokio::spawn overhead.
const HANDLER_SPAWN_THRESHOLD: usize = 4;

/// Invokes a handler by spawning it on the tokio runtime.
/// Ensures async handlers are driven to completion even if not immediately ready.
#[inline]
fn invoke_handler_direct<T: Send + 'static>(
    handler: &Arc<dyn Fn(T) -> BoxedFuture<()> + Send + Sync>,
    msg: T,
    error_msg: &'static str,
) {
    let handler = handler.clone();
    tokio::spawn(async move {
        if let Err(e) = handler(msg).await {
            warn!("{}: {}", error_msg, e);
        }
    });
}

/// Spawns a handler call, logging any errors with the given message.
///
/// This eliminates the repeated `tokio::spawn(async move { if let Err(e) = handler(...).await {...} })`
/// pattern across all publish functions.
#[inline]
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
    // Collect handlers first to determine cloning strategy
    let handlers: Vec<super::TaskHandler> = broker
        .handlers
        .get(qname)
        .map(|entry| entry.value().clone())
        .unwrap_or_default();

    let handler_count = handlers.len();

    // Create Arc for task - only one clone needed
    let task_arc = Arc::new(task.clone());

    // Store the task
    broker
        .tasks
        .entry(qname.to_string())
        .or_default()
        .push(Arc::clone(&task_arc));

    // Invoke handlers based on count
    if handler_count > 0 {
        if handler_count < HANDLER_SPAWN_THRESHOLD {
            // Direct invocation for small handler counts
            for handler in handlers {
                invoke_handler_direct(&handler, Arc::clone(&task_arc), "task handler failed");
            }
        } else {
            // Spawn for larger handler counts
            for handler in handlers {
                spawn_handler(handler.clone(), Arc::clone(&task_arc), "task handler failed");
            }
        }
    }

    Box::pin(async { Ok(()) })
}

/// Publish multiple tasks to a queue.
pub(crate) fn tasks(
    broker: &InMemoryBroker,
    qname: &str,
    tasks: &[Task],
) -> super::super::BoxedFuture<()> {
    // Collect handlers first
    let handlers: Vec<super::TaskHandler> = broker
        .handlers
        .get(qname)
        .map(|entry| entry.value().clone())
        .unwrap_or_default();

    // Early exit if no handlers - just store
    if handlers.is_empty() {
        let task_arcs: Vec<Arc<Task>> = tasks.iter().map(|t| Arc::new(t.clone())).collect();
        broker
            .tasks
            .entry(qname.to_string())
            .or_default()
            .extend(task_arcs);
        return Box::pin(async { Ok(()) });
    }

    // Create Arcs for tasks - only one clone per task
    let task_arcs: Vec<Arc<Task>> = tasks.iter().map(|t| Arc::new(t.clone())).collect();

    // Store the tasks
    broker
        .tasks
        .entry(qname.to_string())
        .or_default()
        .extend(task_arcs.iter().cloned());

    let handler_count = handlers.len();
    let task_count = task_arcs.len();
    let total_invocations = task_count * handler_count;

    Box::pin(async move {
        // Invoke all registered handlers for each task
        if total_invocations < HANDLER_SPAWN_THRESHOLD {
            // Direct invocation for small total invocations
            for task_arc in &task_arcs {
                for handler in &handlers {
                    invoke_handler_direct(handler, Arc::clone(task_arc), "batch task handler failed");
                }
            }
        } else {
            // Spawn with bounded concurrency for larger workloads
            let mut jobs = Vec::with_capacity(total_invocations);
            for task_arc in &task_arcs {
                for handler in &handlers {
                    jobs.push((handler.clone(), Arc::clone(task_arc)));
                }
            }
            futures_util::stream::iter(jobs)
                .for_each_concurrent(256, |(handler, task)| async move {
                    if let Err(e) = handler(task).await {
                        warn!(error = %e, "batch task handler failed");
                    }
                })
                .await;
        }
        Ok(())
    })
}

/// Publish task progress.
pub(crate) fn task_progress(broker: &InMemoryBroker, task: &Task) -> BoxedFuture<()> {
    let handlers = broker.progress_handlers.clone();
    let task = task.clone(); // Clone once outside async block

    Box::pin(async move {
        let handlers = handlers.read().await;
        let handler_count = handlers.len();

        if handler_count == 0 {
            return Ok(());
        }

        if handler_count < HANDLER_SPAWN_THRESHOLD {
            for handler in handlers.iter() {
                invoke_handler_direct(handler, task.clone(), "progress handler failed");
            }
        } else {
            for handler in handlers.iter() {
                spawn_handler(handler.clone(), task.clone(), "progress handler failed");
            }
        }
        Ok(())
    })
}

/// Publish a heartbeat.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn heartbeat(broker: &InMemoryBroker, node: Node) -> BoxedFuture<()> {
    let handlers = broker.heartbeat_handlers.clone();
    let heartbeats = broker.heartbeats.clone();
    let node = node.clone(); // Clone once outside async block

    Box::pin(async move {
        if let Some(ref node_id) = node.id {
            heartbeats
                .write()
                .await
                .insert(node_id.to_string(), node.clone());
        }
        let handlers = handlers.read().await;
        let handler_count = handlers.len();

        if handler_count == 0 {
            return Ok(());
        }

        if handler_count < HANDLER_SPAWN_THRESHOLD {
            for handler in handlers.iter() {
                invoke_handler_direct(handler, node.clone(), "heartbeat handler failed");
            }
        } else {
            for handler in handlers.iter() {
                spawn_handler(handler.clone(), node.clone(), "heartbeat handler failed");
            }
        }
        Ok(())
    })
}

/// Publish a job.
pub(crate) fn job(broker: &InMemoryBroker, job: &Job) -> BoxedFuture<()> {
    let handlers = broker.job_handlers.clone();
    let job = job.clone(); // Clone once outside async block

    Box::pin(async move {
        let handlers = handlers.read().await;
        let handler_count = handlers.len();

        let job_id = job.id.as_deref().unwrap_or(DEFAULT_TASK_NAME);
        debug!("Publishing job {} to {} handlers", job_id, handler_count);

        if handler_count == 0 {
            return Ok(());
        }

        if handler_count < HANDLER_SPAWN_THRESHOLD {
            for handler in handlers.iter() {
                invoke_handler_direct(handler, job.clone(), "job handler failed");
            }
        } else {
            for handler in handlers.iter() {
                spawn_handler(handler.clone(), job.clone(), "job handler failed");
            }
        }
        Ok(())
    })
}

/// Publish an event to a topic.
///
/// This dispatches to both:
/// - Legacy `EventHandler` callbacks (for backward compatibility)
/// - Typed `JobEvent` broadcast channels (for the new subscribe API)
pub(crate) fn event(broker: &InMemoryBroker, topic: String, event: Value) -> BoxedFuture<()> {
    let handlers = broker.event_handlers.clone();
    let typed_channels = broker.typed_event_channels.clone();
    let event = event.clone(); // Clone once outside async block

    Box::pin(async move {
        // Dispatch to legacy callbacks
        for entry in handlers.iter() {
            let pattern = entry.key();
            if wildcard_match(pattern, &topic) {
                let event_clone = event.clone();
                let topic_handlers = entry.value().clone();
                for handler in topic_handlers {
                    let ev_clone = event_clone.clone();
                    spawn_handler(handler, ev_clone, "event handler failed");
                }
            }
        }

        // Dispatch to typed broadcast channels
        if let Ok(job) = serde_json::from_value::<Job>(event.clone()) {
            if let Some(typed_event) = twerk_core::job::job_event_from_state(&job) {
                for entry in typed_channels.iter() {
                    let pattern = entry.key();
                    if wildcard_match(pattern, &topic) {
                        let _ = entry.value().send(typed_event.clone());
                    }
                }
            }
        }

        Ok(())
    })
}

/// Publish a task log part.
pub(crate) fn task_log_part(broker: &InMemoryBroker, part: &TaskLogPart) -> BoxedFuture<()> {
    let handlers = broker.task_log_part_handlers.clone();
    let task_log_parts = broker.task_log_parts.clone();
    let part = part.clone(); // Clone once outside async block

    Box::pin(async move {
        if let Some(task_id) = &part.task_id {
            let task_id_str = task_id.to_string();
            let task_log_parts_guard = task_log_parts.write().await;
            let mut entry = task_log_parts_guard.entry(task_id_str).or_default();
            entry.push(part.clone());
        }
        let handlers = handlers.read().await;
        let handler_count = handlers.len();

        if handler_count == 0 {
            return Ok(());
        }

        if handler_count < HANDLER_SPAWN_THRESHOLD {
            for handler in handlers.iter() {
                invoke_handler_direct(handler, part.clone(), "task log part handler failed");
            }
        } else {
            for handler in handlers.iter() {
                spawn_handler(handler.clone(), part.clone(), "task log part handler failed");
            }
        }
        Ok(())
    })
}
