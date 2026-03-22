//! Input module for Tork job and task definitions
//!
//! This module provides the input types for creating jobs and tasks,
//! along with validation functionality.

pub mod duration;
pub mod job;
pub mod task;
pub mod validate;

pub use job::{AutoDelete, Job, JobDefaults, Permission, ScheduledJob, Webhook};
pub use task::{
    AuxTask, Each, Limits, Mount, Parallel, Probe, Registry, Retry, SidecarTask, SubJob, Task,
};
pub use validate::{
    validate_job, validate_job_with_checker, validate_scheduled_job,
    validate_scheduled_job_with_checker, NoopPermissionChecker, PermissionChecker, ValidationError,
};
