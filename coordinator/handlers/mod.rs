//! Handlers module for coordinator event handling.
//!
//! This module provides handlers for various events:
//! - [`cancel`]: Job cancellation handler
//! - [`completed`]: Task completion handler
//! - [`error`]: Task error/failure handler
//! - [`heartbeat`]: Node heartbeat handler
//! - [`job`]: Job state change handler
//! - [`log`]: Task log handler
//! - [`pending`]: Pending task handler
//! - [`progress`]: Task progress handler
//! - [`redelivered`]: Redelivered task handler
//! - [`schedule`]: Scheduled job handler
//! - [`started`]: Task started handler

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![allow(clippy::pedantic)]
#![warn(clippy::nursery)]

pub mod cancel;
pub mod completed;
pub mod error;
pub mod heartbeat;
pub mod job;
pub mod log;
pub mod pending;
pub mod progress;
pub mod redelivered;
pub mod schedule;
pub mod started;

#[cfg(test)]
pub mod test_helpers;

// Re-export handler constructor functions
pub use cancel::CancelHandler;
pub use completed::CompletedHandler;
pub use error::ErrorHandler;
pub use heartbeat::HeartbeatHandler;
pub use job::JobHandler;
pub use log::LogHandler;
pub use pending::PendingHandler;
pub use progress::ProgressHandler;
pub use redelivered::RedeliveredHandler;
pub use schedule::ScheduleHandler;
pub use started::StartedHandler;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tork::task::Task;

/// Maximum number of redeliveries before a task is marked as failed
pub const MAX_REDELIVERIES: i64 = 5;

/// Event types for task lifecycle events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEventType {
    Started,
    StateChange,
    Redelivered,
    Progress,
    Read,
}

impl TaskEventType {
    /// Returns the string representation of the event type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            TaskEventType::Started => "STARTED",
            TaskEventType::StateChange => "STATE_CHANGE",
            TaskEventType::Redelivered => "REDELIVERED",
            TaskEventType::Progress => "PROGRESS",
            TaskEventType::Read => "READ",
        }
    }
}

impl Default for TaskEventType {
    fn default() -> Self {
        Self::StateChange
    }
}

/// Event types for job lifecycle events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobEventType {
    StateChange,
    Progress,
    Read,
}

impl JobEventType {
    /// Returns the string representation of the event type.
    pub const fn as_str(&self) -> &'static str {
        match self {
            JobEventType::StateChange => "STATE_CHANGE",
            JobEventType::Progress => "PROGRESS",
            JobEventType::Read => "READ",
        }
    }
}

impl Default for JobEventType {
    fn default() -> Self {
        Self::StateChange
    }
}

/// Context for handler operations.
pub type HandlerContext = Arc<()>;

/// Handler function type for task events.
pub type TaskHandlerFunc =
    Arc<dyn Fn(HandlerContext, TaskEventType, &mut Task) -> Result<(), HandlerError> + Send + Sync>;

/// Handler function type for job events.
pub type JobHandlerFunc = Arc<
    dyn Fn(HandlerContext, JobEventType, &mut tork::job::Job) -> Result<(), HandlerError>
        + Send
        + Sync,
>;

/// Handler function type for node events.
pub type NodeHandlerFunc =
    Arc<dyn Fn(HandlerContext, &mut tork::node::Node) -> Result<(), HandlerError> + Send + Sync>;

/// Handler function type for log events.
pub type LogHandlerFunc =
    Arc<dyn Fn(HandlerContext, &[tork::task::TaskLogPart]) -> Result<(), HandlerError> + Send + Sync>;

/// Handler function type for scheduled job events.
pub type ScheduledJobHandlerFunc =
    Arc<dyn Fn(HandlerContext, &mut tork::job::ScheduledJob) -> Result<(), HandlerError> + Send + Sync>;

/// Errors that can occur in handlers.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HandlerError {
    #[error("handler error: {0}")]
    Handler(String),

    #[error("datastore error: {0}")]
    Datastore(String),

    #[error("broker error: {0}")]
    Broker(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("validation error: {0}")]
    Validation(String),
}

/// No-op handler that does nothing.
pub fn noop_task_handler() -> TaskHandlerFunc {
    Arc::new(|_ctx: HandlerContext, _et: TaskEventType, _task: &mut Task| Ok(()))
}

/// No-op job handler that does nothing.
pub fn noop_job_handler() -> JobHandlerFunc {
    Arc::new(|_ctx: HandlerContext, _et: JobEventType, _job: &mut tork::job::Job| Ok(()))
}

/// No-op node handler that does nothing.
pub fn noop_node_handler() -> NodeHandlerFunc {
    Arc::new(|_ctx: HandlerContext, _node: &mut tork::node::Node| Ok(()))
}

/// No-op log handler that does nothing.
pub fn noop_log_handler() -> LogHandlerFunc {
    Arc::new(|_ctx: HandlerContext, _logs: &[tork::task::TaskLogPart]| Ok(()))
}

/// No-op scheduled job handler that does nothing.
pub fn noop_scheduled_job_handler() -> ScheduledJobHandlerFunc {
    Arc::new(|_ctx: HandlerContext, _sj: &mut tork::job::ScheduledJob| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_event_type_as_str() {
        assert_eq!(TaskEventType::Started.as_str(), "STARTED");
        assert_eq!(TaskEventType::StateChange.as_str(), "STATE_CHANGE");
        assert_eq!(TaskEventType::Redelivered.as_str(), "REDELIVERED");
        assert_eq!(TaskEventType::Progress.as_str(), "PROGRESS");
        assert_eq!(TaskEventType::Read.as_str(), "READ");
    }

    #[test]
    fn test_job_event_type_as_str() {
        assert_eq!(JobEventType::StateChange.as_str(), "STATE_CHANGE");
        assert_eq!(JobEventType::Progress.as_str(), "PROGRESS");
        assert_eq!(JobEventType::Read.as_str(), "READ");
    }

    #[test]
    fn test_noop_handlers() {
        use tork::node::Node;
        use time::OffsetDateTime;

        let ctx = Arc::new(());
        let mut task = Task::default();
        assert!(noop_task_handler()(ctx.clone(), TaskEventType::StateChange, &mut task).is_ok());

        let mut job = tork::job::Job::default();
        assert!(noop_job_handler()(ctx.clone(), JobEventType::StateChange, &mut job).is_ok());

        let mut node = Node {
            id: Some("test-node".to_string()),
            name: Some("test-node".to_string()),
            started_at: OffsetDateTime::now_utc(),
            cpu_percent: 0.0,
            last_heartbeat_at: OffsetDateTime::now_utc(),
            queue: None,
            status: tork::node::NODE_STATUS_UP.to_string(),
            hostname: Some("localhost".to_string()),
            port: 8080,
            task_count: 0,
            version: "1.0.0".to_string(),
        };
        assert!(noop_node_handler()(ctx.clone(), &mut node).is_ok());

        let logs: Vec<tork::task::TaskLogPart> = vec![];
        assert!(noop_log_handler()(ctx.clone(), &logs).is_ok());

        let mut sj = tork::job::ScheduledJob::default();
        assert!(noop_scheduled_job_handler()(ctx.clone(), &mut sj).is_ok());
    }
}