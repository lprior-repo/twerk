//! API handlers module - re-exports and aggregates all handler functions.
//!
//! Split into logical resource-based files:
//! - tasks.rs: Task handlers
//! - jobs.rs: Job handlers
//! - queues.rs: Queue handlers
//! - scheduled.rs: Scheduled job handlers
//! - system.rs: System-level handlers (health, nodes, metrics, users)

use axum::extract::Request;
use twerk_core::user::UsernameValue;

use super::AppState;

pub mod jobs;
pub mod queues;
pub mod scheduled;
pub mod system;
pub mod tasks;

// Re-exports from submodules
pub use jobs::{
    cancel_job_handler, create_job_handler, get_job_handler, get_job_log_handler,
    list_jobs_handler, restart_job_handler, CreateJobQuery, WaitMode,
};
pub use queues::{delete_queue_handler, get_queue_handler, list_queues_handler};
pub use scheduled::{
    create_scheduled_job_handler, delete_scheduled_job_handler, get_scheduled_job_handler,
    list_scheduled_jobs_handler, pause_scheduled_job_handler, resume_scheduled_job_handler,
    CreateScheduledJobBody,
};
pub use system::{
    create_user_handler, get_metrics_handler, health_handler, list_nodes_handler, CreateUserBody,
};
pub use tasks::{get_task_handler, get_task_log_handler, PaginationQuery};

// -----------------------------------------------------------------------------
// Shared helper functions (pub(super) for submodules)
// -----------------------------------------------------------------------------

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(super) fn parse_page(p: Option<i64>) -> i64 {
    p.filter(|&v| v >= 1).unwrap_or(1)
}

pub(super) fn parse_size(p: Option<i64>, default: i64, max: i64) -> i64 {
    p.filter(|&v| v >= 1).unwrap_or(default).clamp(1, max)
}

pub(super) fn extract_current_user(req: &Request) -> String {
    req.extensions()
        .get::<UsernameValue>()
        .map(|v| v.0.clone())
        .unwrap_or_default()
}

pub(super) async fn default_user(state: &AppState) -> Option<twerk_core::user::User> {
    state.ds.get_user("guest").await.ok()
}
