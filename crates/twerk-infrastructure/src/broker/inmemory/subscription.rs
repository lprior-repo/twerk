//! Subscription management for the in-memory broker.

use super::super::{
    BoxedFuture, EventHandler, HeartbeatHandler, JobHandler, TaskHandler, TaskLogPartHandler,
    TaskProgressHandler,
};
use super::InMemoryBroker;
use tracing::debug;
use twerk_core::node::Node;
use twerk_core::task::TaskLogPart;

/// Subscribe for tasks on a queue.
pub(crate) fn for_tasks(
    broker: &InMemoryBroker,
    qname: String,
    handler: TaskHandler,
) -> BoxedFuture<()> {
    broker.handlers.entry(qname).or_default().push(handler);
    Box::pin(async { Ok(()) })
}

/// Subscribe for task progress updates.
pub(crate) fn for_task_progress(
    broker: &InMemoryBroker,
    handler: TaskProgressHandler,
) -> BoxedFuture<()> {
    let handlers = broker.progress_handlers.clone();
    Box::pin(async move {
        handlers.write().await.push(handler);
        Ok(())
    })
}

/// Subscribe for heartbeats.
pub(crate) fn for_heartbeats(
    broker: &InMemoryBroker,
    handler: HeartbeatHandler,
) -> BoxedFuture<()> {
    let handlers = broker.heartbeat_handlers.clone();
    let heartbeats = broker.heartbeats.clone();
    Box::pin(async move {
        // Use RwLock to ensure consistent iteration over heartbeats
        let nodes: Vec<Node> = heartbeats
            .read()
            .await
            .iter()
            .map(|e| e.value().clone())
            .collect();
        for node in nodes {
            let handler_clone = handler.clone();
            tokio::spawn(async move {
                let _ = handler_clone(node).await;
            });
        }
        handlers.write().await.push(handler);
        Ok(())
    })
}

/// Subscribe for jobs.
pub(crate) fn for_jobs(broker: &InMemoryBroker, handler: JobHandler) -> BoxedFuture<()> {
    let handlers = broker.job_handlers.clone();
    Box::pin(async move {
        debug!("Subscribing for jobs");
        handlers.write().await.push(handler);
        Ok(())
    })
}

/// Subscribe for events matching a pattern.
pub(crate) fn for_events(
    broker: &InMemoryBroker,
    pattern: String,
    handler: EventHandler,
) -> BoxedFuture<()> {
    broker
        .event_handlers
        .entry(pattern)
        .or_default()
        .push(handler);
    Box::pin(async { Ok(()) })
}

/// Subscribe for task log parts.
pub(crate) fn for_task_log_part(
    broker: &InMemoryBroker,
    handler: TaskLogPartHandler,
) -> BoxedFuture<()> {
    let handlers = broker.task_log_part_handlers.clone();
    let task_log_parts = broker.task_log_parts.clone();
    Box::pin(async move {
        // Use RwLock to ensure consistent iteration over task_log_parts
        let parts: Vec<TaskLogPart> = task_log_parts
            .read()
            .await
            .iter()
            .flat_map(|e| e.value().clone())
            .collect();
        for part in parts {
            let handler_clone = handler.clone();
            tokio::spawn(async move {
                let _ = handler_clone(part).await;
            });
        }
        handlers.write().await.push(handler);
        Ok(())
    })
}
