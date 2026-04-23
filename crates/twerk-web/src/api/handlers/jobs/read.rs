use axum::extract::{Path as AxumPath, Query, State};
use axum::response::{IntoResponse, Response};
use tracing::instrument;
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobSummary};
use twerk_core::task::TaskLogPart;
use twerk_infrastructure::datastore::Page;

use crate::api::error::ApiError;
use crate::api::handlers::tasks::{PaginationQuery, RawPaginationQuery};
use crate::api::handlers::{extract_current_user, AppState};
use crate::api::openapi_types::MessageResponse;
use crate::api::redact::redact_task_log_parts;
use crate::middleware::hooks::{on_read_job, on_read_job_summary};

/// GET /jobs/{id}
#[utoipa::path(
    get,
    path = "/jobs/{id}",
    params(
        ("id" = JobId, description = "The job ID as either an RFC 4122 UUID or a 22-character base57 short ID")
    ),
    responses(
        (status = 200, description = "Job found", body = Job, content_type = "application/json"),
        (status = 404, description = "Job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "get_job_handler", skip_all)]
/// # Errors
/// Returns an error when the job cannot be loaded or its tasks cannot be fetched from the
/// datastore.
pub async fn get_job_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
) -> Result<Response, ApiError> {
    let mut job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    if let Some(job_id) = &job.id {
        let actual_tasks = state
            .ds
            .get_all_tasks_for_job(job_id.as_ref())
            .await
            .map_err(ApiError::from)?;
        if !actual_tasks.is_empty() {
            job.tasks = Some(actual_tasks);
        }
    }

    let secrets = job
        .secrets
        .as_ref()
        .map_or_else(std::collections::HashMap::new, |v| v.clone());
    on_read_job(&mut job, &secrets);

    Ok(axum::Json(job).into_response())
}

/// GET /jobs
#[utoipa::path(
    get,
    path = "/jobs",
    params(
        ("page" = Option<String>, Query, description = "Page number"),
        ("size" = Option<String>, Query, description = "Page size"),
        ("q" = Option<String>, Query, description = "Search query")
    ),
    responses(
        (status = 200, description = "Paginated jobs", body = Page<JobSummary>, content_type = "application/json")
    )
)]
#[instrument(name = "list_jobs_handler", skip_all)]
/// # Errors
/// Returns an error when the requested page of jobs cannot be loaded from the datastore.
pub async fn list_jobs_handler(
    State(state): State<AppState>,
    Query(raw): Query<RawPaginationQuery>,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    let query = PaginationQuery::from_raw(raw);
    let page = query.page()?;
    let size = query.size(10, 20)?;
    let search = query.q.as_ref().map_or_else(String::new, |v| v.clone());
    let current_user = extract_current_user(&req);

    let mut result = state
        .ds
        .get_jobs(&current_user, &search, page, size)
        .await
        .map_err(ApiError::from)?;

    for item in &mut result.items {
        on_read_job_summary(item);
    }
    Ok(axum::Json(result).into_response())
}

/// GET /jobs/{id}/log
#[utoipa::path(
    get,
    path = "/jobs/{id}/log",
    params(
        ("id" = JobId, description = "The job ID"),
        ("page" = Option<String>, Query, description = "Page number"),
        ("size" = Option<String>, Query, description = "Page size")
    ),
    responses(
        (status = 200, description = "Paginated job log parts", body = Page<TaskLogPart>, content_type = "application/json"),
        (status = 404, description = "Job not found", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "get_job_log_handler", skip_all)]
/// # Errors
/// Returns an error when the job cannot be loaded or the requested log page cannot be fetched.
pub async fn get_job_log_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
    Query(raw): Query<RawPaginationQuery>,
) -> Result<Response, ApiError> {
    let query = PaginationQuery::from_raw(raw);
    let page = query.page()?;
    let size = query.size(25, 100)?;
    let job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    let mut parts = state
        .ds
        .get_job_log_parts(&id, "", page, size)
        .await
        .map_err(ApiError::from)?;

    let secrets = job
        .secrets
        .as_ref()
        .map_or_else(std::collections::HashMap::new, |v| v.clone());
    redact_task_log_parts(&mut parts.items, &secrets);

    Ok(axum::Json(parts).into_response())
}
