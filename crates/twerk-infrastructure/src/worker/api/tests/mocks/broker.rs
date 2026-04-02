//! Mock broker implementation for testing.

#![allow(clippy::used_underscore_binding)]

use async_trait::async_trait;

use twerk_core::node::Node;
use twerk_core::task::Task;

use crate::broker::{
    BoxedFuture, Broker, EventHandler, HeartbeatHandler, JobHandler, TaskHandler,
    TaskLogPartHandler, TaskProgressHandler,
};

/// Mock broker implementation for testing
#[derive(Debug, Clone, Default)]
pub struct MockBroker;

#[async_trait]
impl Broker for MockBroker {
    fn publish_task(&self, _qname: String, _task: &Task) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn subscribe_for_tasks(&self, _qname: String, _handler: TaskHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn publish_task_progress(&self, _task: &Task) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn subscribe_for_task_progress(&self, _handler: TaskProgressHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn publish_heartbeat(&self, _node: Node) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn subscribe_for_heartbeats(&self, _handler: HeartbeatHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn publish_job(&self, _job: &twerk_core::job::Job) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn subscribe_for_jobs(&self, _handler: JobHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn publish_event(&self, _topic: String, _event: serde_json::Value) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn subscribe_for_events(&self, _pattern: String, _handler: EventHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn publish_task_log_part(&self, _part: &twerk_core::task::TaskLogPart) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn subscribe_for_task_log_part(&self, _handler: TaskLogPartHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn queues(&self) -> crate::broker::BoxedFuture<Vec<crate::broker::QueueInfo>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn queue_info(&self, _qname: String) -> crate::broker::BoxedFuture<crate::broker::QueueInfo> {
        Box::pin(async {
            Ok(crate::broker::QueueInfo {
                name: _qname,
                size: 0,
                subscribers: 0,
                unacked: 0,
            })
        })
    }
    fn delete_queue(&self, _qname: String) -> crate::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> crate::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}
