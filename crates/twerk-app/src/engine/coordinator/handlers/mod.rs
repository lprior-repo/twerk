//! Event handlers for the coordinator
//!
//! This module provides handlers for job, task, and health events
//! processed by the coordinator.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

mod cancellation;
mod health;
mod job_handlers;
mod retry;
mod subtask_handlers;
mod task_handlers;
mod task_workflow;
mod util;

// ── Typed errors shared across handlers ────────────────────────────

#[derive(Debug, thiserror::Error)]
pub(crate) enum HandlerError {
    #[error("task has no id")]
    MissingTaskId,
    #[error("task has no job_id")]
    MissingJobId,
    #[error("parent task has no id")]
    MissingParentTaskId,
    #[error("job has no tasks")]
    JobHasNoTasks,
    #[error("task out of bounds")]
    TaskOutOfBounds,
    #[error("task has no retry config")]
    MissingRetryConfig,
    #[error("redelivered task has no id")]
    RedeliveredMissingTaskId,
}

// Re-export public APIs from submodules
pub use health::handle_heartbeat;
pub use job_handlers::handle_cancel;
pub use job_handlers::handle_job_event;
pub use task_handlers::handle_error;
pub use task_handlers::handle_log_part;
pub use task_handlers::handle_pending_task;
pub use task_handlers::handle_redelivered;
pub use task_handlers::handle_started;
pub use task_handlers::handle_task_completed;
pub use task_handlers::handle_task_failed;
pub use task_handlers::handle_task_progress;
