//! Input module for Tork job and task definitions
//!
//! This module provides the input types for creating jobs and tasks,
//! along with validation functionality.

pub mod duration;
pub mod job;
pub mod task;
pub mod validate;

pub use job::{AutoDelete, Job, ScheduledJob, JobDefaults, Permission, Webhook};
pub use task::{Task, SubJob, Each, Parallel, Retry, Limits, Registry, Mount, AuxTask, SidecarTask, Probe};
pub use validate::{ValidationError, validate_job, validate_scheduled_job};
