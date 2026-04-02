//! Scheduled job handlers - API endpoints for scheduled job operations.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use twerk_core::job::{
    new_scheduled_job_summary, ScheduledJob, SCHEDULED_JOB_STATE_ACTIVE, SCHEDULED_JOB_STATE_PAUSED,
};
use twerk_core::validation::{validate_cron, validate_job};

use super::super::error::ApiError;
use super::tasks::PaginationQuery;
use super::{default_user, extract_current_user, parse_page, parse_size, AppState};

#[derive(Debug, Deserialize)]
pub struct CreateScheduledJobBody {
    pub name: Option<String>,
    pub description: Option<String>,
    pub cron: Option<String>,
    pub tags: Option<Vec<String>>,
    pub tasks: Option<Vec<twerk_core::task::Task>>,
    pub inputs: Option<std::collections::HashMap<String, String>>,
    pub secrets: Option<std::collections::HashMap<String, String>>,
    pub output: Option<String>,
    pub defaults: Option<twerk_core::job::JobDefaults>,
    pub webhooks: Option<Vec<twerk_core::webhook::Webhook>>,
    pub permissions: Option<Vec<twerk_core::task::Permission>>,
    pub auto_delete: Option<twerk_core::task::AutoDelete>,
}

/// POST /scheduled-jobs
///
/// # Errors
pub async fn create_scheduled_job_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or("", |v| v);

    let sj_input: CreateScheduledJobBody = match content_type {
        "application/json" => {
            serde_json::from_slice(&body).map_err(|e| ApiError::bad_request(e.to_string()))?
        }
        "text/yaml" | "application/x-yaml" => super::super::yaml::from_slice(&body)?,
        _ => return Err(ApiError::bad_request("unsupported content type")),
    };

    let cron = sj_input
        .cron
        .ok_or_else(|| ApiError::bad_request("cron is required"))?;
    let tasks = sj_input
        .tasks
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("tasks is required"))?;

    if let Err(e) = validate_cron(&cron) {
        return Err(ApiError::bad_request(e));
    }
    if let Err(errors) = validate_job(
        sj_input.name.as_ref(),
        sj_input.tasks.as_ref(),
        sj_input.defaults.as_ref(),
        sj_input.output.as_ref(),
    ) {
        return Err(ApiError::bad_request(errors.join("; ")));
    }

    let sj = ScheduledJob {
        id: Some(twerk_core::id::ScheduledJobId::new(
            twerk_core::uuid::new_short_uuid(),
        )),
        name: sj_input.name,
        description: sj_input.description,
        cron: Some(cron.clone()),
        state: SCHEDULED_JOB_STATE_ACTIVE.to_string(),
        inputs: sj_input.inputs,
        tasks: Some(tasks.clone()),
        created_by: default_user(&state).await,
        defaults: sj_input.defaults,
        auto_delete: sj_input.auto_delete,
        webhooks: sj_input.webhooks,
        permissions: sj_input.permissions,
        created_at: Some(time::OffsetDateTime::now_utc()),
        tags: sj_input.tags,
        secrets: sj_input.secrets,
        output: sj_input.output,
    };

    state
        .ds
        .create_scheduled_job(&sj)
        .await
        .map_err(ApiError::from)?;

    let summary = new_scheduled_job_summary(&sj);
    Ok((StatusCode::OK, axum::Json(summary)).into_response())
}

/// GET /scheduled-jobs
///
/// # Errors
pub async fn list_scheduled_jobs_handler(
    State(state): State<AppState>,
    Query(qp): Query<PaginationQuery>,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 10, 20);
    let current_user = extract_current_user(&req);

    let result = state
        .ds
        .get_scheduled_jobs(&current_user, page, size)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(result).into_response())
}

/// GET /scheduled-jobs/{id}
///
/// # Errors
pub async fn get_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(sj).into_response())
}

/// PUT /scheduled-jobs/{id}/pause
///
/// # Errors
pub async fn pause_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    if sj.state != SCHEDULED_JOB_STATE_ACTIVE {
        return Err(ApiError::bad_request("scheduled job is not active"));
    }

    state
        .ds
        .update_scheduled_job(
            &id,
            Box::new(|mut sj| {
                sj.state = SCHEDULED_JOB_STATE_PAUSED.to_string();
                Ok(sj)
            }),
        )
        .await
        .map_err(ApiError::from)?;

    state
        .broker
        .publish_event(
            "scheduled.job".to_string(),
            serde_json::to_value(()).map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// PUT /scheduled-jobs/{id}/resume
///
/// # Errors
pub async fn resume_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    if sj.state != SCHEDULED_JOB_STATE_PAUSED {
        return Err(ApiError::bad_request("scheduled job is not paused"));
    }

    state
        .ds
        .update_scheduled_job(
            &id,
            Box::new(|mut sj| {
                sj.state = SCHEDULED_JOB_STATE_ACTIVE.to_string();
                Ok(sj)
            }),
        )
        .await
        .map_err(ApiError::from)?;

    state
        .broker
        .publish_event(
            "scheduled.job".to_string(),
            serde_json::to_value(()).map_err(|e| ApiError::internal(e.to_string()))?,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// DELETE /scheduled-jobs/{id}
///
/// # Errors
pub async fn delete_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    state
        .ds
        .delete_scheduled_job(&id)
        .await
        .map_err(ApiError::from)?;

    if sj.state == SCHEDULED_JOB_STATE_ACTIVE {
        let mut paused_sj = sj.clone();
        paused_sj.state = SCHEDULED_JOB_STATE_PAUSED.to_string();
        state
            .broker
            .publish_event(
                "scheduled.job".to_string(),
                serde_json::to_value(&paused_sj).map_err(|e| ApiError::internal(e.to_string()))?,
            )
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}
