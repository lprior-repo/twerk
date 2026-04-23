//! Queue name constants and [`QueueInfo`] struct.

use utoipa::ToSchema;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct QueueInfo {
    pub name: String,
    pub size: i32,
    pub subscribers: i32,
    pub unacked: i32,
}
