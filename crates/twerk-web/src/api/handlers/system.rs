//! System handlers - API endpoints for system operations.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use super::super::error::ApiError;
use super::{AppState, VERSION};
use tracing::instrument;

/// Health check handler
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
        (status = 200, description = "List of active nodes")
    )
)]
/// # Errors
#[instrument(name = "list_nodes_handler", skip_all)]
pub async fn list_nodes_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let nodes = state.ds.get_active_nodes().await.map_err(ApiError::from)?;
    Ok(axum::Json(nodes).into_response())
}

/// GET /metrics
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "System metrics")
    )
)]
/// # Errors
#[instrument(name = "get_metrics_handler", skip_all)]
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
///
/// # Errors
#[instrument(name = "create_user_handler", skip_all)]
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

    let user_id = twerk_core::id::UserId::new(twerk_core::uuid::new_short_uuid())?;

    let user = twerk_core::user::User {
        id: Some(user_id),
        username: Some(username),
        password_hash: Some(password_hash),
        ..Default::default()
    };

    state.ds.create_user(&user).await.map_err(ApiError::from)?;

    Ok(StatusCode::OK.into_response())
}
