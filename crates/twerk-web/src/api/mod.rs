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

#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

use axum::routing::{delete, get, post, put};
use axum::Router;
use std::collections::HashMap;
use std::sync::Arc;
use trigger_api::{InMemoryTriggerDatastore, TriggerAppState};
use twerk_app::engine::coordinator::auth::{
    basic_auth_middleware, key_auth_middleware, BasicAuthConfig, KeyAuthConfig,
};
use twerk_app::engine::coordinator::limits::{
    body_limit_middleware, rate_limit_middleware, BodyLimitConfig, RateLimitConfig,
};
use twerk_app::engine::coordinator::middleware::{cors_layer, http_log_middleware, HttpLogConfig};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

pub mod combinatorial;
pub mod domain;
pub mod error;
pub mod handlers;
pub mod redact;
pub mod trigger_api;
pub mod types;
pub mod yaml;

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
    pub trigger_state: TriggerAppState,
    pub config: Config,
}

impl AppState {
    #[must_use]
    pub fn new(broker: Arc<dyn Broker>, ds: Arc<dyn Datastore>, config: Config) -> Self {
        Self {
            broker,
            ds,
            trigger_state: TriggerAppState {
                trigger_ds: Arc::new(InMemoryTriggerDatastore::new()),
            },
            config,
        }
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
            .route(
                "/jobs/{id}/cancel",
                put(handlers::cancel_job_handler).post(handlers::cancel_job_handler),
            )
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

    router = router.route(
        "/api/v1/triggers",
        get(trigger_api::list_triggers_handler)
            .post(trigger_api::create_trigger_handler),
    );

    router = router.route(
        "/api/v1/triggers/{id}",
        get(trigger_api::get_trigger_handler)
            .put(trigger_api::update_trigger_handler)
            .delete(trigger_api::delete_trigger_handler),
    );

    router.with_state(state)
}

#[cfg(test)]
mod trigger_update_unit_red_tests {
    use std::collections::HashMap;

    use crate::api::trigger_api::{
        apply_trigger_update, validate_trigger_update, Trigger, TriggerId, TriggerUpdateError,
        TriggerUpdateRequest, ACTION_REQUIRED_MSG, EVENT_REQUIRED_MSG, METADATA_KEY_MSG,
        NAME_REQUIRED_MSG, TRIGGER_FIELD_MAX_LEN,
    };
    use time::OffsetDateTime;

    fn valid_request() -> TriggerUpdateRequest {
        TriggerUpdateRequest {
            name: "trigger-name".to_string(),
            enabled: true,
            event: "event.created".to_string(),
            condition: Some("x > 1".to_string()),
            action: "notify".to_string(),
            metadata: Some(HashMap::from([("k".to_string(), "v".to_string())])),
            id: None,
            version: Some(1),
        }
    }

    fn base_trigger() -> Trigger {
        let now = OffsetDateTime::UNIX_EPOCH;
        Trigger {
            id: TriggerId::parse("trg_1").expect("valid id"),
            name: "old".to_string(),
            enabled: false,
            event: "old.event".to_string(),
            condition: None,
            action: "old_action".to_string(),
            metadata: HashMap::from([("old".to_string(), "value".to_string())]),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn validate_trigger_update_returns_ok_trigger_id_when_inputs_are_valid() {
        let req = valid_request();
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Ok(TriggerId::from("trg_1"))
        );
    }

    #[test]
    fn validate_trigger_update_returns_invalid_id_format_when_path_id_is_invalid() {
        let req = valid_request();
        assert_eq!(
            validate_trigger_update("bad$id", &req),
            Err(TriggerUpdateError::InvalidIdFormat("bad$id".to_string()))
        );
    }

    #[test]
    fn validate_trigger_update_returns_exact_name_validation_error_when_name_is_blank() {
        let mut req = valid_request();
        req.name = "  ".to_string();
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Err(TriggerUpdateError::ValidationFailed(
                NAME_REQUIRED_MSG.to_string()
            ))
        );
    }

    #[test]
    fn validate_trigger_update_returns_exact_event_validation_error_when_event_is_blank() {
        let mut req = valid_request();
        req.event = "\n\t".to_string();
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Err(TriggerUpdateError::ValidationFailed(
                EVENT_REQUIRED_MSG.to_string()
            ))
        );
    }

    #[test]
    fn validate_trigger_update_returns_exact_action_validation_error_when_action_is_blank() {
        let mut req = valid_request();
        req.action = " ".to_string();
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Err(TriggerUpdateError::ValidationFailed(
                ACTION_REQUIRED_MSG.to_string()
            ))
        );
    }

    #[test]
    fn validate_trigger_update_returns_exact_metadata_validation_error_when_metadata_key_is_non_ascii_or_empty(
    ) {
        let mut req = valid_request();
        req.metadata = Some(HashMap::from([("ключ".to_string(), "v".to_string())]));
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Err(TriggerUpdateError::ValidationFailed(
                METADATA_KEY_MSG.to_string()
            ))
        );
    }

    #[test]
    fn validate_trigger_update_returns_id_mismatch_when_body_id_differs() {
        let mut req = valid_request();
        req.id = Some("trg_2".to_string());
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Err(TriggerUpdateError::IdMismatch {
                path_id: "trg_1".to_string(),
                body_id: "trg_2".to_string(),
            })
        );
    }

    #[test]
    fn validate_trigger_update_accepts_required_fields_when_length_equals_min_one() {
        let req = TriggerUpdateRequest {
            name: "n".to_string(),
            enabled: true,
            event: "e".to_string(),
            condition: None,
            action: "a".to_string(),
            metadata: None,
            id: None,
            version: Some(1),
        };
        assert_eq!(validate_trigger_update("x", &req), Ok(TriggerId::from("x")));
    }

    #[test]
    fn validate_trigger_update_accepts_required_fields_when_length_equals_max() {
        let max = "x".repeat(TRIGGER_FIELD_MAX_LEN);
        let req = TriggerUpdateRequest {
            name: max.clone(),
            enabled: true,
            event: max.clone(),
            condition: None,
            action: max,
            metadata: None,
            id: None,
            version: Some(1),
        };
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Ok(TriggerId::from("trg_1"))
        );
    }

    #[test]
    fn validate_trigger_update_rejects_required_field_when_length_exceeds_max_by_one() {
        let mut req = valid_request();
        req.name = "x".repeat(TRIGGER_FIELD_MAX_LEN + 1);
        assert_eq!(
            validate_trigger_update("trg_1", &req),
            Err(TriggerUpdateError::ValidationFailed(
                "name exceeds max length".to_string()
            ))
        );
    }

    #[test]
    fn validate_trigger_update_returns_invalid_id_format_when_id_length_exceeds_max() {
        let req = valid_request();
        let overlong = "a".repeat(crate::api::trigger_api::TRIGGER_ID_MAX_LEN + 1);
        assert_eq!(
            validate_trigger_update(&overlong, &req),
            Err(TriggerUpdateError::InvalidIdFormat(overlong))
        );
    }

    #[test]
    fn apply_trigger_update_returns_trigger_with_exact_mutable_projection_from_request() {
        let req = valid_request();
        let current = base_trigger();
        let now = current.updated_at + time::Duration::seconds(1);
        let result = apply_trigger_update(current, req.clone(), now).expect("valid apply");
        assert_eq!(result.name, req.name);
        assert_eq!(result.enabled, req.enabled);
        assert_eq!(result.event, req.event);
        assert_eq!(result.condition, req.condition);
        assert_eq!(result.action, req.action);
        assert_eq!(result.metadata, req.metadata.unwrap_or_default());
    }

    #[test]
    fn apply_trigger_update_preserves_id_and_created_at_when_request_valid() {
        let req = valid_request();
        let current = base_trigger();
        let id = current.id.clone();
        let created_at = current.created_at;
        let now = current.updated_at + time::Duration::seconds(1);
        let result = apply_trigger_update(current, req, now).expect("valid apply");
        assert_eq!(result.id, id);
        assert_eq!(result.created_at, created_at);
    }

    #[test]
    fn apply_trigger_update_sets_updated_at_when_now_equals_previous_updated_at() {
        let req = valid_request();
        let current = base_trigger();
        let now = current.updated_at;
        let result = apply_trigger_update(current, req, now).expect("valid apply");
        assert_eq!(result.updated_at, now);
    }

    #[test]
    fn apply_trigger_update_sets_updated_at_to_now_when_now_is_after_previous_updated_at() {
        let req = valid_request();
        let current = base_trigger();
        let now = current.updated_at + time::Duration::seconds(1);
        let result = apply_trigger_update(current, req, now).expect("valid apply");
        assert_eq!(result.updated_at, now);
    }

    #[test]
    fn apply_trigger_update_returns_exact_updated_at_validation_error_when_now_is_before_previous()
    {
        let req = valid_request();
        let current = base_trigger();
        let now = current.updated_at - time::Duration::nanoseconds(1);
        assert_eq!(
            apply_trigger_update(current, req, now),
            Err(TriggerUpdateError::ValidationFailed(
                crate::api::trigger_api::UPDATED_AT_BACKWARDS_MSG.to_string()
            ))
        );
    }

    #[test]
    fn apply_trigger_update_returns_exact_name_validation_error_when_name_blank_after_trim() {
        let mut req = valid_request();
        req.name = "   ".to_string();
        let current = base_trigger();
        assert_eq!(
            apply_trigger_update(current.clone(), req, current.updated_at),
            Err(TriggerUpdateError::ValidationFailed(
                NAME_REQUIRED_MSG.to_string()
            ))
        );
    }

    #[test]
    fn apply_trigger_update_returns_exact_event_validation_error_when_event_blank_after_trim() {
        let mut req = valid_request();
        req.event = "\t".to_string();
        let current = base_trigger();
        assert_eq!(
            apply_trigger_update(current.clone(), req, current.updated_at),
            Err(TriggerUpdateError::ValidationFailed(
                EVENT_REQUIRED_MSG.to_string()
            ))
        );
    }

    #[test]
    fn apply_trigger_update_returns_exact_action_validation_error_when_action_blank_after_trim() {
        let mut req = valid_request();
        req.action = " ".to_string();
        let current = base_trigger();
        assert_eq!(
            apply_trigger_update(current.clone(), req, current.updated_at),
            Err(TriggerUpdateError::ValidationFailed(
                ACTION_REQUIRED_MSG.to_string()
            ))
        );
    }

    #[test]
    fn apply_trigger_update_accepts_required_fields_when_lengths_equal_max() {
        let max = "x".repeat(TRIGGER_FIELD_MAX_LEN);
        let req = TriggerUpdateRequest {
            name: max.clone(),
            enabled: true,
            event: max.clone(),
            condition: None,
            action: max,
            metadata: None,
            id: None,
            version: Some(1),
        };
        let current = base_trigger();
        let result = apply_trigger_update(current.clone(), req, current.updated_at);
        assert_eq!(
            result.map(|trigger| trigger.name),
            Ok("x".repeat(TRIGGER_FIELD_MAX_LEN))
        );
    }

    #[test]
    fn apply_trigger_update_returns_validation_failed_when_required_field_exceeds_max_by_one() {
        let mut req = valid_request();
        req.event = "x".repeat(TRIGGER_FIELD_MAX_LEN + 1);
        let current = base_trigger();
        assert_eq!(
            apply_trigger_update(current.clone(), req, current.updated_at),
            Err(TriggerUpdateError::ValidationFailed(
                "event exceeds max length".to_string()
            ))
        );
    }
}
