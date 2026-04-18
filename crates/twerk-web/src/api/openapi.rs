#![allow(clippy::needless_for_each)]
use crate::api::error::ApiError;
use crate::api::handlers::jobs::{CreateJobQuery, WaitMode};
use crate::api::handlers::scheduled::CreateScheduledJobBody;
use crate::api::handlers::system::CreateUserBody;
use crate::api::trigger_api::domain::{TriggerId, TriggerUpdateRequest, TriggerView};
use twerk_core::id::{JobId, NodeId, ScheduledJobId, TaskId, UserId};
use twerk_core::job::{
    Job, JobDefaults, JobState, JobSummary, ScheduledJob, ScheduledJobState, ScheduledJobSummary,
};
use twerk_core::mount::Mount;
use twerk_core::node::{Node, NodeStatus};
use twerk_core::role::Role;
use twerk_core::stats::Metrics;
use twerk_core::task::{AutoDelete, Permission, Probe, Registry};
use twerk_core::task::{Task, TaskLogPart, TaskState};
use twerk_core::trigger::Trigger;
use twerk_core::user::User;
use twerk_core::webhook::Webhook;
use twerk_infrastructure::broker::QueueInfo;
use twerk_infrastructure::datastore::Page;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        super::handlers::system::health_handler,
        super::handlers::system::list_nodes_handler,
        super::handlers::system::get_metrics_handler,
        super::handlers::system::create_user_handler,
        super::handlers::jobs::create_job_handler,
        super::handlers::jobs::list_jobs_handler,
        super::handlers::jobs::get_job_handler,
        super::handlers::jobs::cancel_job_handler,
        super::handlers::jobs::cancel_job_handler_post,
        super::handlers::jobs::restart_job_handler,
        super::handlers::jobs::get_job_log_handler,
        super::handlers::tasks::get_task_handler,
        super::handlers::tasks::get_task_log_handler,
        super::handlers::scheduled::create_scheduled_job_handler,
        super::handlers::scheduled::list_scheduled_jobs_handler,
        super::handlers::scheduled::get_scheduled_job_handler,
        super::handlers::scheduled::pause_scheduled_job_handler,
        super::handlers::scheduled::resume_scheduled_job_handler,
        super::handlers::scheduled::delete_scheduled_job_handler,
        super::handlers::queues::list_queues_handler,
        super::handlers::queues::get_queue_handler,
        super::handlers::queues::delete_queue_handler,
        super::trigger_api::handlers::list_triggers_handler,
        super::trigger_api::handlers::create_trigger_handler,
        super::trigger_api::handlers::get_trigger_handler,
        super::trigger_api::handlers::update_trigger_handler,
        super::trigger_api::handlers::delete_trigger_handler,
    ),
    components(schemas(
        Job,
        JobSummary,
        ScheduledJob,
        ScheduledJobSummary,
        JobState,
        ScheduledJobState,
        JobDefaults,
        Task,
        TaskLogPart,
        TaskState,
        Node,
        NodeStatus,
        Metrics,
        User,
        Trigger,
        TriggerView,
        TriggerUpdateRequest,
        TriggerId,
        JobId,
        TaskId,
        NodeId,
        UserId,
        ScheduledJobId,
        Permission,
        Mount,
        AutoDelete,
        Probe,
        Webhook,
        Registry,
        Role,
        QueueInfo,
        Page<JobSummary>,
        Page<ScheduledJobSummary>,
        Page<TaskLogPart>,
        ApiError,
        CreateUserBody,
        CreateScheduledJobBody,
        CreateJobQuery,
        WaitMode,
    )),
    info(
        title = "Twerk API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Task scheduling and job queue management API",
    )
)]
pub struct ApiDoc;
