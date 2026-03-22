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
//!
//! # Architecture
//!
//! Follows Data→Calc→Actions: handlers extract data from HTTP requests,
//! call datastore/broker (boundary actions), and return JSON responses.
//! All error handling is explicit — no `unwrap()` or `panic!()`.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::pedantic)]

mod context;
pub mod error;
mod handlers;

pub use context::Context;

use axum::routing::{delete, get, post, put};
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tork::{Broker, Datastore};

/// Configuration for the API server.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to listen on (e.g. "0.0.0.0:8000")
    pub address: String,
    /// Enabled endpoint groups. Empty = all enabled.
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

/// Checks if an endpoint group is enabled.
///
/// Go parity: `if v, ok := cfg.Enabled["health"]; !ok || v { ... }`
fn is_enabled(enabled: &HashMap<String, bool>, key: &str) -> bool {
    enabled.get(key).copied().unwrap_or(true)
}

/// Shared application state, passed to all handlers via axum's `State`.
///
/// Holds `Arc<dyn Trait>` references to the broker and datastore so
/// the state is cheaply cloneable (required by axum).
#[derive(Clone)]
pub struct AppState {
    /// Message broker for pub/sub and task delivery.
    pub broker: Arc<dyn Broker>,
    /// Persistent datastore for jobs, tasks, nodes, users.
    pub ds: Arc<dyn Datastore>,
    /// API configuration.
    pub config: Config,
}

impl AppState {
    /// Create a new AppState with the given broker, datastore, and config.
    #[must_use]
    pub fn new(broker: Arc<dyn Broker>, ds: Arc<dyn Datastore>, config: Config) -> Self {
        Self {
            broker,
            ds,
            config,
        }
    }
}

/// Create a new router with the given state and configured endpoints.
///
/// Go parity: registers all routes from `NewAPI`, respecting the `Enabled` map.
pub fn create_router(state: AppState) -> Router {
    let enabled = &state.config.enabled;
    let cors = CorsLayer::new();

    let mut router = Router::new().layer(cors);

    // Health
    if is_enabled(enabled, "health") {
        router = router.route("/health", get(handlers::health_handler));
    }

    // Tasks
    if is_enabled(enabled, "tasks") {
        router = router
            .route("/tasks/{id}", get(handlers::get_task_handler))
            .route("/tasks/{id}/log", get(handlers::get_task_log_handler));
    }

    // Jobs
    if is_enabled(enabled, "jobs") {
        router = router
            .route("/jobs", post(handlers::create_job_handler).get(handlers::list_jobs_handler))
            .route("/jobs/{id}", get(handlers::get_job_handler))
            .route("/jobs/{id}/log", get(handlers::get_job_log_handler))
            .route("/jobs/{id}/cancel", put(handlers::cancel_job_handler))
            .route("/jobs/{id}/restart", put(handlers::restart_job_handler))
            .route(
                "/scheduled-jobs",
                post(handlers::create_scheduled_job_handler)
                    .get(handlers::list_scheduled_jobs_handler),
            )
            .route("/scheduled-jobs/{id}", get(handlers::get_scheduled_job_handler))
            .route("/scheduled-jobs/{id}/pause", put(handlers::pause_scheduled_job_handler))
            .route(
                "/scheduled-jobs/{id}/resume",
                put(handlers::resume_scheduled_job_handler),
            )
            .route("/scheduled-jobs/{id}", delete(handlers::delete_scheduled_job_handler));
    }

    // Queues
    if is_enabled(enabled, "queues") {
        router = router
            .route("/queues", get(handlers::list_queues_handler))
            .route(
                "/queues/{name}",
                get(handlers::get_queue_handler).delete(handlers::delete_queue_handler),
            );
    }

    // Nodes
    if is_enabled(enabled, "nodes") {
        router = router.route("/nodes", get(handlers::list_nodes_handler));
    }

    // Metrics
    if is_enabled(enabled, "metrics") {
        router = router.route("/metrics", get(handlers::get_metrics_handler));
    }

    // Users
    if is_enabled(enabled, "users") {
        router = router.route("/users", post(handlers::create_user_handler));
    }

    router.with_state(state)
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
    fn test_is_enabled_default() {
        let enabled = HashMap::new();
        assert!(is_enabled(&enabled, "health"));
        assert!(is_enabled(&enabled, "jobs"));
    }

    #[test]
    fn test_is_enabled_explicit() {
        let mut enabled = HashMap::new();
        enabled.insert("health".to_string(), false);
        enabled.insert("jobs".to_string(), true);
        assert!(!is_enabled(&enabled, "health"));
        assert!(is_enabled(&enabled, "jobs"));
        assert!(is_enabled(&enabled, "metrics")); // not in map → default true
    }
}
