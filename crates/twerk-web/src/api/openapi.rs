#![allow(clippy::needless_for_each)]
use serde::Serialize;
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
use twerk_core::trigger::Trigger;

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"name": "example-task"}))]
pub struct TaskSchema {
    #[serde(flatten)]
    pub inner: Task,
}

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"name": "example-scheduled-job"}))]
pub struct ScheduledJobSchema {
    #[serde(flatten)]
    pub inner: ScheduledJob,
}

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"name": "example-trigger"}))]
pub struct TriggerSchema {
    #[serde(flatten)]
    pub inner: Trigger,
}

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"name": "example-user"}))]
pub struct UserSchema {
    #[serde(flatten)]
    pub inner: User,
}

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"name": "default", "size": 0, "subscribers": 0, "unacked": 0}))]
pub struct QueueInfoSchema {
    #[serde(flatten)]
    pub inner: QueueInfo,
}

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"name": "example-node", "status": "UP"}))]
pub struct NodeSchema {
    #[serde(flatten)]
    pub inner: Node,
}

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"jobs": {"running": 0}, "tasks": {"running": 0}, "nodes": {"online": 0, "cpuPercent": 0.0}}))]
pub struct MetricsSchema {
    #[serde(flatten)]
    pub inner: Metrics,
}

#[derive(utoipa::ToSchema)]
#[schema(example = json!({"message": "error message"}))]
pub struct ApiErrorSchema {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Health check successful")
    )
)]
pub fn health_path() {}

#[utoipa::path(
    get,
    path = "/jobs",
    responses(
        (status = 200, description = "List jobs")
    )
)]
pub fn list_jobs_path() {}

#[utoipa::path(
    get,
    path = "/nodes",
    responses(
        (status = 200, description = "List of active nodes")
    )
)]
pub fn list_nodes_path() {}

#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "System metrics")
    )
)]
pub fn get_metrics_path() {}

#[utoipa::path(
    post,
    path = "/users",
    responses(
        (status = 200, description = "User created"),
        (status = 400, description = "Missing username or password")
    )
)]
pub fn create_user_path() {}

#[utoipa::path(
    get,
    path = "/api/v1/triggers",
    responses(
        (status = 200, description = "List of triggers"),
        (status = 500, description = "Persistence error")
    )
)]
pub fn list_triggers_path() {}

#[utoipa::path(
    post,
    path = "/api/v1/triggers",
    responses(
        (status = 201, description = "Trigger created"),
        (status = 400, description = "Validation error")
    )
)]
pub fn create_trigger_path() {}

#[utoipa::path(
    get,
    path = "/api/v1/triggers/{id}",
    params(
        ("id" = String, Path, description = "Trigger ID")
    ),
    responses(
        (status = 200, description = "Trigger found"),
        (status = 404, description = "Trigger not found"),
        (status = 400, description = "Invalid ID format")
    )
)]
pub fn get_trigger_path() {}

#[utoipa::path(
    put,
    path = "/api/v1/triggers/{id}",
    params(
        ("id" = String, Path, description = "Trigger ID")
    ),
    responses(
        (status = 200, description = "Trigger updated"),
        (status = 400, description = "Validation error or invalid request"),
        (status = 404, description = "Trigger not found"),
        (status = 409, description = "Version conflict")
    )
)]
pub fn update_trigger_path() {}

#[utoipa::path(
    delete,
    path = "/api/v1/triggers/{id}",
    params(
        ("id" = String, Path, description = "Trigger ID")
    ),
    responses(
        (status = 204, description = "Trigger deleted"),
        (status = 404, description = "Trigger not found"),
        (status = 400, description = "Invalid ID format")
    )
)]
pub fn delete_trigger_path() {}

#[allow(clippy::needless_for_each)]
#[derive(utoipa::OpenApi)]
#[openapi(
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
        super::handlers::jobs::create_job_handler,
        super::handlers::jobs::get_job_handler,
        super::handlers::jobs::list_jobs_handler,
        super::handlers::jobs::cancel_job_handler,
        super::handlers::jobs::restart_job_handler,
        super::handlers::jobs::get_job_log_handler,
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
        super::trigger_api::handlers::list_triggers_handler,
        super::trigger_api::handlers::create_trigger_handler,
        super::trigger_api::handlers::get_trigger_handler,
        super::trigger_api::handlers::update_trigger_handler,
        super::trigger_api::handlers::delete_trigger_handler,
    )
)]
pub struct ApiDoc;
