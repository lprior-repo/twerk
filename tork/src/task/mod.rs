//! Domain types for task execution.
//!
//! This module provides the core task types used throughout the system.

pub mod impls;
pub mod state;
pub mod types;

pub use state::{TaskState, TASK_STATE_ACTIVE, TASK_STATE_CANCELLED, TASK_STATE_COMPLETED, TASK_STATE_CREATED, TASK_STATE_FAILED, TASK_STATE_PENDING, TASK_STATE_RUNNING, TASK_STATE_SCHEDULED, TASK_STATE_SKIPPED, TASK_STATE_STOPPED};
pub use types::{AutoDelete, EachTask, Mount, ParallelTask, Probe, Registry, SubJobTask, Task, TaskLimits, TaskLogPart, TaskRetry, TaskSummary, Webhook};
pub use impls::Task;
