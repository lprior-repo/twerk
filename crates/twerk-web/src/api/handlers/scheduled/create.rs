use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tracing::instrument;
use twerk_core::job::ScheduledJobSummary;

use super::shared::{
    build_scheduled_job, parse_create_body, scheduled_job_event_value, validate_create_input,
    CreateScheduledJobBody,
};
use crate::api::error::ApiError;
use crate::api::handlers::{default_user, AppState};
use crate::api::openapi_types::MessageResponse;
use crate::api::redact::redact_scheduled_job;
use twerk_core::job::new_scheduled_job_summary;

/// POST /scheduled-jobs
#[utoipa::path(
    post,
    path = "/scheduled-jobs",
    request_body(
        description = "Scheduled job definition as JSON or YAML",
        content(
            (CreateScheduledJobBody = "application/json"),
            (CreateScheduledJobBody = "application/yaml"),
            (CreateScheduledJobBody = "application/x-yaml"),
            (CreateScheduledJobBody = "text/yaml")
        )
    ),
    responses(
        (status = 200, description = "Scheduled job created", body = ScheduledJobSummary, content_type = "application/json"),
        (status = 400, description = "Invalid scheduled job payload", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "create_scheduled_job_handler", skip_all)]
pub async fn create_scheduled_job_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let input = parse_create_body(&headers, &body)?;
    let (cron, tasks) = validate_create_input(&input)?;
    let scheduled_job = build_scheduled_job(input, cron, tasks, default_user(&state).await)?;

    state
        .ds
        .create_scheduled_job(&scheduled_job)
        .await
        .map_err(ApiError::from)?;

    let scheduled_job_id = scheduled_job
        .id
        .clone()
        .ok_or_else(|| ApiError::internal("scheduled job id missing"))?;
    let mut created = state
        .ds
        .get_scheduled_job_by_id(&scheduled_job_id)
        .await
        .map_err(ApiError::from)?;

    redact_scheduled_job(&mut created);
    state
        .broker
        .publish_event(
            "scheduled.job".to_string(),
            scheduled_job_event_value(&created)?,
        )
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok((
        StatusCode::OK,
        axum::Json(new_scheduled_job_summary(&created)),
    )
        .into_response())
}
