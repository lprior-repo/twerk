//! Task state constants and types.

/// TaskState represents the list of states that a task can be in at any given moment.
pub type TaskState = &'static str;

pub const TASK_STATE_CREATED: TaskState = "CREATED";
pub const TASK_STATE_PENDING: TaskState = "PENDING";
pub const TASK_STATE_SCHEDULED: TaskState = "SCHEDULED";
pub const TASK_STATE_RUNNING: TaskState = "RUNNING";
pub const TASK_STATE_CANCELLED: TaskState = "CANCELLED";
pub const TASK_STATE_STOPPED: TaskState = "STOPPED";
pub const TASK_STATE_COMPLETED: TaskState = "COMPLETED";
pub const TASK_STATE_FAILED: TaskState = "FAILED";
pub const TASK_STATE_SKIPPED: TaskState = "SKIPPED";

pub const TASK_STATE_ACTIVE: &[TaskState] = &[
    TASK_STATE_CREATED,
    TASK_STATE_PENDING,
    TASK_STATE_SCHEDULED,
    TASK_STATE_RUNNING,
];
