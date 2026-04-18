//! Job handlers - API endpoints for job operations.

use axum::body::Bytes;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use twerk_core::id::JobId;
use twerk_core::job::{new_job_summary, Job, JobEvent, JobState};

use super::super::error::ApiError;
use super::super::redact::redact_task_log_parts;
use super::tasks::{PaginationQuery, RawPaginationQuery};
use super::{default_user, extract_current_user, parse_page, parse_size, AppState};
use crate::middleware::hooks::{on_read_job, on_read_job_summary};
use tracing::instrument;

/// Whether the create-job endpoint should block until the job completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaitMode {
    #[default]
    Detached,
    Blocking,
}

impl<'de> Deserialize<'de> for WaitMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WaitModeHelper {
            Bool(bool),
            String(String),
        }

        let helper = WaitModeHelper::deserialize(deserializer)?;
        match helper {
            WaitModeHelper::Bool(b) => Ok(if b {
                WaitMode::Blocking
            } else {
                WaitMode::Detached
            }),
            WaitModeHelper::String(s) => match s.to_lowercase().as_str() {
                "blocking" | "true" | "1" | "yes" => Ok(WaitMode::Blocking),
                _ => Ok(WaitMode::Detached),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreateJobQuery {
    pub wait: Option<WaitMode>,
}

/// POST /jobs
///
/// # Errors
#[utoipa::path(
    post,
    path = "/jobs",
<<<<<<< HEAD
    params(
        ("wait" = Option<String>, Query, description = "Whether to block until the job completes (true/false/blocking)")
    ),
    request_body(content = String, description = "Job definition as JSON or YAML", content_type = "application/json"),
    responses(
        (status = 200, description = "Job created"),
        (status = 400, description = "Invalid job definition or unsupported content type")
=======
    request_body = Job,
    responses(
        (status = 200, description = "Job created", body = Job)
>>>>>>> origin/tw-polecat/iota
    )
)]
#[instrument(name = "create_job_handler", skip_all)]
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
        "text/yaml" | "application/x-yaml" | "application/yaml" => {
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

    // JobState defaults to Pending via serde/Default trait

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

    match cq.wait.unwrap_or_default() {
        WaitMode::Blocking => wait_for_job_completion(state, job).await,
        WaitMode::Detached => create_job_no_wait(state, job).await,
    }
}

async fn wait_for_job_completion(state: AppState, job: Job) -> Result<Response, ApiError> {
    let job_id = job
        .id
        .clone()
        .ok_or_else(|| ApiError::internal("job id missing"))?;

    let mut rx = state
        .broker
        .subscribe("job.*".to_string())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    state.ds.create_job(&job).await.map_err(ApiError::from)?;
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let result = tokio::time::timeout(tokio::time::Duration::from_secs(3600), async {
        loop {
            match rx.recv().await {
                Ok(
                    JobEvent::Completed(ref job)
                    | JobEvent::Failed(ref job)
                    | JobEvent::Cancelled(ref job),
                ) if job.id.as_ref() == Some(&job_id) => {
                    return Ok(job.clone());
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(ApiError::internal("subscription channel closed"));
                }
            }
        }
    })
    .await;

    match result {
        Ok(Ok(mut finished_job)) => {
            let secrets = finished_job.secrets.clone().unwrap_or_default();
            on_read_job(&mut finished_job, &secrets);

            // Fetch actual task states from the tasks table
            if let Some(ref job_id) = finished_job.id {
                if let Ok(actual_tasks) = state.ds.get_all_tasks_for_job(job_id.as_ref()).await {
                    if !actual_tasks.is_empty() {
                        finished_job.tasks = Some(actual_tasks);
                    }
                }
            }

            Ok((StatusCode::OK, axum::Json(finished_job)).into_response())
        }
        Ok(Err(e)) => Err(e),
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
#[utoipa::path(
    get,
    path = "/jobs/{id}",
    params(
<<<<<<< HEAD
        ("id" = String, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job found"),
        (status = 404, description = "Job not found")
=======
        ("id" = JobId, description = "The job ID")
    ),
    responses(
        (status = 200, description = "Job found", body = Job)
>>>>>>> origin/tw-polecat/iota
    )
)]
#[instrument(name = "get_job_handler", skip_all)]
pub async fn get_job_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
) -> Result<Response, ApiError> {
    let mut job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    // Fetch actual task states from the tasks table, not just original definitions
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

    let secrets = job.secrets.clone().unwrap_or_default();
    on_read_job(&mut job, &secrets);

    Ok(axum::Json(job).into_response())
}

/// GET /jobs
///
/// # Errors
#[utoipa::path(
    get,
    path = "/jobs",
<<<<<<< HEAD
    params(
        ("page" = Option<String>, Query, description = "Page number"),
        ("size" = Option<String>, Query, description = "Page size"),
        ("q" = Option<String>, Query, description = "Search query")
    ),
    responses(
        (status = 200, description = "List of jobs")
=======
    responses(
        (status = 200, description = "List of jobs", body = Vec<Job>)
>>>>>>> origin/tw-polecat/iota
    )
)]
#[instrument(name = "list_jobs_handler", skip_all)]
pub async fn list_jobs_handler(
    State(state): State<AppState>,
    Query(raw): Query<RawPaginationQuery>,
    req: axum::extract::Request,
) -> Result<Response, ApiError> {
    let qp = PaginationQuery::from_raw(raw);
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
#[utoipa::path(
    put,
    path = "/jobs/{id}/cancel",
    params(
<<<<<<< HEAD
        ("id" = String, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job cancelled"),
        (status = 400, description = "Job cannot be cancelled in its current state")
=======
        ("id" = JobId, description = "The job ID")
    ),
    responses(
        (status = 200, description = "Job cancelled", body = Job)
>>>>>>> origin/tw-polecat/iota
    )
)]
#[instrument(name = "cancel_job_handler", skip_all)]
pub async fn cancel_job_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
) -> Result<Response, ApiError> {
    let mut job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    if matches!(
        job.state,
        JobState::Completed | JobState::Failed | JobState::Cancelled
    ) {
        return Err(ApiError::bad_request(
            "job cannot be cancelled in its current state",
        ));
    }

    job.state = JobState::Cancelled;
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
#[utoipa::path(
    put,
    path = "/jobs/{id}/restart",
    params(
<<<<<<< HEAD
        ("id" = String, Path, description = "Job ID")
    ),
    responses(
        (status = 200, description = "Job restarted"),
        (status = 400, description = "Job cannot be restarted")
=======
        ("id" = JobId, description = "The job ID")
    ),
    responses(
        (status = 200, description = "Job restarted", body = Job)
>>>>>>> origin/tw-polecat/iota
    )
)]
#[instrument(name = "restart_job_handler", skip_all)]
pub async fn restart_job_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
) -> Result<Response, ApiError> {
    let mut job = state.ds.get_job_by_id(&id).await.map_err(ApiError::from)?;

    if !matches!(job.state, JobState::Failed | JobState::Cancelled) {
        return Err(ApiError::bad_request("job cannot be restarted"));
    }

    job.state = JobState::Restart;
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
#[utoipa::path(
    get,
    path = "/jobs/{id}/log",
    params(
<<<<<<< HEAD
        ("id" = String, Path, description = "Job ID"),
        ("page" = Option<String>, Query, description = "Page number"),
        ("size" = Option<String>, Query, description = "Page size")
    ),
    responses(
        (status = 200, description = "Job log entries"),
        (status = 404, description = "Job not found")
=======
        ("id" = JobId, description = "The job ID")
    ),
    responses(
        (status = 200, description = "Job log parts", body = Vec<String>)
>>>>>>> origin/tw-polecat/iota
    )
)]
#[instrument(name = "get_job_log_handler", skip_all)]
pub async fn get_job_log_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<JobId>,
    Query(raw): Query<RawPaginationQuery>,
) -> Result<Response, ApiError> {
    let qp = PaginationQuery::from_raw(raw);
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
