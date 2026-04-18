//! Queue handlers - API endpoints for queue operations.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::super::error::ApiError;
use super::AppState;
use tracing::instrument;

#[utoipa::path(
    get,
    path = "/queues",
    responses(
        (status = 200, description = "List of queues")
    )
)]
/// GET /queues
///
/// # Errors
#[instrument(name = "list_queues_handler", skip_all)]
pub async fn list_queues_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let queues = state
        .broker
        .queues()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(axum::Json(queues).into_response())
}

#[utoipa::path(
    get,
    path = "/queues/{name}",
    params(
        ("name" = String, Path, description = "Queue name")
    ),
    responses(
        (status = 200, description = "Queue info"),
        (status = 404, description = "Queue not found")
    )
)]
/// GET /queues/{name}
///
/// # Errors
#[instrument(name = "get_queue_handler", skip_all)]
pub async fn get_queue_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let queue = state
        .broker
        .queue_info(name.clone())
        .await
        .map_err(|_| ApiError::not_found(format!("queue {name} not found")))?;

    Ok(axum::Json(queue).into_response())
}

#[utoipa::path(
    delete,
    path = "/queues/{name}",
    params(
        ("name" = String, Path, description = "Queue name")
    ),
    responses(
        (status = 200, description = "Queue deleted")
    )
)]
/// DELETE /queues/{name}
///
/// # Errors
#[instrument(name = "delete_queue_handler", skip_all)]
pub async fn delete_queue_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    state
        .broker
        .delete_queue(name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::OK.into_response())
}
