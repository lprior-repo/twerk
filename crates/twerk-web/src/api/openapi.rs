#![allow(clippy::needless_for_each)]
use serde::Serialize;
use twerk_core::job::{Job, ScheduledJob};
use twerk_core::node::Node;
use twerk_core::stats::Metrics;
use twerk_core::task::Task;
use twerk_core::trigger::Trigger;
use twerk_core::user::User;
use twerk_infrastructure::broker::QueueInfo;

#[derive(Serialize, utoipa::ToSchema)]
#[schema(example = json!({"name": "example-job"}))]
pub struct JobSchema {
    #[serde(flatten)]
    pub inner: Job,
}

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

#[utoipa::path(get, path = "/health", responses((status = 200, description = "Health check successful")))]
pub fn health_path() {}

#[utoipa::path(get, path = "/nodes", responses((status = 200, description = "List of active nodes")))]
pub fn list_nodes_path() {}

#[utoipa::path(get, path = "/metrics", responses((status = 200, description = "System metrics")))]
pub fn get_metrics_path() {}

#[utoipa::path(post, path = "/users", responses((status = 200, description = "User created"), (status = 400, description = "Missing username or password")))]
pub fn create_user_path() {}

#[utoipa::path(get, path = "/tasks/{id}", params(("id" = String, description = "Task ID")), responses((status = 200, description = "Task details"), (status = 404, description = "Task not found")))]
pub fn get_task_path() {}

#[utoipa::path(get, path = "/tasks/{id}/log", params(("id" = String, description = "Task ID"), ("page" = Option<String>, Query, description = "Page number"), ("size" = Option<String>, Query, description = "Page size (max 100)"), ("q" = Option<String>, Query, description = "Search query")), responses((status = 200, description = "Paginated task log entries"), (status = 404, description = "Task not found")))]
pub fn get_task_log_path() {}

#[utoipa::path(post, path = "/jobs", params(("wait" = Option<String>, Query, description = "Whether to block until the job completes (true/false/blocking)")), request_body(content = String, description = "Job definition as JSON or YAML"), responses((status = 200, description = "Job created", body = Job), (status = 400, description = "Invalid job definition or unsupported content type")))]
pub fn create_job_path() {}

#[utoipa::path(get, path = "/jobs", params(("page" = Option<String>, Query, description = "Page number"), ("size" = Option<String>, Query, description = "Page size"), ("q" = Option<String>, Query, description = "Search query")), responses((status = 200, description = "List of jobs", body = Vec<Job>)))]
pub fn list_jobs_path() {}

#[utoipa::path(get, path = "/jobs/{id}", params(("id" = String, description = "The job ID")), responses((status = 200, description = "Job found", body = Job), (status = 404, description = "Job not found")))]
pub fn get_job_path() {}

#[utoipa::path(put, path = "/jobs/{id}/cancel", params(("id" = String, description = "The job ID")), responses((status = 200, description = "Job cancelled", body = Job), (status = 400, description = "Job cannot be cancelled in its current state")))]
pub fn cancel_job_path() {}

#[utoipa::path(put, path = "/jobs/{id}/restart", params(("id" = String, description = "The job ID")), responses((status = 200, description = "Job restarted", body = Job), (status = 400, description = "Job cannot be restarted")))]
pub fn restart_job_path() {}

#[utoipa::path(get, path = "/jobs/{id}/log", params(("id" = String, description = "The job ID"), ("page" = Option<String>, Query, description = "Page number"), ("size" = Option<String>, Query, description = "Page size")), responses((status = 200, description = "Job log parts", body = Vec<String>), (status = 404, description = "Job not found")))]
pub fn get_job_log_path() {}

#[utoipa::path(post, path = "/scheduled-jobs", responses((status = 200, description = "Scheduled job created")))]
pub fn create_scheduled_job_path() {}

#[utoipa::path(get, path = "/scheduled-jobs", responses((status = 200, description = "List of scheduled jobs")))]
pub fn list_scheduled_jobs_path() {}

#[utoipa::path(get, path = "/scheduled-jobs/{id}", params(("id" = String, description = "Scheduled job ID")), responses((status = 200, description = "Scheduled job found")))]
pub fn get_scheduled_job_path() {}

#[utoipa::path(put, path = "/scheduled-jobs/{id}/pause", params(("id" = String, description = "Scheduled job ID")), responses((status = 200, description = "Scheduled job paused")))]
pub fn pause_scheduled_job_path() {}

#[utoipa::path(put, path = "/scheduled-jobs/{id}/resume", params(("id" = String, description = "Scheduled job ID")), responses((status = 200, description = "Scheduled job resumed")))]
pub fn resume_scheduled_job_path() {}

#[utoipa::path(delete, path = "/scheduled-jobs/{id}", params(("id" = String, description = "Scheduled job ID")), responses((status = 200, description = "Scheduled job deleted")))]
pub fn delete_scheduled_job_path() {}

#[utoipa::path(get, path = "/queues", responses((status = 200, description = "List of queues")))]
pub fn list_queues_path() {}

#[utoipa::path(get, path = "/queues/{name}", params(("name" = String, description = "Queue name")), responses((status = 200, description = "Queue info"), (status = 404, description = "Queue not found")))]
pub fn get_queue_path() {}

#[utoipa::path(delete, path = "/queues/{name}", params(("name" = String, description = "Queue name")), responses((status = 200, description = "Queue deleted")))]
pub fn delete_queue_path() {}

#[utoipa::path(get, path = "/api/v1/triggers", responses((status = 200, description = "List of triggers"), (status = 500, description = "Persistence error")))]
pub fn list_triggers_path() {}

#[utoipa::path(post, path = "/api/v1/triggers", responses((status = 201, description = "Trigger created"), (status = 400, description = "Validation error")))]
pub fn create_trigger_path() {}

#[utoipa::path(get, path = "/api/v1/triggers/{id}", params(("id" = String, description = "Trigger ID")), responses((status = 200, description = "Trigger found"), (status = 404, description = "Trigger not found"), (status = 400, description = "Invalid ID format")))]
pub fn get_trigger_path() {}

#[utoipa::path(put, path = "/api/v1/triggers/{id}", params(("id" = String, description = "Trigger ID")), responses((status = 200, description = "Trigger updated"), (status = 400, description = "Validation error or invalid request"), (status = 404, description = "Trigger not found"), (status = 409, description = "Version conflict")))]
pub fn update_trigger_path() {}

#[utoipa::path(delete, path = "/api/v1/triggers/{id}", params(("id" = String, description = "Trigger ID")), responses((status = 204, description = "Trigger deleted"), (status = 404, description = "Trigger not found"), (status = 400, description = "Invalid ID format")))]
pub fn delete_trigger_path() {}

#[allow(clippy::needless_for_each)]
#[derive(utoipa::OpenApi)]
#[openapi(
    components(schemas(
        JobSchema,
        TaskSchema,
        ScheduledJobSchema,
        TriggerSchema,
        UserSchema,
        QueueInfoSchema,
        NodeSchema,
        MetricsSchema,
        ApiErrorSchema
    )),
    paths(
        health_path,
        list_nodes_path,
        get_metrics_path,
        create_user_path,
        get_task_path,
        get_task_log_path,
        create_job_path,
        list_jobs_path,
        get_job_path,
        cancel_job_path,
        restart_job_path,
        get_job_log_path,
        create_scheduled_job_path,
        list_scheduled_jobs_path,
        get_scheduled_job_path,
        pause_scheduled_job_path,
        resume_scheduled_job_path,
        delete_scheduled_job_path,
        list_queues_path,
        get_queue_path,
        delete_queue_path,
        list_triggers_path,
        create_trigger_path,
        get_trigger_path,
        update_trigger_path,
        delete_trigger_path
    )
)]
pub struct ApiDoc;
