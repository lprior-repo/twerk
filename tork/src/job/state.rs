//! Job state constants for the tork runtime.

pub type JobState = String;

pub const JOB_STATE_PENDING: &str = "PENDING";
pub const JOB_STATE_SCHEDULED: &str = "SCHEDULED";
pub const JOB_STATE_RUNNING: &str = "RUNNING";
pub const JOB_STATE_CANCELLED: &str = "CANCELLED";
pub const JOB_STATE_COMPLETED: &str = "COMPLETED";
pub const JOB_STATE_FAILED: &str = "FAILED";
pub const JOB_STATE_RESTART: &str = "RESTART";

pub type ScheduledJobState = String;

pub const SCHEDULED_JOB_STATE_ACTIVE: &str = "ACTIVE";
pub const SCHEDULED_JOB_STATE_PAUSED: &str = "PAUSED";
