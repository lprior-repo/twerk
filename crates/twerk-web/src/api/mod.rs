//! API module for the coordinator HTTP server.

use axum::routing::{delete, get, post, put};
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;
use tower_http::cors::CorsLayer;

pub mod handlers;
pub mod error;
pub mod redact;

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
fn is_enabled(enabled: &HashMap<String, bool>, key: &str) -> bool {
    enabled.get(key).copied().unwrap_or(true)
}

/// Shared application state, passed to all handlers via axum's `State`.
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
        Self { broker, ds, config }
    }
}

/// Create a new router with the given state and configured endpoints.
pub fn create_router(state: AppState) -> Router {
    let enabled = &state.config.enabled;

    let mut router = Router::new();

    // Apply layers (CORS, etc.) - simplified for now
    router = router.layer(CorsLayer::permissive());

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
            .route(
                "/jobs",
                post(handlers::create_job_handler).get(handlers::list_jobs_handler),
            )
            .route("/jobs/{id}", get(handlers::get_job_handler))
            .route("/jobs/{id}/log", get(handlers::get_job_log_handler))
            .route("/jobs/{id}/cancel", put(handlers::cancel_job_handler))
            .route("/jobs/{id}/restart", put(handlers::restart_job_handler))
            .route(
                "/scheduled-jobs",
                post(handlers::create_scheduled_job_handler)
                    .get(handlers::list_scheduled_jobs_handler),
            )
            .route(
                "/scheduled-jobs/{id}",
                get(handlers::get_scheduled_job_handler),
            )
            .route(
                "/scheduled-jobs/{id}/pause",
                put(handlers::pause_scheduled_job_handler),
            )
            .route(
                "/scheduled-jobs/{id}/resume",
                put(handlers::resume_scheduled_job_handler),
            )
            .route(
                "/scheduled-jobs/{id}",
                delete(handlers::delete_scheduled_job_handler),
            );
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
