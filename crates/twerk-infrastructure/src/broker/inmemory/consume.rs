//! Queue consumption and management for the in-memory broker.

use super::super::{BoxedFuture, QueueInfo};
use super::InMemoryBroker;
use std::collections::BTreeSet;

// ── Typed errors for in-memory broker ──────────────────────────────

#[derive(Debug, thiserror::Error)]
#[error("queue {queue} not found")]
struct QueueNotFound {
    queue: String,
}

/// Get all queues.
pub(crate) fn queues(broker: &InMemoryBroker) -> BoxedFuture<Vec<QueueInfo>> {
    let names = broker
        .tasks
        .iter()
        .map(|entry| entry.key().clone())
        .chain(broker.handlers.iter().map(|entry| entry.key().clone()))
        .collect::<BTreeSet<_>>();
    let queues = names
        .into_iter()
        .map(|qname| queue_snapshot(broker, qname))
        .collect();
    Box::pin(async { Ok(queues) })
}

fn queue_snapshot(broker: &InMemoryBroker, qname: String) -> QueueInfo {
    let size = broker
        .tasks
        .get(&qname)
        .map_or(0, |entry| i32::try_from(entry.len()).unwrap_or(0));
    let subscribers = broker
        .handlers
        .get(&qname)
        .map_or(0, |entry| i32::try_from(entry.len()).unwrap_or(0));

    QueueInfo {
        name: qname,
        size,
        subscribers,
        unacked: 0,
    }
}

/// Get information about a specific queue.
pub(crate) fn queue_info(broker: &InMemoryBroker, qname: String) -> BoxedFuture<QueueInfo> {
    let task_entry = broker.tasks.get(&qname);
    let handler_entry = broker.handlers.get(&qname);

    if task_entry.is_none() && handler_entry.is_none() {
        return Box::pin(async move { Err(QueueNotFound { queue: qname }.into()) });
    }

    let size = task_entry.map_or(0, |entry| i32::try_from(entry.len()).unwrap_or(0));
    let subscribers = handler_entry.map_or(0, |entry| i32::try_from(entry.len()).unwrap_or(0));
    Box::pin(async move {
        Ok(QueueInfo {
            name: qname,
            size,
            subscribers,
            unacked: 0,
        })
    })
}

/// Delete a queue.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn delete_queue(broker: &InMemoryBroker, qname: String) -> BoxedFuture<()> {
    let task_entry = broker.tasks.get(&qname);
    let handler_entry = broker.handlers.get(&qname);

    if task_entry.is_none() && handler_entry.is_none() {
        return Box::pin(async move { Err(QueueNotFound { queue: qname }.into()) });
    }

    drop(task_entry);
    drop(handler_entry);
    broker.tasks.remove(&qname);
    broker.handlers.remove(&qname);
    Box::pin(async { Ok(()) })
}
