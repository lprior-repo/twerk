use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use twerk_core::job::{ScheduledJob, ScheduledJobState};
use twerk_core::repository;
use twerk_core::user::User;

use crate::api::content_type::{
    classify_content_type, normalized_content_type, RequestContentType,
};
use crate::api::error::ApiError;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
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

pub fn parse_create_body(
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<CreateScheduledJobBody, ApiError> {
    match classify_content_type(&normalized_content_type(headers)) {
        RequestContentType::Json => {
            serde_json::from_slice(body).map_err(|error| ApiError::bad_request(error.to_string()))
        }
        RequestContentType::Yaml => crate::api::yaml::from_slice(body),
        RequestContentType::Unsupported => Err(ApiError::bad_request("unsupported content type")),
    }
}

pub fn validate_create_input(
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

    twerk_core::validation::validate_cron(&cron).map_err(ApiError::bad_request)?;
    twerk_core::validation::validate_job(
        body.name.as_ref(),
        body.tasks.as_ref(),
        body.defaults.as_ref(),
        body.output.as_ref(),
    )
    .map_err(|errors| ApiError::bad_request(errors.join("; ")))?;

    Ok((cron, tasks))
}

pub fn build_scheduled_job(
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

pub fn validate_pause(job: &ScheduledJob) -> Result<(), ApiError> {
    if job.state == ScheduledJobState::Active {
        Ok(())
    } else {
        Err(ApiError::bad_request("scheduled job is not active"))
    }
}

pub fn validate_resume(job: &ScheduledJob) -> Result<(), ApiError> {
    if job.state == ScheduledJobState::Paused {
        Ok(())
    } else {
        Err(ApiError::bad_request("scheduled job is not paused"))
    }
}

pub fn pause_state_transition(
) -> Box<dyn FnOnce(ScheduledJob) -> Result<ScheduledJob, repository::Error> + Send> {
    Box::new(|mut job| {
        job.state = ScheduledJobState::Paused;
        Ok(job)
    })
}

pub fn resume_state_transition(
) -> Box<dyn FnOnce(ScheduledJob) -> Result<ScheduledJob, repository::Error> + Send> {
    Box::new(|mut job| {
        job.state = ScheduledJobState::Active;
        Ok(job)
    })
}

pub fn status_ok_response() -> Response {
    (StatusCode::OK, axum::Json(json!({"status": "OK"}))).into_response()
}

pub fn was_active(job: &ScheduledJob) -> bool {
    job.state == ScheduledJobState::Active
}

pub fn build_paused_for_event(job: &ScheduledJob) -> ScheduledJob {
    let mut paused = job.clone();
    paused.state = ScheduledJobState::Paused;
    paused
}

pub fn scheduled_job_event_value(job: &ScheduledJob) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(job).map_err(|error| ApiError::internal(error.to_string()))
}
