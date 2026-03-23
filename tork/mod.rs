//! Tork - Task queue and job scheduling system
//!
//! This crate provides the core data types and logic for a distributed
//! task execution system.
//!
//! # Architecture
//!
//! - **Data**: Domain types (Task, Job, Node, etc.) - pure data structs
//! - **Calc**: Pure calculation functions - cloning, state checks, etc.
//! - **Actions**: I/O operations at the boundary
//!
//! # Modules
//!
//! - [`task`]: Task-related types and functions
//! - [`job`]: Job-related types and functions
//! - [`node`]: Node-related types and functions
//! - [`user`]: User-related types and functions
//! - [`role`]: Role-related types and functions
//! - [`mount`]: Mount-related types and functions
//! - [`stats`]: Metrics and statistics types
//! - [`runtime`]: Runtime execution support

pub mod broker;
pub mod datastore;
pub mod eval;
pub mod job;
pub mod mount;
pub mod node;
pub mod role;
pub mod runtime;
pub mod stats;
pub mod task;
pub mod user;
pub mod version;
pub mod webhook;

// Re-export commonly used types
pub use broker::{Broker, QueueInfo};
pub use datastore::Datastore;
pub use job::{
    new_job_summary, new_scheduled_job_summary, Job, JobState, ScheduledJob,
    JOB_STATE_CANCELLED, JOB_STATE_COMPLETED, JOB_STATE_FAILED, JOB_STATE_PENDING,
    JOB_STATE_RESTART, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED, SCHEDULED_JOB_STATE_ACTIVE,
    SCHEDULED_JOB_STATE_PAUSED,
};
pub use mount::Mount;
pub use node::{Node, NodeStatus};
pub use runtime::Runtime;
pub use task::{Task, TaskState, TaskSummary};
pub use user::User;
pub use role::Role;
pub use version::VERSION;

// Test modules (conditionally compiled)
#[cfg(test)]
mod job_test;

#[cfg(test)]
mod task_test;

#[cfg(test)]
mod user_test;
