//! API handlers module - re-exports and aggregates all handler functions.
//!
//! Split into logical resource-based files:
//! - tasks.rs: Task handlers
//! - jobs.rs: Job handlers
//! - queues.rs: Queue handlers
//! - scheduled.rs: Scheduled job handlers

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use twerk_core::user::UsernameValue;

pub mod jobs;
pub mod queues;
pub mod scheduled;
pub mod tasks;

// Re-exports from submodules
pub use jobs::{
    cancel_job_handler, create_job_handler, get_job_handler, get_job_log_handler,
    list_jobs_handler, restart_job_handler, CreateJobQuery,
};
pub use queues::{delete_queue_handler, get_queue_handler, list_queues_handler};
pub use scheduled::{
    create_scheduled_job_handler, delete_scheduled_job_handler, get_scheduled_job_handler,
    list_scheduled_jobs_handler, pause_scheduled_job_handler, resume_scheduled_job_handler,
    CreateScheduledJobBody,
};
pub use tasks::{get_task_handler, get_task_log_handler, PaginationQuery};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// -----------------------------------------------------------------------------
// Helper functions
// -----------------------------------------------------------------------------

fn parse_page(p: Option<i64>) -> i64 {
    p.filter(|&v| v >= 1).unwrap_or(1)
}

fn parse_size(p: Option<i64>, default: i64, max: i64) -> i64 {
    p.filter(|&v| v >= 1).unwrap_or(default).clamp(1, max)
}

fn extract_current_user(req: &Request) -> String {
    req.extensions()
        .get::<UsernameValue>()
        .map(|v| v.0.clone())
        .unwrap_or_default()
}

async fn default_user(state: &AppState) -> Option<twerk_core::user::User> {
    state.ds.get_user("guest").await.ok()
}

// -----------------------------------------------------------------------------
// Remaining handlers
// -----------------------------------------------------------------------------

use super::error::ApiError;
use super::AppState;

/// Health check handler
pub async fn health_handler(State(state): State<AppState>) -> Response {
    let ds_ok = state.ds.health_check().await.is_ok();
    let broker_ok = state.broker.health_check().await.is_ok();

    let (status, body) = if ds_ok && broker_ok {
        (StatusCode::OK, json!({"status": "UP", "version": VERSION}))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"status": "DOWN", "version": VERSION}),
        )
    };
    (status, axum::Json(body)).into_response()
}

/// GET /nodes
pub async fn list_nodes_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let nodes = state.ds.get_active_nodes().await.map_err(ApiError::from)?;
    Ok(axum::Json(nodes).into_response())
}

/// GET /metrics
pub async fn get_metrics_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let metrics = state.ds.get_metrics().await.map_err(ApiError::from)?;
    Ok(axum::Json(metrics).into_response())
}

/// User creation body
#[derive(Debug, Deserialize)]
pub struct CreateUserBody {
    pub username: Option<String>,
    pub password: Option<String>,
}

/// POST /users
pub async fn create_user_handler(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateUserBody>,
) -> Result<Response, ApiError> {
    let username = body
        .username
        .ok_or_else(|| ApiError::bad_request("missing username"))?;
    let password = body
        .password
        .ok_or_else(|| ApiError::bad_request("missing password"))?;

    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let user = twerk_core::user::User {
        id: None,
        username: Some(username),
        password_hash: Some(password_hash),
        ..Default::default()
    };

    state.ds.create_user(&user).await.map_err(ApiError::from)?;

    Ok(StatusCode::OK.into_response())
}
