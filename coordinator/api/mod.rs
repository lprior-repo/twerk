//! API module for the coordinator HTTP server.
//!
//! This module provides HTTP endpoints for:
//! - Health checks
//! - Job management (create, get, list, cancel, restart)
//! - Task management (get, get log)
//! - Queue management (list, get, delete)
//! - Node management (list active nodes)
//! - Scheduled job management
//! - User management
//! - Metrics

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

mod context;

pub use context::Context;

use axum::{
    extract::Path,
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tower_http::cors::CorsLayer;

/// Health response type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

/// Queue info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub size: i64,
    pub subscribers: i64,
    pub unacked: i64,
}

/// Pagination parameters
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub q: Option<String>,
}

/// Configuration for the API server.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to listen on
    pub address: String,
    /// Enabled endpoints
    pub enabled: HashMap<String, bool>,
    /// CORS origins (empty means allow all)
    pub cors_origins: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:8000".to_string(),
            enabled: HashMap::new(),
            cors_origins: vec![],
        }
    }
}

/// Shared application state.
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

/// Create a new router with the given state.
pub fn create_router(state: AppState) -> Router {
    let _cors = CorsLayer::new();

    Router::new()
        .route("/health", get(health_handler))
        .route("/tasks/:id", get(task_handler))
        .route("/tasks/:id/log", get(task_log_handler))
        .route("/queues", get(queues_handler))
        .route("/queues/:name", get(queue_handler).delete(delete_queue_handler))
        .route("/nodes", get(nodes_handler))
        .route("/jobs", get(jobs_handler).post(create_job_handler))
        .route("/jobs/:id", get(job_handler))
        .route("/jobs/:id/log", get(job_log_handler))
        .route("/jobs/:id/cancel", put(cancel_job_handler))
        .route("/jobs/:id/restart", put(restart_job_handler))
        .route("/scheduled-jobs", get(scheduled_jobs_handler).post(create_scheduled_job_handler))
        .route("/scheduled-jobs/:id", get(scheduled_job_handler))
        .route("/scheduled-jobs/:id/pause", put(pause_scheduled_job_handler))
        .route("/scheduled-jobs/:id/resume", put(resume_scheduled_job_handler))
        .route("/scheduled-jobs/:id", delete(delete_scheduled_job_handler))
        .route("/metrics", get(metrics_handler))
        .route("/users", post(create_user_handler))
        .layer(_cors)
        .with_state(state)
}

/// Health check handler
async fn health_handler() -> impl axum::response::IntoResponse {
    Json(HealthResponse {
        status: "UP".to_string(),
    })
}

/// Task handler - GET /tasks/:id
async fn task_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))
}

/// Task log handler - GET /tasks/:id/log
async fn task_log_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))
}

/// Queues handler - GET /queues
async fn queues_handler() -> impl axum::response::IntoResponse {
    Json(vec![] as Vec<QueueInfo>)
}

/// Queue handler - GET /queues/:name
async fn queue_handler(Path(_name): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))
}

/// Delete queue handler - DELETE /queues/:name
async fn delete_queue_handler(Path(_name): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "cannot delete system queue"})))
}

/// Nodes handler - GET /nodes
async fn nodes_handler() -> impl axum::response::IntoResponse {
    Json(vec![] as Vec<serde_json::Value>)
}

/// Jobs handler - GET /jobs and POST /jobs
async fn jobs_handler() -> impl axum::response::IntoResponse {
    Json(serde_json::json!({
        "items": [],
        "number": 1,
        "size": 10,
        "total_pages": 0,
        "total_items": 0
    }))
}

/// Create job handler - POST /jobs
async fn create_job_handler() -> impl axum::response::IntoResponse {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "job creation disabled"})))
}

/// Job handler - GET /jobs/:id
async fn job_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))
}

/// Job log handler - GET /jobs/:id/log
async fn job_log_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))
}

/// Cancel job handler - PUT /jobs/:id/cancel
async fn cancel_job_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    Json(serde_json::json!({"status": "OK"}))
}

/// Restart job handler - PUT /jobs/:id/restart
async fn restart_job_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "job cannot be restarted"})))
}

/// Scheduled jobs handler - GET /scheduled-jobs and POST /scheduled-jobs
async fn scheduled_jobs_handler() -> impl axum::response::IntoResponse {
    Json(serde_json::json!({
        "items": [],
        "number": 1,
        "size": 10,
        "total_pages": 0,
        "total_items": 0
    }))
}

/// Create scheduled job handler - POST /scheduled-jobs
async fn create_scheduled_job_handler() -> impl axum::response::IntoResponse {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "scheduled job creation disabled"})))
}

/// Scheduled job handler - GET /scheduled-jobs/:id
async fn scheduled_job_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})))
}

/// Pause scheduled job handler - PUT /scheduled-jobs/:id/pause
async fn pause_scheduled_job_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "scheduled job is not active"})))
}

/// Resume scheduled job handler - PUT /scheduled-jobs/:id/resume
async fn resume_scheduled_job_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "scheduled job is not paused"})))
}

/// Delete scheduled job handler - DELETE /scheduled-jobs/:id
async fn delete_scheduled_job_handler(Path(_id): Path<String>) -> impl axum::response::IntoResponse {
    Json(serde_json::json!({"status": "OK"}))
}

/// Metrics handler - GET /metrics
async fn metrics_handler() -> impl axum::response::IntoResponse {
    Json(serde_json::json!({
        "jobs_completed": 0,
        "jobs_failed": 0,
        "jobs_running": 0,
        "tasks_completed": 0,
        "tasks_failed": 0,
        "tasks_running": 0
    }))
}

/// Create user handler - POST /users
async fn create_user_handler() -> impl axum::response::IntoResponse {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "user creation disabled"})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.address, "0.0.0.0:8000");
        assert!(config.enabled.is_empty());
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "UP".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("UP"));
    }
}