//! Webhook event type constants.

/// Occurs when a task's state changes.
pub const EVENT_TASK_STATE_CHANGE: &str = "task.StateChange";

/// Occurs when a task's progress changes.
pub const EVENT_TASK_PROGRESS: &str = "task.Progress";

/// Occurs when a job's state changes.
pub const EVENT_JOB_STATE_CHANGE: &str = "job.StateChange";

/// Occurs when a job's progress changes.
pub const EVENT_JOB_PROGRESS: &str = "job.Progress";

/// Default event type (matches all events).
pub const EVENT_DEFAULT: &str = "";

/// Determines if an HTTP status code is retryable.
#[allow(dead_code)]
const fn is_retryable_status(status: u16) -> bool {
    matches!(
        status,
        429 | // Too Many Requests
        500 | // Internal Server Error
        502 | // Bad Gateway
        503 | // Service Unavailable
        504 // Gateway Timeout
    )
}
