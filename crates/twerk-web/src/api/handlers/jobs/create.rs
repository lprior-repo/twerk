use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::sync::broadcast::error::RecvError;
use tracing::instrument;
use twerk_core::id::JobId;
use twerk_core::job::{new_job_summary, Job, JobEvent};

use super::types::{CreateJobQuery, WaitMode};
use crate::api::content_type::{
    classify_content_type, normalized_content_type, RequestContentType,
};
use crate::api::error::ApiError;
use crate::api::handlers::{default_user, AppState};
use crate::api::openapi_types::{CreateJobResponse, MessageResponse};
use crate::middleware::hooks::{on_read_job, on_read_job_summary};

fn parse_job_request(headers: &HeaderMap, body: &Bytes) -> Result<Job, ApiError> {
    match classify_content_type(&normalized_content_type(headers)) {
        RequestContentType::Json => {
            serde_json::from_slice(body).map_err(|error| ApiError::bad_request(error.to_string()))
        }
        RequestContentType::Yaml => crate::api::yaml::from_slice(body),
        RequestContentType::Unsupported => Err(ApiError::bad_request("unsupported content type")),
    }
}

async fn enrich_job_defaults(state: &AppState, mut job: Job) -> Job {
    if job.id.is_none() {
        job.id = JobId::new(twerk_core::uuid::new_short_uuid()).ok();
    }
    job.task_count = job.tasks.as_ref().map_or(job.task_count, |tasks| {
        i64::try_from(tasks.len()).map_or(i64::MAX, std::convert::identity)
    });
    if job.created_at.is_none() {
        job.created_at = Some(time::OffsetDateTime::now_utc());
    }
    if job.created_by.is_none() {
        job.created_by = default_user(state).await;
    }
    job
}

fn validate_job(job: &Job) -> Result<(), ApiError> {
    twerk_core::validation::validate_job(
        job.name.as_ref(),
        job.tasks.as_ref(),
        job.defaults.as_ref(),
        job.output.as_ref(),
    )
    .map_err(|errors| ApiError::bad_request(errors.join("; ")))
}

/// POST /jobs
#[utoipa::path(
    post,
    path = "/jobs",
    params(
        ("wait" = Option<String>, Query, description = "Whether to block until the job completes (true/false/blocking)")
    ),
    request_body(
        description = "Job definition as JSON or YAML",
        content(
            (Job = "application/json"),
            (Job = "application/yaml"),
            (Job = "application/x-yaml"),
            (Job = "text/yaml")
        )
    ),
    responses(
        (status = 200, description = "Job created or completed when wait=blocking", body = CreateJobResponse, content_type = "application/json"),
        (status = 400, description = "Invalid job definition or unsupported content type", body = MessageResponse, content_type = "application/json")
    )
)]
#[instrument(name = "create_job_handler", skip_all)]
/// # Errors
/// Returns an error when the request body cannot be parsed, the job fails validation, the
/// datastore or broker operation fails, or the blocking wait path times out.
pub async fn create_job_handler(
    State(state): State<AppState>,
    Query(query): Query<CreateJobQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let job = enrich_job_defaults(&state, parse_job_request(&headers, &body)?).await;
    validate_job(&job)?;

    match query
        .wait
        .as_ref()
        .map_or_else(WaitMode::default, |v| v.clone())
    {
        WaitMode::Blocking => wait_for_job_completion(state, job).await,
        WaitMode::Detached => create_job_no_wait(state, job).await,
    }
}

async fn wait_for_job_completion(state: AppState, job: Job) -> Result<Response, ApiError> {
    let job_id = job
        .id
        .clone()
        .ok_or_else(|| ApiError::internal("job id missing"))?;

    let mut subscription = state
        .broker
        .subscribe("job.*".to_string())
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    state.ds.create_job(&job).await.map_err(ApiError::from)?;
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    let completion = tokio::time::timeout(tokio::time::Duration::from_secs(3600), async {
        loop {
            match subscription.recv().await {
                Ok(
                    JobEvent::Completed(ref finished)
                    | JobEvent::Failed(ref finished)
                    | JobEvent::Cancelled(ref finished),
                ) if finished.id.as_ref() == Some(&job_id) => return Ok(finished.clone()),
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(ApiError::internal("subscription channel closed"));
                }
            }
        }
    })
    .await;

    match completion {
        Ok(Ok(mut finished_job)) => {
            let secrets = finished_job
                .secrets
                .as_ref()
                .map_or_else(std::collections::HashMap::new, |v| v.clone());
            on_read_job(&mut finished_job, &secrets);

            if let Some(job_id) = &finished_job.id {
                if let Ok(actual_tasks) = state.ds.get_all_tasks_for_job(job_id.as_ref()).await {
                    if !actual_tasks.is_empty() {
                        finished_job.tasks = Some(actual_tasks);
                    }
                }
            }

            Ok((StatusCode::OK, axum::Json(finished_job)).into_response())
        }
        Ok(Err(error)) => Err(error),
        Err(_) => Err(ApiError::internal("timeout waiting for job")),
    }
}

async fn create_job_no_wait(state: AppState, job: Job) -> Result<Response, ApiError> {
    state.ds.create_job(&job).await.map_err(ApiError::from)?;
    state
        .broker
        .publish_job(&job)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    let mut summary = new_job_summary(&job);
    on_read_job_summary(&mut summary);

    Ok((StatusCode::OK, axum::Json(summary)).into_response())
}
