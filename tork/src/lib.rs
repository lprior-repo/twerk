//! Tork Domain Types
//!
//! This crate defines all domain types for the tork system,
//! allowing other crates to use `tork::Job`, `tork::Task`, `tork::Node`, etc.
//!
//! # Example
//!
//! ```rust
//! use tork::{Job, Task, Node};
//! ```

#![deny(clippy::unwrap_used)]
#![allow(clippy::pedantic)]

// Domain type modules
pub mod job;
pub mod node;
pub mod role;
pub mod stats;
pub mod task;
pub mod user;

// Re-export Job domain types
pub use job::{
    AutoDelete, EachTask, Job, JobContext, JobDefaults, JobSchedule, JobState, JobSummary,
    Mount, ParallelTask, Permission, Probe, Registry, Role, ScheduledJob, ScheduledJobState,
    SubJobTask, Task, TaskLimits, TaskRetry, User, Webhook, JOB_STATE_CANCELLED,
    JOB_STATE_COMPLETED, JOB_STATE_FAILED, JOB_STATE_PENDING, JOB_STATE_RESTART,
    JOB_STATE_RUNNING, JOB_STATE_SCHEDULED, SCHEDULED_JOB_STATE_ACTIVE, SCHEDULED_JOB_STATE_PAUSED,
};

// Re-export Task domain types (aliased from job module)
pub use job::{
    TaskAutoDelete, TaskEachTask, TaskLogPart, TaskMount, TaskParallelTask, TaskProbe,
    TaskRegistry, TaskState, TaskSubJobTask, TaskSummary, TaskTask, TaskTaskLimits,
    TaskTaskRetry, TaskWebhook,
};

// Re-export Node domain types
pub use node::{Node, NodeStatus, HEARTBEAT_RATE, LAST_HEARTBEAT_TIMEOUT};

// Re-export Stats domain types
pub use stats::{JobMetrics, Metrics, NodeMetrics, TaskMetrics};

// Re-export Role domain types
pub use role::{Role as RoleType, ROLE_PUBLIC, UserRole};

// Re-export User domain types
pub use user::{User as UserType, USERNAME, USER_GUEST};
