use axum::body::Bytes;
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use twerk_core::job::{Job, JOB_STATE_CANCELLED, JOB_STATE_COMPLETED, JOB_STATE_FAILED, JOB_STATE_SCHEDULED, JOB_STATE_RUNNING, JOB_STATE_RESTART};
use twerk_core::user::UsernameValue;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

use super::error::ApiError;
use super::AppState;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub q: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreateJobQuery {
    pub wait: Option<bool>,
}

fn parse_page(p: Option<i64>) -> i64 {
    p.filter(|&v| v >= 1).unwrap_or(1)
}

fn parse_size(p: Option<i64>, default: i64, max: i64) -> i64 {
    p.filter(|&v| v >= 1).unwrap_or(default).clamp(1, max)
}

fn extract_current_user(req: &Request) -> String {
    req.extensions()
        .get::<UsernameValue>()
        .map(|v| v.0.clone())
        .unwrap_or_default()
}

// Health
pub async fn health_handler(State(state): State<AppState>) -> Response {
    let ds_ok = state.ds.health_check().await.is_ok();
    let broker_ok = state.broker.health_check().await.is_ok();

    let (status, body) = if ds_ok && broker_ok {
        (
            StatusCode::OK,
            json!({"status": "UP", "version": VERSION}),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"status": "DOWN", "version": VERSION}),
        )
    };
    (status, axum::Json(body)).into_response()
}

// Tasks
pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let task = state
        .ds
        .get_task_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(task).into_response())
}

pub async fn get_task_log_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 25, 100);
    let q = qp.q.unwrap_or_default();

    // Verify task exists
    state
        .ds
        .get_task_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    let parts = state
        .ds
        .get_task_log_parts(&id, &q, page, size)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(parts).into_response())
}

// Jobs
pub async fn create_job_handler(
    State(state): State<AppState>,
    Query(_cq): Query<CreateJobQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let job: Job = match content_type {
        "application/json" => serde_json::from_slice(&body)
            .map_err(|e| ApiError::bad_request(e.to_string()))?,
        _ => return Err(ApiError::bad_request("unsupported content type")),
    };

    // Simplified: just create the job in DS and publish to broker
    state.ds.create_job(&job).await.map_err(ApiError::from)?;
    state.broker.publish_job(&job).await.map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(job)).into_response())
}

pub async fn get_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let job = state
        .ds
        .get_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(job).into_response())
}

pub async fn list_jobs_handler(
    State(state): State<AppState>,
    Query(qp): Query<PaginationQuery>,
    req: Request,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 10, 20);
    let q = qp.q.unwrap_or_default();
    let current_user = extract_current_user(&req);

    let result = state
        .ds
        .get_jobs(&current_user, &q, page, size)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(result).into_response())
}

pub async fn cancel_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut job = state
        .ds
        .get_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    if job.state != JOB_STATE_RUNNING && job.state != JOB_STATE_SCHEDULED {
        return Err(ApiError::bad_request("job is not running"));
    }

    job.state = JOB_STATE_CANCELLED.to_string();
    state.broker.publish_job(&job).await.map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

pub async fn restart_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut job = state
        .ds
        .get_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    if job.state != JOB_STATE_FAILED && job.state != JOB_STATE_CANCELLED {
        return Err(ApiError::bad_request("job cannot be restarted"));
    }

    job.state = JOB_STATE_RESTART.to_string();
    state.broker.publish_job(&job).await.map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

pub async fn get_job_log_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 25, 100);

    // Verify job exists
    state
        .ds
        .get_job_by_id(&id)
        .await
        .map_err(ApiError::from)?;

    let parts = state
        .ds
        .get_job_log_parts(&id, "", page, size)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(parts).into_response())
}

// Queues
pub async fn list_queues_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let queues = state.broker.queues().await.map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(axum::Json(queues).into_response())
}

pub async fn get_queue_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let queue = state
        .broker
        .queue_info(name.clone())
        .await
        .map_err(|_| ApiError::not_found(format!("queue {name} not found")))?;

    Ok(axum::Json(queue).into_response())
}

pub async fn delete_queue_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    state.broker.delete_queue(name).await.map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::OK.into_response())
}

// Nodes
pub async fn list_nodes_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let nodes = state.ds.get_active_nodes().await.map_err(ApiError::from)?;
    Ok(axum::Json(nodes).into_response())
}

// Metrics
pub async fn get_metrics_handler(State(state): State<AppState>) -> Result<Response, ApiError> {
    let metrics = state.ds.get_metrics().await.map_err(ApiError::from)?;
    Ok(axum::Json(metrics).into_response())
}

// Users
#[derive(Debug, Deserialize)]
pub struct CreateUserBody {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub async fn create_user_handler(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateUserBody>,
) -> Result<Response, ApiError> {
    let username = body.username.ok_or_else(|| ApiError::bad_request("missing username"))?;
    let password = body.password.ok_or_else(|| ApiError::bad_request("missing password"))?;

    let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| ApiError::internal(e.to_string()))?;

    let user = twerk_core::user::User {
        username: Some(username),
        password_hash: Some(password_hash),
        ..Default::default()
    };

    state.ds.create_user(&user).await.map_err(ApiError::from)?;

    Ok(StatusCode::OK.into_response())
}

// Scheduled Jobs
pub async fn create_scheduled_job_handler(
    State(_state): State<AppState>,
) -> Result<Response, ApiError> {
    Err(ApiError::internal("not implemented"))
}

pub async fn list_scheduled_jobs_handler(
    State(_state): State<AppState>,
) -> Result<Response, ApiError> {
    Err(ApiError::internal("not implemented"))
}

pub async fn get_scheduled_job_handler(
    State(_state): State<AppState>,
) -> Result<Response, ApiError> {
    Err(ApiError::internal("not implemented"))
}

pub async fn pause_scheduled_job_handler(
    State(_state): State<AppState>,
) -> Result<Response, ApiError> {
    Err(ApiError::internal("not implemented"))
}

pub async fn resume_scheduled_job_handler(
    State(_state): State<AppState>,
) -> Result<Response, ApiError> {
    Err(ApiError::internal("not implemented"))
}

pub async fn delete_scheduled_job_handler(
    State(_state): State<AppState>,
) -> Result<Response, ApiError> {
    Err(ApiError::internal("not implemented"))
}
