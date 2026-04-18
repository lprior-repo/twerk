//! Scheduled job handlers - API endpoints for scheduled job operations.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use twerk_core::id::ScheduledJobId;
use twerk_core::job::{new_scheduled_job_summary, ScheduledJob, ScheduledJobState};
use twerk_core::repository;
use twerk_core::user::User;
use twerk_core::validation::{validate_cron, validate_job};

use super::super::error::ApiError;
use super::tasks::{PaginationQuery, RawPaginationQuery};
use super::{default_user, extract_current_user, parse_page, parse_size, AppState};
use tracing::instrument;

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

// ============================================================================================
// Pure extraction and validation functions
// ============================================================================================

/// Extract content type from headers (pure).
fn extract_content_type(headers: &HeaderMap) -> &str {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map_or("", |v| v)
}

/// Parse create body based on content type (partial - I/O in yaml parsing but contained).
fn parse_create_body(content_type: &str, body: &[u8]) -> Result<CreateScheduledJobBody, ApiError> {
    match content_type {
        "application/json" => {
            serde_json::from_slice(body).map_err(|e| ApiError::bad_request(e.to_string()))
        }
        "text/yaml" | "application/x-yaml" | "application/yaml" => super::super::yaml::from_slice(body),
        _ => Err(ApiError::bad_request("unsupported content type")),
    }
}

/// Validate create input and extract required fields (pure).
fn validate_create_input(
    body: &CreateScheduledJobBody,
) -> Result<(String, Vec<twerk_core::task::Task>), ApiError> {
    let cron = body
        .cron
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("cron is required"))?
        .clone();
    let tasks = body
        .tasks
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("tasks is required"))?
        .clone();

    validate_cron(&cron).map_err(ApiError::bad_request)?;

    let job_err = validate_job(
        body.name.as_ref(),
        body.tasks.as_ref(),
        body.defaults.as_ref(),
        body.output.as_ref(),
    );
    if let Err(errors) = job_err {
        return Err(ApiError::bad_request(errors.join("; ")));
    }

    Ok((cron, tasks))
}

/// Build `ScheduledJob` from validated input (pure).
fn build_scheduled_job(
    body: CreateScheduledJobBody,
    cron: String,
    tasks: Vec<twerk_core::task::Task>,
    created_by: Option<User>,
) -> Result<ScheduledJob, ApiError> {
    let id = twerk_core::id::ScheduledJobId::new(twerk_core::uuid::new_short_uuid())?;
    Ok(ScheduledJob {
        id: Some(id),
        name: body.name,
        description: body.description,
        cron: Some(cron),
        state: ScheduledJobState::Active,
        inputs: body.inputs,
        tasks: Some(tasks),
        created_by,
        defaults: body.defaults,
        auto_delete: body.auto_delete,
        webhooks: body.webhooks,
        permissions: body.permissions,
        created_at: Some(time::OffsetDateTime::now_utc()),
        tags: body.tags,
        secrets: body.secrets,
        output: body.output,
    })
}

/// Validate scheduled job can be paused (pure).
fn validate_pause(sj: &ScheduledJob) -> Result<(), ApiError> {
    if sj.state != ScheduledJobState::Active {
        return Err(ApiError::bad_request("scheduled job is not active"));
    }
    Ok(())
}

/// Validate scheduled job can be resumed (pure).
fn validate_resume(sj: &ScheduledJob) -> Result<(), ApiError> {
    if sj.state != ScheduledJobState::Paused {
        return Err(ApiError::bad_request("scheduled job is not paused"));
    }
    Ok(())
}

/// Build pause state transition closure (pure factory).
fn pause_state_transition(
) -> Box<dyn FnOnce(ScheduledJob) -> Result<ScheduledJob, repository::Error> + Send> {
    Box::new(|mut sj| {
        sj.state = ScheduledJobState::Paused;
        Ok(sj)
    })
}

/// Build resume state transition closure (pure factory).
fn resume_state_transition(
) -> Box<dyn FnOnce(ScheduledJob) -> Result<ScheduledJob, repository::Error> + Send> {
    Box::new(|mut sj| {
        sj.state = ScheduledJobState::Active;
        Ok(sj)
    })
}

/// Build status OK response (pure).
fn status_ok_response() -> Response {
    (StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response()
}

/// Check if scheduled job was active (pure).
fn was_active(sj: &ScheduledJob) -> bool {
    sj.state == ScheduledJobState::Active
}

/// Build paused scheduled job for event (pure).
fn build_paused_for_event(sj: &ScheduledJob) -> ScheduledJob {
    let mut paused = sj.clone();
    paused.state = ScheduledJobState::Paused;
    paused
}

/// Build event value from scheduled job (pure).
fn build_scheduled_job_event_value_from_sj(
    sj: &ScheduledJob,
) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(sj).map_err(|e| ApiError::internal(e.to_string()))
}

// ============================================================================================
// Handlers (thin orchestration)
// ============================================================================================

/// POST /scheduled-jobs
///
/// # Errors
#[utoipa::path(
    post,
    path = "/scheduled-jobs",
    responses(
        (status = 200, description = "Scheduled job created")
    )
)]
#[instrument(name = "create_scheduled_job_handler", skip_all)]
pub async fn create_scheduled_job_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let content_type = extract_content_type(&headers);
    let sj_input = parse_create_body(content_type, &body)?;
    let (cron, tasks) = validate_create_input(&sj_input)?;
    let user = default_user(&state).await;
    let sj = build_scheduled_job(sj_input, cron, tasks, user)?;

    state
        .ds
        .create_scheduled_job(&sj)
        .await
        .map_err(ApiError::from)?;

    state
        .broker
        .publish_event(
            "scheduled.job".to_string(),
            build_scheduled_job_event_value_from_sj(&sj)?,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let summary = new_scheduled_job_summary(&sj);
    Ok((StatusCode::OK, axum::Json(summary)).into_response())
}

/// GET /scheduled-jobs
///
/// # Errors
#[utoipa::path(
    get,
    path = "/scheduled-jobs",
    responses(
        (status = 200, description = "List of scheduled jobs")
    )
)]
#[instrument(name = "list_scheduled_jobs_handler", skip_all)]
pub async fn list_scheduled_jobs_handler(
    State(state): State<AppState>,
    Query(raw): Query<RawPaginationQuery>,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    let qp = PaginationQuery::from_raw(raw);
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

#[utoipa::path(
    get,
    path = "/scheduled-jobs/{id}",
    params(
        ("id" = String, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job details"),
        (status = 404, description = "Scheduled job not found")
    )
)]
/// GET /scheduled-jobs/{id}
///
/// # Errors
#[utoipa::path(
    get,
    path = "/scheduled-jobs/{id}",
    params(
        ("id" = twerk_core::id::ScheduledJobId, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job found")
    )
)]
#[instrument(name = "get_scheduled_job_handler", skip_all)]
pub async fn get_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(sj).into_response())
}

#[utoipa::path(
    put,
    path = "/scheduled-jobs/{id}/pause",
    params(
        ("id" = String, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job paused"),
        (status = 404, description = "Scheduled job not found"),
        (status = 400, description = "Scheduled job is not active")
    )
)]
/// PUT /scheduled-jobs/{id}/pause
///
/// # Errors
#[utoipa::path(
    put,
    path = "/scheduled-jobs/{id}/pause",
    params(
        ("id" = twerk_core::id::ScheduledJobId, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job paused")
    )
)]
#[instrument(name = "pause_scheduled_job_handler", skip_all)]
pub async fn pause_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    validate_pause(&sj)?;

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
            build_scheduled_job_event_value_from_sj(&paused)?,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(status_ok_response())
}

#[utoipa::path(
    put,
    path = "/scheduled-jobs/{id}/resume",
    params(
        ("id" = String, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job resumed"),
        (status = 404, description = "Scheduled job not found"),
        (status = 400, description = "Scheduled job is not paused")
    )
)]
/// PUT /scheduled-jobs/{id}/resume
///
/// # Errors
#[utoipa::path(
    put,
    path = "/scheduled-jobs/{id}/resume",
    params(
        ("id" = twerk_core::id::ScheduledJobId, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job resumed")
    )
)]
#[instrument(name = "resume_scheduled_job_handler", skip_all)]
pub async fn resume_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    validate_resume(&sj)?;

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
            build_scheduled_job_event_value_from_sj(&resumed)?,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(status_ok_response())
}

#[utoipa::path(
    delete,
    path = "/scheduled-jobs/{id}",
    params(
        ("id" = String, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job deleted"),
        (status = 404, description = "Scheduled job not found")
    )
)]
/// DELETE /scheduled-jobs/{id}
///
/// # Errors
#[utoipa::path(
    delete,
    path = "/scheduled-jobs/{id}",
    params(
        ("id" = twerk_core::id::ScheduledJobId, Path, description = "Scheduled job ID")
    ),
    responses(
        (status = 200, description = "Scheduled job deleted")
    )
)]
#[instrument(name = "delete_scheduled_job_handler", skip_all)]
pub async fn delete_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<ScheduledJobId>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    let is_active = was_active(&sj);

    state
        .ds
        .delete_scheduled_job(&id)
        .await
        .map_err(ApiError::from)?;

    if is_active {
        let paused_sj = build_paused_for_event(&sj);
        state
            .broker
            .publish_event(
                "scheduled.job".to_string(),
                build_scheduled_job_event_value_from_sj(&paused_sj)?,
            )
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    Ok(status_ok_response())
}
