//! Broker module for message queue and pub/sub functionality.
//!
//! This module provides broker implementations for delivering tasks
//! and coordinating between workers and the coordinator.

use std::sync::Arc;
use std::pin::Pin;
use std::future::Future;
use anyhow::Result;
use serde_json::Value;

use twerk_core::task::{Task, TaskLogPart};
use twerk_core::node::Node;
use twerk_core::job::Job;

pub type BoxedFuture<T> = Pin<Box<dyn Future<Output = Result<T>> + Send>>;
pub type BoxedHandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

pub type TaskHandler = Arc<dyn Fn(Arc<Task>) -> BoxedHandlerFuture + Send + Sync>;
pub type TaskProgressHandler = Arc<dyn Fn(Task) -> BoxedHandlerFuture + Send + Sync>;
pub type HeartbeatHandler = Arc<dyn Fn(Node) -> BoxedHandlerFuture + Send + Sync>;
pub type JobHandler = Arc<dyn Fn(Job) -> BoxedHandlerFuture + Send + Sync>;
pub type EventHandler = Arc<dyn Fn(Value) -> BoxedHandlerFuture + Send + Sync>;
pub type TaskLogPartHandler = Arc<dyn Fn(TaskLogPart) -> BoxedHandlerFuture + Send + Sync>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub size: i32,
    pub subscribers: i32,
    pub unacked: i32,
}

pub mod queue {
    pub const QUEUE_PENDING: &str = "x-pending";
    pub const QUEUE_COMPLETED: &str = "x-completed";
    pub const QUEUE_FAILED: &str = "x-failed";
    pub const QUEUE_STARTED: &str = "x-started";
    pub const QUEUE_HEARTBEAT: &str = "x-heartbeat";
    pub const QUEUE_JOBS: &str = "x-jobs";
    pub const QUEUE_PROGRESS: &str = "x-progress";
    pub const QUEUE_TASK_LOG_PART: &str = "x-task_log_part";
    pub const QUEUE_LOGS: &str = "x-task_log_part";
    pub const QUEUE_EXCLUSIVE_PREFIX: &str = "x-exclusive.";
    pub const QUEUE_REDELIVERIES: &str = "x-redeliveries";
}

pub fn is_coordinator_queue(qname: &str) -> bool {
    matches!(qname, queue::QUEUE_COMPLETED | queue::QUEUE_FAILED | queue::QUEUE_STARTED | queue::QUEUE_HEARTBEAT | queue::QUEUE_JOBS | queue::QUEUE_PROGRESS | queue::QUEUE_TASK_LOG_PART | queue::QUEUE_REDELIVERIES)
}

pub fn is_worker_queue(qname: &str) -> bool {
    !is_coordinator_queue(qname) && !qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX)
}

pub fn is_task_queue(qname: &str) -> bool {
    is_worker_queue(qname)
}

#[derive(Debug, Clone, Default)]
pub struct RabbitMQOptions {
    pub management_url: Option<String>,
    pub durable_queues: bool,
    pub queue_type: String,
    pub consumer_timeout: Option<std::time::Duration>,
}

pub trait Broker: Send + Sync {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()>;
    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()>;
    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()>;
    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()>;
    fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()>;
    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()>;
    fn publish_job(&self, job: &Job) -> BoxedFuture<()>;
    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()>;
    fn publish_event(&self, topic: String, event: Value) -> BoxedFuture<()>;
    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()>;
    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()>;
    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()>;
    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>>;
    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo>;
    fn delete_queue(&self, qname: String) -> BoxedFuture<()>;
    fn health_check(&self) -> BoxedFuture<()>;
    fn shutdown(&self) -> BoxedFuture<()>;
}

pub mod rabbitmq;
