//! Broker module for message queue and pub/sub functionality.
//!
//! This module provides the broker interface for delivering tasks
//! and coordinating between workers and the coordinator.

use crate::node::Node;
use crate::task::Task;
use std::pin::Pin;
use std::sync::Arc;

/// Boxed future type for broker operations
pub type BoxedFuture<T> =
    Pin<Box<dyn std::future::Future<Output = Result<T, anyhow::Error>> + Send>>;

/// Boxed handler future type
pub type BoxedHandlerFuture = Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

/// Topic constant for job progress events
pub const TOPIC_JOB_PROGRESS: &str = "job.progress";

/// Queue names
pub mod queue {
    /// The queue used by the API to insert new tasks into
    pub const QUEUE_PENDING: &str = "pending";
    /// The queue used by workers to notify the coordinator that a task has began processing
    pub const QUEUE_STARTED: &str = "started";
    /// The queue used by workers to send tasks to when a task completes successfully
    pub const QUEUE_COMPLETED: &str = "completed";
    /// The queue used by workers to send tasks to when an error occurs in processing
    pub const QUEUE_ERROR: &str = "error";
    /// The default queue for tasks
    pub const QUEUE_DEFAULT: &str = "default";
    /// The queue used by workers to periodically notify the coordinator about their aliveness
    pub const QUEUE_HEARTBEAT: &str = "heartbeat";
    /// The queue used by the Coordinator for job creation and job-related state changes
    pub const QUEUE_JOBS: &str = "jobs";
    /// The queue used by workers to send task logs to the Coordinator
    pub const QUEUE_LOGS: &str = "logs";
    /// The queue used by workers to send task progress to the Coordinator
    pub const QUEUE_PROGRESS: &str = "progress";
    /// The queue used when a message is redelivered
    pub const QUEUE_REDELIVERIES: &str = "redeliveries";
    /// The prefix used for queues that are exclusive
    pub const QUEUE_EXCLUSIVE_PREFIX: &str = "x-";
}

/// Checks if a queue is a coordinator queue
#[must_use]
pub fn is_coordinator_queue(qname: &str) -> bool {
    matches!(
        qname,
        queue::QUEUE_PENDING
            | queue::QUEUE_STARTED
            | queue::QUEUE_COMPLETED
            | queue::QUEUE_ERROR
            | queue::QUEUE_HEARTBEAT
            | queue::QUEUE_JOBS
            | queue::QUEUE_LOGS
            | queue::QUEUE_PROGRESS
            | queue::QUEUE_REDELIVERIES
    )
}

/// Checks if a queue is a worker queue
#[must_use]
pub fn is_worker_queue(qname: &str) -> bool {
    !is_coordinator_queue(qname)
}

/// Checks if a queue is a task queue
#[must_use]
pub fn is_task_queue(qname: &str) -> bool {
    !is_coordinator_queue(qname) && !qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX)
}

/// Task handler type for subscribe_for_tasks
pub type TaskHandler = Arc<dyn Fn(Arc<Task>) -> BoxedHandlerFuture + Send + Sync + 'static>;

/// Task progress handler type
pub type TaskProgressHandler = Arc<dyn Fn(Task) -> BoxedHandlerFuture + Send + Sync + 'static>;

/// Heartbeat handler type
pub type HeartbeatHandler = Arc<dyn Fn(Node) -> BoxedHandlerFuture + Send + Sync + 'static>;

/// Job handler type
pub type JobHandler = Arc<dyn Fn(crate::job::Job) -> BoxedHandlerFuture + Send + Sync + 'static>;

/// Event handler type
pub type EventHandler =
    Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync + 'static>;

/// Task log part handler type
pub type TaskLogPartHandler =
    Arc<dyn Fn(crate::task::TaskLogPart) -> BoxedHandlerFuture + Send + Sync + 'static>;

/// Broker is the message-queue, pub/sub mechanism used for delivering tasks.
pub trait Broker: Send + Sync {
    // Task operations
    /// Publishes a task to a queue
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()>;
    /// Subscribes to a task queue
    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()>;

    // Task progress operations
    /// Publishes task progress
    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()>;
    /// Subscribes to task progress updates
    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()>;

    // Heartbeat operations
    /// Publishes a heartbeat
    fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()>;
    /// Subscribes to heartbeats
    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()>;

    // Job operations
    /// Publishes a job
    fn publish_job(&self, job: &crate::job::Job) -> BoxedFuture<()>;
    /// Subscribes to jobs
    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()>;

    // Event operations
    /// Publishes an event
    fn publish_event(&self, topic: String, event: serde_json::Value) -> BoxedFuture<()>;
    /// Subscribes to events
    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()>;

    // Task log operations
    /// Publishes a task log part
    fn publish_task_log_part(&self, part: &crate::task::TaskLogPart) -> BoxedFuture<()>;
    /// Subscribes to task log parts
    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()>;

    // Queue operations
    /// Returns information about all queues
    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>>;
    /// Returns information about a specific queue
    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo>;
    /// Deletes a queue
    fn delete_queue(&self, qname: String) -> BoxedFuture<()>;

    // Health operations
    /// Performs a health check
    fn health_check(&self) -> BoxedFuture<()>;
    /// Shuts down the broker
    fn shutdown(&self) -> BoxedFuture<()>;
}

/// Queue information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueInfo {
    /// Queue name
    pub name: String,
    /// Queue size
    pub size: i64,
    /// Number of subscribers
    pub subscribers: i64,
    /// Number of unacked messages
    pub unacked: i64,
}
