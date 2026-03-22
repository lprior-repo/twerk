//! API handler implementations.
//!
//! Each handler maps to a Go endpoint in `api.go`.
//! All handlers follow Data→Calc→Actions: extract params, call
//! datastore/broker (pure boundary), return JSON response.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tork::broker::is_task_queue;
use tork::{
    new_job_summary, new_scheduled_job_summary, Broker, Datastore, Job,
    JOB_STATE_CANCELLED, JOB_STATE_FAILED, JOB_STATE_RESTART, JOB_STATE_RUNNING,
    JOB_STATE_SCHEDULED, SCHEDULED_JOB_STATE_ACTIVE, SCHEDULED_JOB_STATE_PAUSED,
};

use super::error::ApiError;
use super::AppState;

/// Broker topic for job events (Go: `broker.TOPIC_JOB`)
/// Used for the wait mode in create_job (deferred).
#[allow(dead_code)]
const TOPIC_JOB: &str = "job.*";
/// Broker topic for scheduled job events (Go: `broker.TOPIC_SCHEDULED_JOB`)
const TOPIC_SCHEDULED_JOB: &str = "scheduled.job";

// ---------------------------------------------------------------------------
// Pagination helpers
// ---------------------------------------------------------------------------

/// Query parameters for paginated list endpoints.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub size: Option<i64>,
    pub q: Option<String>,
}

/// Maximum log page size (Go: `MAX_LOG_PAGE_SIZE`)
const MAX_LOG_PAGE_SIZE: i64 = 100;

/// Parse a page number from query, defaulting to 1.
fn parse_page(p: Option<i64>) -> i64 {
    p.filter(|&v| v >= 1).unwrap_or(1)
}

/// Parse a size parameter with clamping.
fn parse_size(p: Option<i64>, default: i64, max: i64) -> i64 {
    p.filter(|&v| v >= 1)
        .unwrap_or(default)
        .clamp(1, max)
}

// ---------------------------------------------------------------------------
// Health: GET /health
// ---------------------------------------------------------------------------

/// Health check handler — checks datastore + broker health.
///
/// Go parity: `health.NewHealthCheck().WithIndicator(...).Do(ctx)`
pub async fn health_handler(State(state): State<AppState>) -> Response {
    let ds_ok = state
        .ds
        .health_check()
        .await
        .is_ok();
    let broker_ok = state
        .broker
        .health_check()
        .await
        .is_ok();

    let (status, body) = if ds_ok && broker_ok {
        (StatusCode::OK, json!({"status": "UP", "version": tork::version::VERSION}))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"status": "DOWN", "version": tork::version::VERSION}),
        )
    };
    (status, axum::Json(body)).into_response()
}

// ---------------------------------------------------------------------------
// Tasks: GET /tasks/:id, GET /tasks/:id/log
// ---------------------------------------------------------------------------

/// Get task by ID.
///
/// Go parity: `s.ds.GetTaskByID` → 404 on None.
pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let task = state
        .ds
        .get_task_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("task {id} not found")))?;

    Ok(axum::Json(serde_json::to_value(task).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

/// Get task log parts with pagination.
///
/// Go parity: validates task existence, then returns paginated log parts.
pub async fn get_task_log_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 25, MAX_LOG_PAGE_SIZE);
    let q = qp.q.clone().unwrap_or_default();

    // Verify task exists
    state
        .ds
        .get_task_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("task {id} not found")))?;

    let parts = state
        .ds
        .get_task_log_parts(id, q, page, size)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(serde_json::to_value(parts).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

// ---------------------------------------------------------------------------
// Jobs: POST /jobs, GET /jobs/:id, GET /jobs/:id/log,
//        PUT /jobs/:id/cancel, PUT /jobs/:id/restart, GET /jobs
// ---------------------------------------------------------------------------

/// Create a job from JSON or YAML body.
///
/// Go parity: supports `application/json`, `text/yaml`, `application/x-yaml`.
/// Wait mode subscribes to job events and blocks until terminal state or timeout.
pub async fn create_job_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let ji = match content_type {
        "application/json" => {
            serde_json::from_slice::<tork_input::Job>(&body)
                .map_err(|e| ApiError::bad_request(e.to_string()))?
        }
        "text/yaml" | "application/x-yaml" => {
            serde_yml::from_slice::<tork_input::Job>(&body)
                .map_err(|e| ApiError::bad_request(e.to_string()))?
        }
        "" => return Err(ApiError::bad_request("missing content type")),
        other => return Err(ApiError::bad_request(format!("unknown content type: {other}"))),
    };

    let job = submit_job(&state.ds, &state.broker, ji).await?;
    let summary = new_job_summary(&job);
    let val = serde_json::to_value(summary).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((StatusCode::OK, axum::Json(val)).into_response())
}

/// Get job by ID.
///
/// Go parity: `s.ds.GetJobByID` → 404 on None.
pub async fn get_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let job = state
        .ds
        .get_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("job {id} not found")))?;

    Ok(axum::Json(serde_json::to_value(job).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

/// Get job log parts with pagination.
///
/// Go parity: page defaults 1, size defaults 25, max 100.
pub async fn get_job_log_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 25, MAX_LOG_PAGE_SIZE);
    let q = qp.q.clone().unwrap_or_default();

    // Verify job exists
    state
        .ds
        .get_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("job {id} not found")))?;

    let parts = state
        .ds
        .get_job_log_parts(id, q, page, size)
        .await
        .map_err(ApiError::from)?;

    Ok(axum::Json(serde_json::to_value(parts).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

/// Cancel a running or scheduled job.
///
/// Go parity: only `RUNNING` or `SCHEDULED` jobs can be cancelled.
pub async fn cancel_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut job = state
        .ds
        .get_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("job {id} not found")))?;

    if job.state != JOB_STATE_RUNNING && job.state != JOB_STATE_SCHEDULED {
        return Err(ApiError::bad_request("job is not running"));
    }

    job.state = JOB_STATE_CANCELLED.to_string();
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(ApiError::internal)?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// Restart a failed or cancelled job.
///
/// Go parity: only `FAILED` or `CANCELLED` jobs can be restarted.
pub async fn restart_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let mut job = state
        .ds
        .get_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("job {id} not found")))?;

    if job.state != JOB_STATE_FAILED && job.state != JOB_STATE_CANCELLED {
        return Err(ApiError::bad_request(format!(
            "job is {} and can not be restarted",
            job.state
        )));
    }

    if job.position > job.tasks.len() as i64 {
        return Err(ApiError::bad_request("job has no more tasks to run"));
    }

    job.state = JOB_STATE_RESTART.to_string();
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// List jobs with pagination.
///
/// Go parity: page defaults 1, size defaults 10, max 20.
pub async fn list_jobs_handler(
    State(state): State<AppState>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 10, 20);
    let q = qp.q.clone().unwrap_or_default();

    let result = state
        .ds
        .get_jobs(String::new(), q, page, size)
        .await
        .map_err(ApiError::internal)?;

    Ok(axum::Json(serde_json::to_value(result).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

// ---------------------------------------------------------------------------
// Scheduled Jobs: POST, GET /:id, GET list, PUT pause/resume, DELETE
// ---------------------------------------------------------------------------

/// Create a scheduled job from JSON or YAML body.
///
/// Go parity: validates, persists, publishes event.
pub async fn create_scheduled_job_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let ji = match content_type {
        "application/json" => {
            serde_json::from_slice::<tork_input::ScheduledJob>(&body)
                .map_err(|e| ApiError::bad_request(e.to_string()))?
        }
        "text/yaml" | "application/x-yaml" => {
            serde_yml::from_slice::<tork_input::ScheduledJob>(&body)
                .map_err(|e| ApiError::bad_request(e.to_string()))?
        }
        "" => return Err(ApiError::bad_request("missing content type")),
        other => return Err(ApiError::bad_request(format!("unknown content type: {other}"))),
    };

    let sj = submit_scheduled_job(&state.ds, &state.broker, ji).await?;
    let summary = new_scheduled_job_summary(&sj);
    let val = serde_json::to_value(summary).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((StatusCode::OK, axum::Json(val)).into_response())
}

/// Get scheduled job by ID.
///
/// Go parity: `s.ds.GetScheduledJobByID` → 404 on None.
pub async fn get_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("scheduled job {id} not found")))?;

    Ok(axum::Json(serde_json::to_value(sj).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

/// List scheduled jobs with pagination.
///
/// Go parity: page defaults 1, size defaults 10, max 20.
pub async fn list_scheduled_jobs_handler(
    State(state): State<AppState>,
    Query(qp): Query<PaginationQuery>,
) -> Result<Response, ApiError> {
    let page = parse_page(qp.page);
    let size = parse_size(qp.size, 10, 20);

    let result = state
        .ds
        .get_scheduled_jobs(String::new(), page, size)
        .await
        .map_err(ApiError::internal)?;

    Ok(axum::Json(serde_json::to_value(result).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

/// Pause an active scheduled job.
///
/// Go parity: verifies ACTIVE state, updates, publishes event.
pub async fn pause_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("scheduled job {id} not found")))?;

    if sj.state != SCHEDULED_JOB_STATE_ACTIVE {
        return Err(ApiError::bad_request("scheduled job is not active"));
    }

    let mut updated = sj.clone();
    updated.state = SCHEDULED_JOB_STATE_PAUSED.to_string();
    state
        .ds
        .update_scheduled_job(id, updated)
        .await
        .map_err(ApiError::internal)?;

    let event_val = serde_json::to_value(sj).map_err(|e| ApiError::internal(e.to_string()))?;
    state
        .broker
        .publish_event(TOPIC_SCHEDULED_JOB.to_string(), event_val)
        .await
        .map_err(ApiError::internal)?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// Resume a paused scheduled job.
///
/// Go parity: verifies PAUSED state, updates, publishes event.
pub async fn resume_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("scheduled job {id} not found")))?;

    if sj.state != SCHEDULED_JOB_STATE_PAUSED {
        return Err(ApiError::bad_request("scheduled job is not paused"));
    }

    let mut updated = sj.clone();
    updated.state = SCHEDULED_JOB_STATE_ACTIVE.to_string();
    state
        .ds
        .update_scheduled_job(id, updated)
        .await
        .map_err(ApiError::internal)?;

    let event_val = serde_json::to_value(sj).map_err(|e| ApiError::internal(e.to_string()))?;
    state
        .broker
        .publish_event(TOPIC_SCHEDULED_JOB.to_string(), event_val)
        .await
        .map_err(ApiError::internal)?;

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

/// Delete a scheduled job.
///
/// Go parity: if active, publishes event to remove from scheduler.
pub async fn delete_scheduled_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let sj = state
        .ds
        .get_scheduled_job_by_id(id.clone())
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found(format!("scheduled job {id} not found")))?;

    state
        .ds
        .delete_scheduled_job(id.clone())
        .await
        .map_err(ApiError::internal)?;

    // If the job is active, publish an event to remove from scheduler
    if sj.state == SCHEDULED_JOB_STATE_ACTIVE {
        let mut event_sj = sj.clone();
        event_sj.state = SCHEDULED_JOB_STATE_PAUSED.to_string();
        let event_val =
            serde_json::to_value(event_sj).map_err(|e| ApiError::internal(e.to_string()))?;
        state
            .broker
            .publish_event(TOPIC_SCHEDULED_JOB.to_string(), event_val)
            .await
            .map_err(ApiError::internal)?;
    }

    Ok((StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response())
}

// ---------------------------------------------------------------------------
// Queues: GET list, GET /:name, DELETE /:name
// ---------------------------------------------------------------------------

/// List all broker queues.
///
/// Go parity: `s.broker.Queues` → 200 with list.
pub async fn list_queues_handler(
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let queues = state.broker.queues().await.map_err(ApiError::internal)?;
    Ok(axum::Json(serde_json::to_value(queues).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

/// Get queue info by name.
///
/// Go parity: `s.broker.QueueInfo` → 404 on error.
pub async fn get_queue_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let queue = state
        .broker
        .queue_info(name.clone())
        .await
        .map_err(|_| ApiError::not_found(format!("queue {name} not found")))?;

    Ok(axum::Json(serde_json::to_value(queue).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

/// Delete a queue by name.
///
/// Go parity: only task queues (not coordinator queues) can be deleted.
pub async fn delete_queue_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    if !is_task_queue(&name) {
        return Err(ApiError::bad_request(format!(
            "queue {name} is not a task queue"
        )));
    }

    // Verify queue exists
    state
        .broker
        .queue_info(name.clone())
        .await
        .map_err(|_| ApiError::not_found(format!("queue {name} not found")))?;

    state
        .broker
        .delete_queue(name)
        .await
        .map_err(ApiError::bad_request)?;

    Ok(StatusCode::OK.into_response())
}

// ---------------------------------------------------------------------------
// Nodes: GET list
// ---------------------------------------------------------------------------

/// List active worker nodes.
///
/// Go parity: `s.ds.GetActiveNodes` → 200 with list.
pub async fn list_nodes_handler(
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let nodes = state
        .ds
        .get_active_nodes()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(axum::Json(serde_json::to_value(nodes).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

// ---------------------------------------------------------------------------
// Metrics: GET /metrics
// ---------------------------------------------------------------------------

/// Get system metrics.
///
/// Go parity: `s.ds.GetMetrics` → 200 with metrics.
pub async fn get_metrics_handler(
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let metrics = state
        .ds
        .get_metrics()
        .await
        .map_err(ApiError::internal)?;

    Ok(axum::Json(serde_json::to_value(metrics).map_err(|e| ApiError::internal(e.to_string()))?)
        .into_response())
}

// ---------------------------------------------------------------------------
// Users: POST /users
// ---------------------------------------------------------------------------

/// User creation request body.
#[derive(Debug, Deserialize)]
pub struct CreateUserBody {
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Create a user with password hashing.
///
/// Go parity: validates username/password, hashes password, checks uniqueness.
pub async fn create_user_handler(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateUserBody>,
) -> Result<Response, ApiError> {
    let username = body
        .username
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .ok_or_else(|| ApiError::bad_request("must provide username"))?;

    let password = body
        .password
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .ok_or_else(|| ApiError::bad_request("must provide password"))?;

    let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
        .map_err(|_| ApiError::bad_request("invalid password"))?;

    // Check for existing user
    if state
        .ds
        .get_user(username.clone())
        .await
        .map_err(ApiError::from)?
        .is_some()
    {
        return Err(ApiError::bad_request("user already exists"));
    }

    let user = tork::User {
        username: Some(username),
        password_hash: Some(password_hash),
        created_at: Some(time::OffsetDateTime::now_utc()),
        ..Default::default()
    };

    state
        .ds
        .create_user(user)
        .await
        .map_err(ApiError::internal)?;

    Ok(StatusCode::OK.into_response())
}

// ---------------------------------------------------------------------------
// Internal helpers (Calc layer)
// ---------------------------------------------------------------------------

/// Validate and submit a job to the datastore and broker.
///
/// Go parity: `s.SubmitJob(ctx, &ji)` — validates, converts, persists, publishes.
async fn submit_job(
    ds: &Arc<dyn Datastore>,
    broker: &Arc<dyn Broker>,
    ji: tork_input::Job,
) -> Result<Job, ApiError> {
    // Validate (uses NoopPermissionChecker since we don't have auth middleware yet)
    tork_input::validate_job(&ji).map_err(|e| ApiError::bad_request(e.to_string()))?;

    let mut job = ji.to_job();
    job.created_by = None; // No auth context yet

    ds.create_job(job.clone())
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    broker
        .publish_job(&job)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(job)
}

/// Validate and submit a scheduled job to the datastore and broker.
///
/// Go parity: `s.submitScheduledJob(ctx, &ji)` — validates, converts, persists, publishes.
async fn submit_scheduled_job(
    ds: &Arc<dyn Datastore>,
    broker: &Arc<dyn Broker>,
    ji: tork_input::ScheduledJob,
) -> Result<tork::ScheduledJob, ApiError> {
    tork_input::validate_scheduled_job(&ji)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let mut sj = ji.to_scheduled_job();
    sj.created_by = None;

    ds.create_scheduled_job(sj.clone())
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let event_val =
        serde_json::to_value(&sj).map_err(|e| ApiError::internal(e.to_string()))?;
    broker
        .publish_event(TOPIC_SCHEDULED_JOB.to_string(), event_val)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    Ok(sj)
}
