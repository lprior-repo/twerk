#![allow(clippy::needless_for_each)]
use twerk_core::id::{JobId, ScheduledJobId, TaskId};
use twerk_core::job::{
    Job, JobContext, JobDefaults, JobSchedule, JobState, JobSummary, ScheduledJob,
    ScheduledJobState, ScheduledJobSummary,
};
use twerk_core::node::Node;
use twerk_core::stats::Metrics;
use twerk_core::task::{AutoDelete, Permission, Task, TaskLogPart};
use twerk_core::user::User;
use twerk_core::webhook::Webhook;
use twerk_infrastructure::broker::QueueInfo;

use super::error::ApiError;
use super::trigger_api::{TriggerId, TriggerUpdateRequest, TriggerView};

#[derive(utoipa::OpenApi)]
#[openapi(
    info(title = "Twerk API", version = "0.1.0"),
    components(schemas(
        // Job domain
        Job,
        JobId,
        JobState,
        JobSummary,
        JobContext,
        JobDefaults,
        JobSchedule,
        ScheduledJob,
        ScheduledJobId,
        ScheduledJobState,
        ScheduledJobSummary,
        // Task domain
        Task,
        TaskId,
        TaskLogPart,
        Permission,
        AutoDelete,
        // Webhook
        Webhook,
        // Trigger domain
        TriggerId,
        TriggerView,
        TriggerUpdateRequest,
        // Other domains
        User,
        Node,
        Metrics,
        QueueInfo,
        ApiError,
    )),
    paths(
        // Health
        super::handlers::system::health_handler,
        // Jobs
        super::handlers::jobs::create::create_job_handler,
        super::handlers::jobs::read::get_job_handler,
        super::handlers::jobs::read::list_jobs_handler,
        super::handlers::jobs::mutation::cancel_job_handler,
        super::handlers::jobs::mutation::restart_job_handler,
        super::handlers::jobs::mutation::delete_job_handler,
        super::handlers::jobs::read::get_job_log_handler,
        // Scheduled jobs
        super::handlers::scheduled::create_scheduled_job_handler,
        super::handlers::scheduled::list_scheduled_jobs_handler,
        super::handlers::scheduled::get_scheduled_job_handler,
        super::handlers::scheduled::pause_scheduled_job_handler,
        super::handlers::scheduled::resume_scheduled_job_handler,
        super::handlers::scheduled::delete_scheduled_job_handler,
        // Tasks
        super::handlers::tasks::get_task_handler,
        super::handlers::tasks::get_task_log_handler,
        // Queues
        super::handlers::queues::list_queues_handler,
        super::handlers::queues::get_queue_handler,
        super::handlers::queues::delete_queue_handler,
        // System
        super::handlers::system::list_nodes_handler,
        super::handlers::system::get_metrics_handler,
        super::handlers::system::create_user_handler,
        // Triggers
        super::trigger_api::handlers::query::list_triggers_handler,
        super::trigger_api::handlers::command::create_trigger_handler,
        super::trigger_api::handlers::query::get_trigger_handler,
        super::trigger_api::handlers::command::update_trigger_handler,
        super::trigger_api::handlers::query::delete_trigger_handler,
    )
)]
pub struct ApiDoc;

/// Generate the `OpenAPI` spec as a JSON string.
///
/// # Errors
///
/// Returns an error if the `OpenAPI` spec cannot be serialized to JSON.
pub fn generate_json() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&<ApiDoc as utoipa::OpenApi>::openapi())
}

/// Return the set of route specs documented in the `OpenAPI` spec.
#[must_use]
pub fn documented_route_specs() -> Vec<(String, String)> {
    let mut routes = Vec::new();
    let spec = <ApiDoc as utoipa::OpenApi>::openapi();
    for (path, item) in spec.paths.paths {
        if item.get.is_some() {
            routes.push(("GET".to_string(), path.clone()));
        }
        if item.post.is_some() {
            routes.push(("POST".to_string(), path.clone()));
        }
        if item.put.is_some() {
            routes.push(("PUT".to_string(), path.clone()));
        }
        if item.delete.is_some() {
            routes.push(("DELETE".to_string(), path.clone()));
        }
        if item.patch.is_some() {
            routes.push(("PATCH".to_string(), path.clone()));
        }
        if item.head.is_some() {
            routes.push(("HEAD".to_string(), path.clone()));
        }
        if item.options.is_some() {
            routes.push(("OPTIONS".to_string(), path.clone()));
        }
        if item.trace.is_some() {
            routes.push(("TRACE".to_string(), path.clone()));
        }
    }
    routes.sort();
    routes
}

/// Return the set of route specs actually mounted in the router.
#[must_use]
pub fn mounted_route_specs() -> Vec<(String, String)> {
    let mut routes = vec![
        ("GET".to_string(), "/health".to_string()),
        ("GET".to_string(), "/nodes".to_string()),
        ("GET".to_string(), "/metrics".to_string()),
        ("POST".to_string(), "/users".to_string()),
        ("GET".to_string(), "/tasks/{id}".to_string()),
        ("GET".to_string(), "/tasks/{id}/log".to_string()),
        ("POST".to_string(), "/jobs".to_string()),
        ("GET".to_string(), "/jobs".to_string()),
        ("GET".to_string(), "/jobs/{id}".to_string()),
        ("DELETE".to_string(), "/jobs/{id}".to_string()),
        ("GET".to_string(), "/jobs/{id}/log".to_string()),
        ("PUT".to_string(), "/jobs/{id}/cancel".to_string()),
        ("PUT".to_string(), "/jobs/{id}/restart".to_string()),
        ("POST".to_string(), "/scheduled-jobs".to_string()),
        ("GET".to_string(), "/scheduled-jobs".to_string()),
        ("GET".to_string(), "/scheduled-jobs/{id}".to_string()),
        ("PUT".to_string(), "/scheduled-jobs/{id}/pause".to_string()),
        ("PUT".to_string(), "/scheduled-jobs/{id}/resume".to_string()),
        ("DELETE".to_string(), "/scheduled-jobs/{id}".to_string()),
        ("GET".to_string(), "/queues".to_string()),
        ("GET".to_string(), "/queues/{name}".to_string()),
        ("DELETE".to_string(), "/queues/{name}".to_string()),
        ("GET".to_string(), "/triggers".to_string()),
        ("POST".to_string(), "/triggers".to_string()),
        ("GET".to_string(), "/triggers/{id}".to_string()),
        ("PUT".to_string(), "/triggers/{id}".to_string()),
        ("DELETE".to_string(), "/triggers/{id}".to_string()),
    ];
    routes.sort();
    routes
}

/// Sync the current `OpenAPI` spec to tracked artifact files.
///
/// # Errors
///
/// Returns an error if JSON/YAML serialization fails or file writes fail.
pub fn sync_tracked_artifacts(
    workspace_root: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = generate_json()?;
    let yaml = serde_yaml::to_string(&serde_json::from_str::<serde_json::Value>(&json)?)?;

    let docs_dir = workspace_root.join("docs");
    let web_dir = workspace_root.join("crates/twerk-web");

    std::fs::write(docs_dir.join("openapi.json"), &json)?;
    std::fs::write(docs_dir.join("openapi.yaml"), &yaml)?;
    std::fs::write(web_dir.join("openapi.json"), &json)?;

    Ok(())
}
