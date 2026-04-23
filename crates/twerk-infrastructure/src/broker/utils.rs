//! Queue name utility functions.

use super::queue;

/// Prefixes a queue name with `engine_id` if non-empty.
/// Returns queue unchanged when `engine_id` is empty (backward compatible).
#[must_use]
pub fn prefixed_queue(queue: &str, engine_id: &str) -> String {
    let trimmed = engine_id.trim();
    if trimmed.is_empty() || queue.ends_with(&format!(".{trimmed}")) {
        queue.to_string()
    } else {
        format!("{queue}.{trimmed}")
    }
}

pub(crate) fn coordinator_queue_names() -> [&'static str; 8] {
    [
        queue::QUEUE_COMPLETED,
        queue::QUEUE_FAILED,
        queue::QUEUE_STARTED,
        queue::QUEUE_HEARTBEAT,
        queue::QUEUE_JOBS,
        queue::QUEUE_PROGRESS,
        queue::QUEUE_TASK_LOG_PART,
        queue::QUEUE_REDELIVERIES,
    ]
}

fn base_queue_name(qname: &str) -> &str {
    coordinator_queue_names()
        .into_iter()
        .find(|queue_name| {
            let dotted = format!("{queue_name}.");
            qname.starts_with(&dotted)
        })
        .map_or(qname, |s| s)
}

/// Extracts `engine_id` from a prefixed queue name.
/// Returns `None` if the queue is not prefixed (no dot separator at the end).
#[must_use]
pub fn extract_engine_id(queue_name: &str) -> Option<String> {
    coordinator_queue_names()
        .into_iter()
        .find_map(|coordinator_queue| {
            let prefix = format!("{coordinator_queue}.");
            queue_name.strip_prefix(&prefix).and_then(|suffix| {
                let trimmed = suffix.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
        })
}

#[must_use]
pub fn is_coordinator_queue(qname: &str) -> bool {
    matches!(
        base_queue_name(qname),
        queue::QUEUE_COMPLETED
            | queue::QUEUE_FAILED
            | queue::QUEUE_STARTED
            | queue::QUEUE_HEARTBEAT
            | queue::QUEUE_JOBS
            | queue::QUEUE_PROGRESS
            | queue::QUEUE_TASK_LOG_PART
            | queue::QUEUE_REDELIVERIES
    )
}

#[must_use]
pub fn is_worker_queue(qname: &str) -> bool {
    !is_coordinator_queue(qname) && !qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX)
}

#[must_use]
pub fn is_task_queue(qname: &str) -> bool {
    is_worker_queue(qname)
}
