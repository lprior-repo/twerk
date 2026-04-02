//! Job handlers - API endpoints for job operations.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use twerk_core::job::{
    new_job_summary, Job, JOB_STATE_CANCELLED, JOB_STATE_COMPLETED, JOB_STATE_FAILED,
    JOB_STATE_RESTART, JOB_STATE_RUNNING, JOB_STATE_SCHEDULED,
};

use super::super::error::ApiError;
use super::super::redact::redact_task_log_parts;
use super::tasks::PaginationQuery;
use super::{default_user, extract_current_user, parse_page, parse_size, AppState};
use crate::middleware::hooks::{on_read_job, on_read_job_summary};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreateJobQuery {
    pub wait: Option<bool>,
}

/// POST /jobs
///
/// # Errors
pub async fn create_job_handler(
    State(state): State<AppState>,
    Query(cq): Query<CreateJobQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or("", |v| v);

    let mut job: Job = match content_type {
        "application/json" => {
            serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?
        }
        "text/yaml" | "application/x-yaml" => {
            super::super::yaml::from_slice(&body)?
        }
        _ => return Err(ApiError::bad_request("unsupported content type")),
    };

    if job.id.is_none() {
        job.id = Some(twerk_core::uuid::new_short_uuid().into());
    }

    if job.created_at.is_none() {
        job.created_at = Some(time::OffsetDateTime::now_utc());
    }

    if job.state.is_empty() {
        job.state = twerk_core::job::JOB_STATE_PENDING.to_string();
    }

    if job.created_by.is_none() {
        job.created_by = default_user(&state).await;
    }

    if let Err(errors) = twerk_core::validation::validate_job(
        job.name.as_ref(),
        job.tasks.as_ref(),
        job.defaults.as_ref(),
        job.output.as_ref(),
    ) {
        return Err(ApiError::bad_request(errors.join("; ")));
    }

    if cq.wait.unwrap_or(false) {
        wait_for_job_completion(state, job).await
    } else {
        create_job_no_wait(state, job).await
    }
}

async fn wait_for_job_completion(state: AppState, job: Job) -> Result<Response, ApiError> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let job_id = job
        .id
        .clone()
        .ok_or_else(|| ApiError::internal("job id missing"))?;

    let pattern = "job.*".to_string();
    state
        .broker
        .subscribe_for_events(
            pattern,
            Arc::new(move |val| {
                let tx = tx.clone();
                let job_id = job_id.clone();
                Box::pin(async move {
                    if let Ok(ev_job) = serde_json::from_value::<Job>(val) {
                        if ev_job.id.as_ref() == Some(&job_id)
                            && (ev_job.state == JOB_STATE_COMPLETED
                                || ev_job.state == JOB_STATE_FAILED
                                || ev_job.state == JOB_STATE_CANCELLED)
                        {
                            let _ = tx.send(ev_job).await;
                        }
                    }
                    Ok(())
                })
            }),
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    state.ds.create_job(&job).await.map_err(ApiError::from)?;
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    match tokio::time::timeout(tokio::time::Duration::from_secs(3600), rx.recv()).await {
        Ok(Some(mut finished_job)) => {
            let secrets = finished_job.secrets.clone().unwrap_or_default();
            on_read_job(&mut finished_job, &secrets);
            Ok((StatusCode::OK, axum::Json(new_job_summary(&finished_job))).into_response())
        }
        Ok(None) => Err(ApiError::internal("subscription channel closed")),
        Err(_) => Err(ApiError::internal("timeout waiting for job")),
    }
}

async fn create_job_no_wait(state: AppState, job: Job) -> Result<Response, ApiError> {
    state.ds.create_job(&job).await.map_err(ApiError::from)?;
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut summary = new_job_summary(&job);
    on_read_job_summary(&mut summary);

    Ok((StatusCode::OK, axum::Json(summary)).into_response())
}

/// GET /jobs/{id}
///
/// # Errors
pub async fn get_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    let secrets = job.secrets.clone().unwrap_or_default();
    on_read_job(&mut job, &secrets);

    Ok(axum::Json(job).into_response())
}

/// GET /jobs
///
/// # Errors
pub async fn list_jobs_handler(
    State(state): State<AppState>,
    Query(qp): Query<PaginationQuery>,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 10, 20);
    let q = qp.q.unwrap_or_default();
    let current_user = extract_current_user(&req);

    let mut result = state
        .ds
        .get_jobs(&current_user, &q, page, size)
        .await
        .map_err(ApiError::from)?;

    for item in &mut result.items {
        on_read_job_summary(item);
    }

    Ok(axum::Json(result).into_response())
}

/// PUT /jobs/{id}/cancel
///
/// # Errors
pub async fn cancel_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    if job.state != JOB_STATE_RUNNING && job.state != JOB_STATE_SCHEDULED {
        return Err(ApiError::bad_request("job is not running"));
    }

    job.state = JOB_STATE_CANCELLED.to_string();
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// PUT /jobs/{id}/restart
///
/// # Errors
pub async fn restart_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    if job.state != JOB_STATE_FAILED && job.state != JOB_STATE_CANCELLED {
        return Err(ApiError::bad_request("job cannot be restarted"));
    }

    job.state = JOB_STATE_RESTART.to_string();
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// GET /jobs/{id}/log
///
/// # Errors
pub async fn get_job_log_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 25, 100);

    let job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    let mut parts = state
        .ds
        .get_job_log_parts(&id, "", page, size)
        .await
        .map_err(ApiError::from)?;

    let secrets = job.secrets.unwrap_or_default();
    redact_task_log_parts(&mut parts.items, &secrets);

    Ok(axum::Json(parts).into_response())
}
