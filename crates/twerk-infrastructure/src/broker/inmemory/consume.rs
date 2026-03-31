//! Queue consumption and management for the in-memory broker.

use super::super::{BoxedFuture, QueueInfo};
use super::InMemoryBroker;

/// Get all queues.
pub(crate) fn queues(broker: &InMemoryBroker) -> BoxedFuture<Vec<QueueInfo>> {
    let queues = broker
        .tasks
        .iter()
        .map(|entry| {
            let qname = entry.key().clone();
            let task_list = entry.value();
            let subscribers = broker
                .handlers
                .get(&qname)
                .map_or(0, |h| i32::try_from(h.len()).unwrap_or(0));
            QueueInfo {
                name: qname,
                size: i32::try_from(task_list.len()).unwrap_or(0),
                subscribers,
                unacked: 0,
            }
        })
        .collect();
    Box::pin(async { Ok(queues) })
}

/// Get information about a specific queue.
pub(crate) fn queue_info(broker: &InMemoryBroker, qname: String) -> BoxedFuture<QueueInfo> {
    let size = broker
        .tasks
        .get(&qname)
        .map_or(0, |entry| i32::try_from(entry.len()).unwrap_or(0));
    let subscribers = broker
        .handlers
        .get(&qname)
        .map_or(0, |entry| i32::try_from(entry.len()).unwrap_or(0));
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
    broker.tasks.remove(&qname);
    broker.handlers.remove(&qname);
    Box::pin(async { Ok(()) })
}
