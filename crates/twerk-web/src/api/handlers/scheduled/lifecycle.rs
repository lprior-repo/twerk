use axum::extract::{Path, State};
use axum::response::Response;
use tracing::instrument;
use twerk_core::id::ScheduledJobId;

use super::shared::{
    build_paused_for_event, pause_state_transition, resume_state_transition,
    scheduled_job_event_value, status_ok_response, validate_pause, validate_resume, was_active,
};
use crate::api::error::ApiError;
use crate::api::handlers::AppState;
use crate::api::openapi_types::{MessageResponse, StatusResponse};

#[utoipa::path(
    put,
    path = "/scheduled-jobs/{id}/pause",
    params(("id" = ScheduledJobId, Path, description = "Scheduled job ID")),
    responses(
        (status = 200, description = "Scheduled job paused", body = StatusResponse, content_type = "application/json"),
        (status = 400, description = "Scheduled job cannot be paused", body = MessageResponse, content_type = "application/json"),
        (status = 404, description = "Scheduled job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[allow(clippy::missing_errors_doc)]
#[instrument(name = "pause_scheduled_job_handler", skip_all)]
pub async fn pause_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let scheduled_job = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    validate_pause(&scheduled_job)?;
    state
        .ds
        .update_scheduled_job(&id, pause_state_transition())
        .await
        .map_err(ApiError::from)?;

    let paused = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    state
        .broker
        .publish_event(
            "scheduled.job".to_string(),
            scheduled_job_event_value(&paused)?,
        )
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(status_ok_response())
}

#[utoipa::path(
    put,
    path = "/scheduled-jobs/{id}/resume",
    params(("id" = ScheduledJobId, Path, description = "Scheduled job ID")),
    responses(
        (status = 200, description = "Scheduled job resumed", body = StatusResponse, content_type = "application/json"),
        (status = 400, description = "Scheduled job cannot be resumed", body = MessageResponse, content_type = "application/json"),
        (status = 404, description = "Scheduled job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[allow(clippy::missing_errors_doc)]
#[instrument(name = "resume_scheduled_job_handler", skip_all)]
pub async fn resume_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let scheduled_job = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    validate_resume(&scheduled_job)?;
    state
        .ds
        .update_scheduled_job(&id, resume_state_transition())
        .await
        .map_err(ApiError::from)?;

    let resumed = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    state
        .broker
        .publish_event(
            "scheduled.job".to_string(),
            scheduled_job_event_value(&resumed)?,
        )
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(status_ok_response())
}

#[utoipa::path(
    delete,
    path = "/scheduled-jobs/{id}",
    params(("id" = ScheduledJobId, Path, description = "Scheduled job ID")),
    responses(
        (status = 200, description = "Scheduled job deleted", body = StatusResponse, content_type = "application/json"),
        (status = 404, description = "Scheduled job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[allow(clippy::missing_errors_doc)]
#[instrument(name = "delete_scheduled_job_handler", skip_all)]
pub async fn delete_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let scheduled_job = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;
    let is_active = was_active(&scheduled_job);

    state
        .ds
        .delete_scheduled_job(&id)
        .await
        .map_err(ApiError::from)?;

    if is_active {
        state
            .broker
            .publish_event(
                "scheduled.job".to_string(),
                scheduled_job_event_value(&build_paused_for_event(&scheduled_job))?,
            )
            .await
            .map_err(|error| ApiError::internal(error.to_string()))?;
    }

    Ok(status_ok_response())
}
