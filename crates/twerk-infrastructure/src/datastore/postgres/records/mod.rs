//! PostgreSQL record types and conversions to domain types.
//!
//! This module organizes database records by domain concept:
//! - **task.rs**: TaskRecord and Task conversion
//! - **job.rs**: JobRecord and Job conversion
//! - **scheduled_job.rs**: ScheduledJobRecord and ScheduledJob conversion
//! - **node.rs**: NodeRecord and Node conversion
//! - **auth.rs**: UserRecord, RoleRecord, and permission records
//! - **log.rs**: TaskLogPartRecord
//! - **helpers.rs**: Shared helper functions

pub mod auth;
pub mod helpers;
pub mod job;
pub mod log;
pub mod node;
pub mod scheduled_job;
pub mod task;

// Re-export record types for backwards compatibility
pub use auth::{JobPermRecord, RoleRecord, ScheduledPermRecord, UserRecord};
pub use helpers::str_to_task_state;
pub use job::JobRecord;
pub use log::TaskLogPartRecord;
pub use node::NodeRecord;
pub use scheduled_job::ScheduledJobRecord;
pub use task::TaskRecord;

// Re-export conversion traits for backwards compatibility
// Note: In the future, prefer using these traits directly for type clarity
pub use auth::RoleRecordExt;
pub use auth::UserRecordExt;
pub use job::JobRecordExt;
pub use log::TaskLogPartRecordExt;
pub use node::NodeRecordExt;
pub use scheduled_job::ScheduledJobRecordExt;
pub use task::TaskRecordExt;
