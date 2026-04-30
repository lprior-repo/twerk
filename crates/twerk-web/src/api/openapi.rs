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
