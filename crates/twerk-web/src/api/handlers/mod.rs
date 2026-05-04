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
use crate::api::domain::pagination::{Page, PageSize};
use crate::api::error::ApiError;

pub mod jobs;
pub mod queues;
pub mod scheduled;
pub mod system;
pub mod tasks;
pub mod triggers;

// Re-exports from submodules
pub use jobs::{
    cancel_job_handler, cancel_job_handler_post, create_job_handler, delete_job_handler,
    get_job_handler, get_job_log_handler, job_cancel_put, list_jobs_handler, restart_job_handler,
    CreateJobQuery, WaitMode,
};
pub use queues::{delete_queue_handler, get_queue_handler, list_queues_handler};
pub use scheduled::{
    create_scheduled_job_handler, delete_scheduled_job_handler, get_scheduled_job_handler,
    list_scheduled_jobs_handler, pause_scheduled_job_handler, resume_scheduled_job_handler,
    CreateScheduledJobBody,
};
pub use system::{
    create_user_handler, get_metrics_handler, get_node_handler, health_handler, list_nodes_handler,
    CreateUserBody,
};
pub use tasks::{get_task_handler, get_task_log_handler, PaginationQuery};
pub use triggers::{
    create_trigger_handler, delete_trigger_handler, get_trigger_handler, list_triggers_handler,
    update_trigger_handler,
};

// -----------------------------------------------------------------------------
// Shared helper functions (pub(super) for submodules)
// -----------------------------------------------------------------------------

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_integer_param(raw: &str, field_name: &str) -> Result<i64, ApiError> {
    raw.parse::<i64>()
        .map_err(|_| ApiError::bad_request(format!("{field_name} must be a positive integer")))
}

pub(super) fn parse_page(raw: Option<&str>) -> Result<i64, ApiError> {
    raw.map_or(Ok(1), |value| {
        let parsed = parse_integer_param(value, "page")?;
        Page::try_from(parsed)
            .map(i64::from)
            .map_err(|err| ApiError::bad_request(err.to_string()))
    })
}

pub(super) fn parse_size(raw: Option<&str>, default: i64, max: i64) -> Result<i64, ApiError> {
    raw.map_or(Ok(default), |value| {
        let parsed = parse_integer_param(value, "size")?;
        if parsed > max {
            return Err(ApiError::bad_request(format!(
                "page size {parsed} exceeds maximum allowed ({max})"
            )));
        }
        PageSize::try_from(parsed)
            .map(i64::from)
            .map_err(|err| ApiError::bad_request(err.to_string()))
    })
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
