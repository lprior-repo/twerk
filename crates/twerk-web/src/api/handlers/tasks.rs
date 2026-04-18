//! Task handlers - API endpoints for task operations.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use twerk_core::id::TaskId;

use super::super::error::ApiError;
use super::super::redact::redact_task_log_parts;
use super::{parse_page, parse_size, AppState};
use crate::middleware::hooks::on_read_task;
use tracing::instrument;
use utoipa::ToSchema;

/// Raw pagination query parameters — uses String types to avoid
/// serde rejection errors when users send invalid input like `?page=abc`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct RawPaginationQuery {
    pub page: Option<String>,
    pub size: Option<String>,
    pub q: Option<String>,
}

/// Typed pagination query parsed from raw strings.
/// Invalid integer values silently become `None` rather than
/// leaking raw serde/axum error messages to the user.
#[derive(Debug, Clone, ToSchema)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub q: Option<String>,
}

impl PaginationQuery {
    /// Parse from raw query strings, converting parse failures to `None` gracefully.
    #[must_use]
    pub fn from_raw(raw: RawPaginationQuery) -> Self {
        Self {
            page: raw.page.and_then(|s| s.parse().ok()),
            size: raw.size.and_then(|s| s.parse().ok()),
            q: raw.q,
        }
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
        (status = 200, description = "Task details"),
        (status = 404, description = "Task not found")
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
        (status = 200, description = "Paginated task log entries"),
        (status = 404, description = "Task not found")
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
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 25, 100);
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
