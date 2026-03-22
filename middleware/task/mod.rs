//! Task middleware module.
//!
//! This module provides middleware functionality for task event handling.

mod hostenv;
mod redact;
mod task_error;
mod webhook;
mod webhook_events;

pub use hostenv::HostEnv;
pub use redact::redact_middleware;
pub use task_error::TaskMiddlewareError;
pub use webhook::webhook_middleware;

pub use self::task_handler::{apply_middleware, noop_handler};

mod task_types {
    // Re-export types from tork crate
    pub use tork::job::{Job, JobContext, JobSummary};
    pub use tork::task::Webhook;
    pub use tork::task::{Task, TaskState, TaskSummary};

    // Re-export task state constants
    pub use tork::task::TASK_STATE_CANCELLED;
    pub use tork::task::TASK_STATE_COMPLETED;
    pub use tork::task::TASK_STATE_CREATED;
    pub use tork::task::TASK_STATE_FAILED;
    pub use tork::task::TASK_STATE_PENDING;
    pub use tork::task::TASK_STATE_RUNNING;
    pub use tork::task::TASK_STATE_SCHEDULED;
    pub use tork::task::TASK_STATE_SKIPPED;
    pub use tork::task::TASK_STATE_STOPPED;

    // Re-export job state constants
    pub use tork::JOB_STATE_CANCELLED;
    pub use tork::JOB_STATE_COMPLETED;
    pub use tork::JOB_STATE_FAILED;
    pub use tork::JOB_STATE_PENDING;
    pub use tork::JOB_STATE_RUNNING;
    pub use tork::JOB_STATE_SCHEDULED;

    /// Event types for task lifecycle events.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum EventType {
        Started,
        StateChange,
        Redelivered,
        Progress,
        Read,
    }

    impl EventType {
        pub const fn as_str(&self) -> &'static str {
            match self {
                EventType::Started => "STARTED",
                EventType::StateChange => "STATE_CHANGE",
                EventType::Redelivered => "REDELIVERED",
                EventType::Progress => "PROGRESS",
                EventType::Read => "READ",
            }
        }
    }

    impl From<&str> for EventType {
        fn from(s: &str) -> Self {
            match s {
                "STARTED" => EventType::Started,
                "STATE_CHANGE" => EventType::StateChange,
                "REDELIVERED" => EventType::Redelivered,
                "PROGRESS" => EventType::Progress,
                "READ" => EventType::Read,
                _ => EventType::Read,
            }
        }
    }

    impl std::fmt::Display for EventType {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_str())
        }
    }
}

mod task_handler {
    use super::task_types::*;
    use std::sync::Arc;

    /// Context type for task operations.
    pub type Context = Arc<std::sync::RwLock<()>>;

    /// Handler function type for task events.
    pub type HandlerFunc = Arc<
        dyn Fn(Context, EventType, &mut Task) -> Result<(), super::task_error::TaskMiddlewareError>
            + Send
            + Sync,
    >;

    /// No-op handler that does nothing.
    pub fn noop_handler() -> HandlerFunc {
        Arc::new(|_ctx: Context, _et: EventType, _task: &mut Task| Ok(()))
    }

    /// Middleware function type that wraps a handler.
    pub type MiddlewareFunc = Arc<dyn Fn(HandlerFunc) -> HandlerFunc + Send + Sync>;

    /// Applies a chain of middleware to a handler.
    pub fn apply_middleware(h: HandlerFunc, mws: &[MiddlewareFunc]) -> HandlerFunc {
        mws.iter().fold(h, |next, mw| mw(next))
    }
}
