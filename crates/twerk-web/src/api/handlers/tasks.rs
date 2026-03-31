//! Task handlers - API endpoints for task operations.

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use super::super::error::ApiError;
use super::super::redact::redact_task_log_parts;
use super::{parse_page, parse_size, AppState};
use crate::middleware::hooks::on_read_task;

/// Pagination query parameters for list endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub q: Option<String>,
}

/// GET /tasks/{id}
pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
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
pub async fn get_task_log_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
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
