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
    paths(health_path, list_jobs_path)
)]
pub struct ApiDoc;
