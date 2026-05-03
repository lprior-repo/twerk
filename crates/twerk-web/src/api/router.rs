use axum::routing::{delete, get, post, put};
use axum::Router;

use twerk_app::engine::coordinator::auth::{basic_auth_middleware, key_auth_middleware};
use twerk_app::engine::coordinator::limits::{body_limit_middleware, rate_limit_middleware};
use twerk_app::engine::coordinator::middleware::{cors_layer, http_log_middleware};

use super::handlers;
use super::openapi;
use super::state::AppState;

fn is_enabled(enabled: &std::collections::HashMap<String, bool>, key: &str) -> bool {
    enabled.get(key).copied().unwrap_or(true)
}

#[allow(clippy::type_complexity, clippy::too_many_lines)]
pub fn create_router(state: AppState) -> Router {
    let enabled = &state.config.enabled;
    let mut router = Router::new();

    if let Some(body_limit) = state.config.body_limit.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            body_limit,
            |st, req, next| Box::pin(async move { body_limit_middleware(st, req, next).await }),
        ));
    }

    if twerk_infrastructure::config::bool("middleware.web.cors.enabled") {
        router = router.layer(cors_layer());
    }

    if let Some(basic_auth_config) = state.config.basic_auth.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            basic_auth_config,
            |st, req, next| Box::pin(async move { basic_auth_middleware(st, req, next).await }),
        ));
    }

    if let Some(key_auth_config) = state.config.key_auth.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            key_auth_config,
            |st, req, next| Box::pin(async move { key_auth_middleware(st, req, next).await }),
        ));
    }

    if let Some(rate_limit) = state.config.rate_limit.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            rate_limit,
            |st, req, next| Box::pin(async move { rate_limit_middleware(st, req, next).await }),
        ));
    }

    if let Some(http_log) = state.config.http_log.clone() {
        router = router.layer(axum::middleware::from_fn_with_state(
            http_log,
            |st, req, next| Box::pin(async move { http_log_middleware(st, req, next).await }),
        ));
    }

    router = mount_system_routes(router, enabled);
    router = mount_task_routes(router, enabled);
    router = mount_job_routes(router, enabled);
    router = mount_queue_routes(router, enabled);
    router = mount_trigger_routes(router);
    router = router.route("/openapi.json", get(serve_openapi_spec));

    router.with_state(state)
}

fn mount_system_routes(
    router: Router<AppState>,
    enabled: &std::collections::HashMap<String, bool>,
) -> Router<AppState> {
    let router = if is_enabled(enabled, "health") {
        router.route("/health", get(handlers::health_handler))
    } else {
        router
    };
    let router = if is_enabled(enabled, "nodes") {
        router
            .route("/nodes", get(handlers::list_nodes_handler))
            .route("/nodes/{id}", get(handlers::get_node_handler))
    } else {
        router
    };
    let router = if is_enabled(enabled, "metrics") {
        router.route("/metrics", get(handlers::get_metrics_handler))
    } else {
        router
    };

    if is_enabled(enabled, "users") {
        router.route("/users", post(handlers::create_user_handler))
    } else {
        router
    }
}

fn mount_task_routes(
    router: Router<AppState>,
    enabled: &std::collections::HashMap<String, bool>,
) -> Router<AppState> {
    if is_enabled(enabled, "tasks") {
        router
            .route("/tasks/{id}", get(handlers::get_task_handler))
            .route("/tasks/{id}/log", get(handlers::get_task_log_handler))
    } else {
        router
    }
}

fn mount_job_routes(
    router: Router<AppState>,
    enabled: &std::collections::HashMap<String, bool>,
) -> Router<AppState> {
    if !is_enabled(enabled, "jobs") {
        return router;
    }

    router
        .route(
            "/jobs",
            post(handlers::create_job_handler).get(handlers::list_jobs_handler),
        )
        .route("/jobs/{id}", get(handlers::get_job_handler))
        .route("/jobs/{id}/log", get(handlers::get_job_log_handler))
        .route(
            "/jobs/{id}/cancel",
            put(handlers::cancel_job_handler).post(handlers::cancel_job_handler_post),
        )
        .route("/jobs/{id}/restart", put(handlers::restart_job_handler))
        .route(
            "/scheduled-jobs",
            post(handlers::create_scheduled_job_handler).get(handlers::list_scheduled_jobs_handler),
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
        )
}

fn mount_queue_routes(
    router: Router<AppState>,
    enabled: &std::collections::HashMap<String, bool>,
) -> Router<AppState> {
    if is_enabled(enabled, "queues") {
        router
            .route("/queues", get(handlers::list_queues_handler))
            .route(
                "/queues/{name}",
                get(handlers::get_queue_handler).delete(handlers::delete_queue_handler),
            )
    } else {
        router
    }
}

fn mount_trigger_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            "/api/v1/triggers",
            get(handlers::list_triggers_handler).post(handlers::create_trigger_handler),
        )
        .route(
            "/api/v1/triggers/{id}",
            get(handlers::get_trigger_handler)
                .put(handlers::update_trigger_handler)
                .delete(handlers::delete_trigger_handler),
        )
}

async fn serve_openapi_spec() -> axum::http::Response<axum::body::Body> {
    openapi::generate_json()
        .map(|json| {
            axum::response::Response::builder()
                .status(200)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(json.into_bytes()))
                .unwrap()
        })
        .unwrap_or_else(|_| {
            axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::empty())
                .unwrap()
        })
}
