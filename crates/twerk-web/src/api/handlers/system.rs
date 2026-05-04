//! System handlers - API endpoints for system operations.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use twerk_core::node::Node;
use twerk_core::stats::Metrics;

use super::super::domain::{Password, PasswordError, Username, UsernameError};
use super::super::error::ApiError;
use super::super::openapi_types::{HealthResponse, MessageResponse};
use super::{AppState, VERSION};
use tracing::instrument;
use utoipa::ToSchema;

/// Health check handler
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse, content_type = "application/json"),
        (status = 503, description = "Service is unhealthy", body = HealthResponse, content_type = "application/json")
    )
)]
#[instrument(name = "health_handler", skip_all)]
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
#[utoipa::path(
    get,
    path = "/nodes",
    responses(
        (status = 200, description = "List of active nodes", body = Vec<Node>, content_type = "application/json")
    )
)]
/// # Errors
#[instrument(name = "list_nodes_handler", skip_all)]
pub async fn list_nodes_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let nodes = state.ds.get_active_nodes().await.map_err(ApiError::from)?;
    Ok(axum::Json(nodes).into_response())
}

/// GET /nodes/{id}
///
/// # Errors
///
/// Returns an error if the node is not found or the database query fails.
#[utoipa::path(
    get,
    path = "/nodes/{id}",
    params(
        ("id" = String, description = "The node ID")
    ),
    responses(
        (status = 200, description = "Node found", body = Node, content_type = "application/json"),
        (status = 404, description = "Node not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "get_node_handler", skip_all)]
pub async fn get_node_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let node = state.ds.get_node_by_id(&id).await.map_err(ApiError::from)?;
    Ok(axum::Json(node).into_response())
}

/// GET /metrics
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "System metrics", body = Metrics, content_type = "application/json")
    )
)]
/// # Errors
#[instrument(name = "get_metrics_handler", skip_all)]
pub async fn get_metrics_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let metrics = state.ds.get_metrics().await.map_err(ApiError::from)?;
    Ok(axum::Json(metrics).into_response())
}

/// User creation body
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserBody {
    pub username: Option<String>,
    pub password: Option<String>,
}

fn username_error_to_string(err: &UsernameError) -> String {
    match err {
        UsernameError::Empty => "username cannot be empty".to_string(),
        UsernameError::LengthOutOfRange => "username must be 3-64 characters".to_string(),
        UsernameError::InvalidCharacter => {
            "username must start with a letter and contain only alphanumeric characters, underscores, or hyphens".to_string()
        }
    }
}

fn password_error_to_string(err: &PasswordError) -> String {
    match err {
        PasswordError::Empty => "password cannot be empty".to_string(),
        PasswordError::TooShort => "password must be at least 8 characters".to_string(),
    }
}

/// POST /users
///
/// # Errors
#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUserBody,
    responses(
        (status = 200, description = "User created"),
        (status = 400, description = "Missing username or password", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "create_user_handler", skip_all)]
pub async fn create_user_handler(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateUserBody>,
) -> Result<Response, ApiError> {
    let username = body
        .username
        .ok_or_else(|| ApiError::bad_request("username is required"))?;
    let username = Username::new(&username).map_err(|e| {
        ApiError::bad_request(format!(
            "invalid username: {}",
            username_error_to_string(&e)
        ))
    })?;

    let password = body
        .password
        .ok_or_else(|| ApiError::bad_request("password is required"))?;
    let password = Password::new(&password).map_err(|e| {
        ApiError::bad_request(format!(
            "invalid password: {}",
            password_error_to_string(&e)
        ))
    })?;

    let password_hash = bcrypt::hash(password.as_str(), bcrypt::DEFAULT_COST)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let user_id = twerk_core::id::UserId::new(twerk_core::uuid::new_short_uuid())?;

    let user = twerk_core::user::User {
        id: Some(user_id),
        username: Some(username.as_str().to_string()),
        password_hash: Some(password_hash),
        ..Default::default()
    };

    state.ds.create_user(&user).await.map_err(ApiError::from)?;

    Ok(StatusCode::OK.into_response())
}
