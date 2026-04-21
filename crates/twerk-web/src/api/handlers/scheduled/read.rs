use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use tracing::instrument;
use twerk_core::id::ScheduledJobId;
use twerk_core::job::{ScheduledJob, ScheduledJobSummary};
use twerk_infrastructure::datastore::Page;

use crate::api::error::ApiError;
use crate::api::handlers::tasks::{PaginationQuery, RawPaginationQuery};
use crate::api::handlers::{extract_current_user, parse_page, parse_size, AppState};
use crate::api::openapi_types::MessageResponse;
use crate::api::redact::{redact_scheduled_job, redact_scheduled_job_summary};

/// GET /scheduled-jobs
#[utoipa::path(
    get,
    path = "/scheduled-jobs",
    responses(
        (status = 200, description = "List of scheduled jobs", body = Page<ScheduledJobSummary>, content_type = "application/json")
    )
)]
#[instrument(name = "list_scheduled_jobs_handler", skip_all)]
/// # Errors
/// Returns an error when the requested page of scheduled jobs cannot be loaded.
pub async fn list_scheduled_jobs_handler(
    State(state): State<AppState>,
    Query(raw): Query<RawPaginationQuery>,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    let query = PaginationQuery::from_raw(raw);
    let page = parse_page(query.page);
    let size = parse_size(query.size, 10, 20);
    let current_user = extract_current_user(&req);

    let mut result = state
        .ds
        .get_scheduled_jobs(&current_user, page, size)
        .await
        .map_err(ApiError::from)?;

    result
        .items
        .iter_mut()
        .for_each(redact_scheduled_job_summary);
    Ok(axum::Json(result).into_response())
}

#[utoipa::path(
    get,
    path = "/scheduled-jobs/{id}",
    params(("id" = ScheduledJobId, Path, description = "Scheduled job ID")),
    responses(
        (status = 200, description = "Scheduled job found", body = ScheduledJob, content_type = "application/json"),
        (status = 404, description = "Scheduled job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[allow(clippy::missing_errors_doc)]
#[instrument(name = "get_scheduled_job_handler", skip_all)]
pub async fn get_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let mut scheduled_job = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    redact_scheduled_job(&mut scheduled_job);
    Ok(axum::Json(scheduled_job).into_response())
}
