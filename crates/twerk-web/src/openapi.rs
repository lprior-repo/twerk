use utoipa::OpenApi;

/// OpenAPI spec for the Twerk API.
///
/// This struct aggregates all API components for OpenAPI spec generation.
#[derive(OpenApi)]
#[openapi(
   tags(
        (name = "System", description = "System health, metrics, and node information"),
        (name = "Tasks", description = "Task management and retrieval"),
        (name = "Jobs", description = "Job execution and lifecycle management"),
        (name = "ScheduledJobs", description = "Scheduled job configuration"),
        (name = "Triggers", description = "Event-driven trigger management"),
        (name = "Queues", description = "Queue operations and monitoring")
    ),
    paths(
        crate::api::handlers::system::health_handler,
        crate::api::handlers::system::list_nodes_handler,
        crate::api::handlers::system::get_metrics_handler,
        crate::api::handlers::system::create_user_handler,
        crate::api::handlers::tasks::get_task_handler,
        crate::api::handlers::tasks::get_task_log_handler,
        crate::api::handlers::jobs::list_jobs_handler,
        crate::api::handlers::jobs::create_job_handler,
        crate::api::handlers::jobs::get_job_handler,
        crate::api::handlers::jobs::get_job_log_handler,
        crate::api::handlers::jobs::cancel_job_handler,
        crate::api::handlers::jobs::restart_job_handler,
        crate::api::handlers::scheduled::list_scheduled_jobs_handler,
        crate::api::handlers::scheduled::create_scheduled_job_handler,
        crate::api::handlers::scheduled::get_scheduled_job_handler,
        crate::api::handlers::scheduled::pause_scheduled_job_handler,
        crate::api::handlers::scheduled::resume_scheduled_job_handler,
        crate::api::handlers::scheduled::delete_scheduled_job_handler,
        crate::api::handlers::queues::list_queues_handler,
        crate::api::handlers::queues::get_queue_handler,
        crate::api::handlers::queues::delete_queue_handler,
        crate::api::trigger_api::handlers::list_triggers_handler,
        crate::api::trigger_api::handlers::get_trigger_handler,
        crate::api::trigger_api::handlers::update_trigger_handler,
        crate::api::trigger_api::handlers::delete_trigger_handler,
        crate::api::trigger_api::handlers::create_trigger_handler,
    ),
    components(schemas(
        crate::api::domain::api::ServerAddress,
        crate::api::domain::api::ContentType,
        crate::api::domain::api::ApiFeature,
        crate::api::domain::api::FeatureFlags,
        crate::api::domain::auth::Username,
        crate::api::domain::auth::Password,
        crate::api::domain::pagination::Page,
        crate::api::domain::pagination::PageSize,
        crate::api::domain::search::SearchQuery,
        crate::api::handlers::system::CreateUserBody,
        crate::api::handlers::scheduled::CreateScheduledJobBody,
        crate::api::handlers::tasks::PaginationQuery,
        crate::api::handlers::tasks::RawPaginationQuery,
        crate::api::handlers::jobs::WaitMode,
        crate::api::trigger_api::domain::TriggerId,
        crate::api::trigger_api::domain::TriggerUpdateRequest,
        crate::api::trigger_api::domain::TriggerView,
    ))
)]
pub struct ApiOpenApi;

/// Generate the OpenAPI spec as JSON string.
pub fn create_openapi_spec(_version: &str) -> String {
    let openapi = ApiOpenApi::openapi();
    serde_json::to_string_pretty(&openapi).unwrap()
}
