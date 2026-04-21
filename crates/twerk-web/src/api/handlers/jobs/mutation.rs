use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tracing::instrument;
use twerk_core::id::JobId;
use twerk_core::job::JobState;

use crate::api::error::ApiError;
use crate::api::handlers::AppState;
use crate::api::openapi_types::{MessageResponse, StatusResponse};

fn ok_status_response() -> Response {
    (StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response()
}

async fn cancel_job_impl(state: AppState, id: JobId) -> Result<Response, ApiError> {
    let job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;
    if matches!(
        job.state,
        JobState::Completed | JobState::Failed | JobState::Cancelled
    ) {
        return Err(ApiError::bad_request(
            "job cannot be cancelled in its current state",
        ));
    }

    state
        .ds
        .update_job(
            id.as_ref(),
            Box::new(|mut current| {
                current.state = JobState::Cancelled;
                Ok(current)
            }),
        )
        .await
        .map_err(ApiError::from)?;

    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(ok_status_response())
}

/// PUT /jobs/{id}/cancel
#[utoipa::path(
    put,
    path = "/jobs/{id}/cancel",
    params(("id" = JobId, description = "The job ID")),
    responses(
        (status = 200, description = "Job cancelled", body = StatusResponse, content_type = "application/json"),
        (status = 400, description = "Job cannot be cancelled in its current state", body = MessageResponse, content_type = "application/json"),
        (status = 404, description = "Job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "cancel_job_handler", skip_all)]
pub async fn cancel_job_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
) -> Result<Response, ApiError> {
    cancel_job_impl(state, id).await
}

/// POST /jobs/{id}/cancel
#[utoipa::path(
    post,
    path = "/jobs/{id}/cancel",
    params(("id" = JobId, description = "The job ID")),
    responses(
        (status = 200, description = "Job cancelled", body = StatusResponse, content_type = "application/json"),
        (status = 400, description = "Job cannot be cancelled in its current state", body = MessageResponse, content_type = "application/json"),
        (status = 404, description = "Job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "cancel_job_handler_post", skip_all)]
pub async fn cancel_job_handler_post(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
) -> Result<Response, ApiError> {
    cancel_job_impl(state, id).await
}

/// PUT /jobs/{id}/restart
#[utoipa::path(
    put,
    path = "/jobs/{id}/restart",
    params(("id" = JobId, description = "The job ID")),
    responses(
        (status = 200, description = "Job restarted", body = StatusResponse, content_type = "application/json"),
        (status = 400, description = "Job cannot be restarted", body = MessageResponse, content_type = "application/json"),
        (status = 404, description = "Job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "restart_job_handler", skip_all)]
pub async fn restart_job_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
) -> Result<Response, ApiError> {
    let job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;
    if !matches!(job.state, JobState::Failed | JobState::Cancelled) {
        return Err(ApiError::bad_request("job cannot be restarted"));
    }

    state
        .ds
        .update_job(
            id.as_ref(),
            Box::new(|mut current| {
                current.state = JobState::Restart;
                Ok(current)
            }),
        )
        .await
        .map_err(ApiError::from)?;

    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(ok_status_response())
}
