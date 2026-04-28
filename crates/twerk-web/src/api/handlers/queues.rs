//! Queue handlers - API endpoints for queue operations.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::super::error::ApiError;
use super::super::openapi_types::MessageResponse;
use super::AppState;
use tracing::instrument;
use twerk_infrastructure::broker::QueueInfo;

#[utoipa::path(
    get,
    path = "/queues",
    tag = "Queues",
    responses(
        (status = 200, description = "List of queues", body = Vec<QueueInfo>, content_type = "application/json")
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
    tag = "Queues",
    params(
        ("name" = String, Path, description = "Queue name")
    ),
    responses(
        (status = 200, description = "Queue info", body = QueueInfo, content_type = "application/json"),
        (status = 404, description = "Queue not found", body = MessageResponse, content_type = "application/json")
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
    tag = "Queues",
    params(
        ("name" = String, Path, description = "Queue name")
    ),
    responses(
        (status = 200, description = "Queue deleted"),
        (status = 404, description = "Queue not found", body = MessageResponse, content_type = "application/json")
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
        .delete_queue(name.clone())
        .await
        .map_err(|_| ApiError::not_found(format!("queue {name} not found")))?;
    Ok(StatusCode::OK.into_response())
}
