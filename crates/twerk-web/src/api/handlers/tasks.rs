//! Task handlers - API endpoints for task operations.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use twerk_core::id::TaskId;
use twerk_core::task::{Task, TaskLogPart};
use twerk_infrastructure::datastore::Page;

use super::super::error::ApiError;
use super::super::openapi_types::MessageResponse;
use super::super::redact::redact_task_log_parts;
use super::{parse_page, parse_size, AppState};
use crate::middleware::hooks::on_read_task;
use tracing::instrument;

/// Raw pagination query parameters — uses String types to avoid
/// axum extraction rejection errors when users send invalid input like `?page=abc`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawPaginationQuery {
    pub page: Option<String>,
    pub size: Option<String>,
    pub q: Option<String>,
}

/// Pagination query preserved as raw strings so handlers can return stable 400 responses.
#[derive(Debug, Clone)]
pub struct PaginationQuery {
    page: Option<String>,
    size: Option<String>,
    pub q: Option<String>,
}

impl PaginationQuery {
    /// Preserve raw values so handlers can validate each field explicitly.
    #[must_use]
    pub fn from_raw(raw: RawPaginationQuery) -> Self {
        Self {
            page: raw.page,
            size: raw.size,
            q: raw.q,
        }
    }

    /// # Errors
    /// Returns an error when `page` is not a positive integer.
    pub fn page(&self) -> Result<i64, ApiError> {
        parse_page(self.page.as_deref())
    }

    /// # Errors
    /// Returns an error when `size` is not a positive integer within the allowed range.
    pub fn size(&self, default: i64, max: i64) -> Result<i64, ApiError> {
        parse_size(self.size.as_deref(), default, max)
    }
}

/// GET /tasks/{id}
#[utoipa::path(
    get,
    path = "/tasks/{id}",
    params(
        ("id" = String, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task details", body = Task, content_type = "application/json"),
        (status = 404, description = "Task not found", body = MessageResponse, content_type = "application/json")
    )
)]
/// # Errors
#[instrument(name = "get_task_handler", skip_all)]
pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(id): Path<TaskId>,
) -> Result<Response, ApiError> {
    let mut task = state.ds.get_task_by_id(&id).await.map_err(ApiError::from)?;

    if let Some(ref job_id) = task.job_id {
        if let Ok(job) = state.ds.get_job_by_id(job_id).await {
            let secrets = job.secrets.unwrap_or_default();
            on_read_task(&mut task, &secrets);
        }
    }

    Ok(axum::Json(task).into_response())
}

/// GET /tasks/{id}/log
#[utoipa::path(
    get,
    path = "/tasks/{id}/log",
    params(
        ("id" = String, Path, description = "Task ID"),
        ("page" = Option<String>, Query, description = "Page number"),
        ("size" = Option<String>, Query, description = "Page size (max 100)"),
        ("q" = Option<String>, Query, description = "Search query")
    ),
    responses(
        (status = 200, description = "Paginated task log entries", body = Page<TaskLogPart>, content_type = "application/json"),
        (status = 404, description = "Task not found", body = MessageResponse, content_type = "application/json")
    )
)]
/// # Errors
#[instrument(name = "get_task_log_handler", skip_all)]
pub async fn get_task_log_handler(
    State(state): State<AppState>,
    Path(id): Path<TaskId>,
    Query(raw): Query<RawPaginationQuery>,
) -> Result<Response, ApiError> {
    let qp = PaginationQuery::from_raw(raw);
    let page = qp.page()?;
    let size = qp.size(25, 100)?;
    let q = qp.q.unwrap_or_default();

    let task = state.ds.get_task_by_id(&id).await.map_err(ApiError::from)?;

    let mut parts = state
        .ds
        .get_task_log_parts(&id, &q, page, size)
        .await
        .map_err(ApiError::from)?;

    if let Some(ref job_id) = task.job_id {
        if let Ok(job) = state.ds.get_job_by_id(job_id).await {
            let secrets = job.secrets.unwrap_or_default();
            redact_task_log_parts(&mut parts.items, &secrets);
        }
    }

    Ok(axum::Json(parts).into_response())
}
