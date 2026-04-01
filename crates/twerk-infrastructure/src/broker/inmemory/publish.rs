//! Publishing logic for the in-memory broker.

use super::super::BoxedFuture;
use super::InMemoryBroker;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;
use twerk_common::wildcard::wildcard_match;
use twerk_core::job::Job;
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

/// Publish a task to a queue.
pub(crate) fn task(broker: &InMemoryBroker, qname: &str, task: &Task) -> BoxedFuture<()> {
    let task = Arc::new(task.clone());

    // Store the task
    broker
        .tasks
        .entry(qname.to_string())
        .or_default()
        .push(Arc::clone(&task));

    // Collect handlers for this queue before spawning tasks
    let handlers: Vec<super::TaskHandler> = broker
        .handlers
        .get(qname)
        .map(|entry| entry.value().clone())
        .unwrap_or_default();

    // Invoke all registered handlers for this queue
    for handler in handlers {
        let task_clone = Arc::clone(&task);
        tokio::spawn(async move {
            let _ = handler(task_clone).await;
        });
    }

    Box::pin(async { Ok(()) })
}

/// Publish multiple tasks to a queue.
pub(crate) fn tasks(broker: &InMemoryBroker, qname: &str, tasks: &[Task]) -> super::super::BoxedFuture<()> {
    let mut task_arcs = Vec::with_capacity(tasks.len());
    for t in tasks {
        task_arcs.push(Arc::new(t.clone()));
    }

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

    // Invoke all registered handlers for this queue
    for task_arc in task_arcs {
        for handler in &handlers {
            let task_clone = Arc::clone(&task_arc);
            let handler_clone = Arc::clone(handler);
            tokio::spawn(async move {
                let _ = handler_clone(task_clone).await;
            });
        }
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
            let handler_clone = handler.clone();
            tokio::spawn(async move {
                let _ = handler_clone(task_clone).await;
            });
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
            heartbeats
                .write()
                .await
                .insert(node_id.to_string(), node.clone());
        }
        let handlers = handlers.read().await;
        for handler in handlers.iter() {
            let node_clone = node.clone();
            let handler_clone = handler.clone();
            tokio::spawn(async move {
                let _ = handler_clone(node_clone).await;
            });
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
            job.id.as_deref().unwrap_or("unknown"),
            handlers.len()
        );
        for handler in handlers.iter() {
            let job_clone = job.clone();
            let handler_clone = handler.clone();
            tokio::spawn(async move {
                let _ = handler_clone(job_clone).await;
            });
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
                    let h_clone = handler.clone();
                    tokio::spawn(async move {
                        let _ = h_clone(ev_clone).await;
                    });
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
            let task_log_parts_guard = task_log_parts.write().await;
            let mut entry = task_log_parts_guard.entry(task_id_str).or_default();
            entry.push(part.clone());
        }
        let handlers = handlers.read().await;
        for handler in handlers.iter() {
            let part_clone = part.clone();
            let handler_clone = handler.clone();
            tokio::spawn(async move {
                let _ = handler_clone(part_clone).await;
            });
        }
        Ok(())
    })
}
