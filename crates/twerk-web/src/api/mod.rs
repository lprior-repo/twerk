//! API module for the coordinator HTTP server.
//!
//! Go parity: internal/coordinator/api/api.go
//! Middleware ordering follows Go's engine/coordinator.go:
//! 1. Body limit (always applied)
//! 2. CORS (config-gated)
//! 3. Basic auth (config-gated)
//! 4. Key auth (config-gated)
//! 5. Rate limit (config-gated)
//! 6. Logger (default enabled)

use axum::routing::{delete, get, post, put};
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
use twerk_app::engine::coordinator::auth::{
    basic_auth_middleware, key_auth_middleware, BasicAuthConfig, KeyAuthConfig,
};
use twerk_app::engine::coordinator::limits::{
    body_limit_middleware, rate_limit_middleware, BodyLimitConfig, RateLimitConfig,
};
use twerk_app::engine::coordinator::middleware::{cors_layer, http_log_middleware, HttpLogConfig};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

pub mod error;
pub mod handlers;
pub mod redact;

#[derive(Clone)]
pub struct Config {
    pub address: String,
    pub enabled: HashMap<String, bool>,
    pub cors_origins: Vec<String>,
    pub basic_auth: Option<BasicAuthConfig>,
    pub key_auth: Option<KeyAuthConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub body_limit: Option<BodyLimitConfig>,
    pub http_log: Option<HttpLogConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "0.0.0.0:8000".to_string(),
            enabled: HashMap::new(),
            cors_origins: vec![],
            basic_auth: None,
            key_auth: None,
            rate_limit: None,
            body_limit: None,
            http_log: None,
        }
    }
}

fn is_enabled(enabled: &HashMap<String, bool>, key: &str) -> bool {
    enabled.get(key).copied().unwrap_or(true)
}

#[derive(Clone)]
pub struct AppState {
    pub broker: Arc<dyn Broker>,
    pub ds: Arc<dyn Datastore>,
    pub config: Config,
}

impl AppState {
    #[must_use]
    pub fn new(broker: Arc<dyn Broker>, ds: Arc<dyn Datastore>, config: Config) -> Self {
        Self { broker, ds, config }
    }
}

#[allow(clippy::type_complexity)]
pub fn create_router(state: AppState) -> Router {
    let enabled = &state.config.enabled;

    let mut router = Router::new();

    // Go parity: body limit always applied (default 500K)
    let body_limit = state.config.body_limit.clone();
    if let Some(bl) = body_limit {
        router = router.layer(axum::middleware::from_fn_with_state(bl, |st, req, next| {
            Box::pin(async move { body_limit_middleware(st, req, next).await })
        }));
    }

    // Go parity: CORS config-gated
    if twerk_infrastructure::config::bool("middleware.web.cors.enabled") {
        router = router.layer(cors_layer());
    }

    // Go parity: basic auth (config-gated)
    if let Some(basic_auth_config) = state.config.basic_auth.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            basic_auth_config,
            |st, req, next| Box::pin(async move { basic_auth_middleware(st, req, next).await }),
        ));
    }

    // Go parity: key auth (config-gated)
    if let Some(key_auth_config) = state.config.key_auth.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            key_auth_config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));
    }

    // Go parity: rate limit (config-gated)
    if let Some(rl) = state.config.rate_limit.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(rl, |st, req, next| {
            Box::pin(async move { rate_limit_middleware(st, req, next).await })
        }));
    }

    // Go parity: HTTP logger (default enabled)
    if let Some(http_log) = state.config.http_log.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            http_log,
            |st, req, next| Box::pin(async move { http_log_middleware(st, req, next).await }),
        ));
    }

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
