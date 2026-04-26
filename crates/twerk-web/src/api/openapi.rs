#![allow(clippy::needless_for_each)]

use crate::api::error::ApiError;
use crate::api::handlers::jobs::{CreateJobQuery, WaitMode};
use crate::api::handlers::scheduled::CreateScheduledJobBody;
use crate::api::handlers::system::CreateUserBody;
use crate::api::openapi_types::{
    CreateJobResponse, HealthResponse, MessageResponse, StatusResponse, TriggerErrorResponse,
};
use crate::api::trigger_api::domain::{TriggerId, TriggerUpdateRequest, TriggerView};
use anyhow::Context;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
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
        super::handlers::system::get_node_handler,
        super::handlers::system::get_metrics_handler,
        super::handlers::system::create_user_handler,
        super::handlers::jobs::create::create_job_handler,
        super::handlers::jobs::read::list_jobs_handler,
        super::handlers::jobs::read::get_job_handler,
        super::handlers::jobs::mutation::cancel_job_handler,
        super::handlers::jobs::mutation::cancel_job_handler_post,
        super::handlers::jobs::mutation::restart_job_handler,
        super::handlers::jobs::read::get_job_log_handler,
        super::handlers::tasks::get_task_handler,
        super::handlers::tasks::get_task_log_handler,
        super::handlers::scheduled::create::create_scheduled_job_handler,
        super::handlers::scheduled::read::list_scheduled_jobs_handler,
        super::handlers::scheduled::read::get_scheduled_job_handler,
        super::handlers::scheduled::lifecycle::pause_scheduled_job_handler,
        super::handlers::scheduled::lifecycle::resume_scheduled_job_handler,
        super::handlers::scheduled::lifecycle::delete_scheduled_job_handler,
        super::handlers::queues::list_queues_handler,
        super::handlers::queues::get_queue_handler,
        super::handlers::queues::delete_queue_handler,
        super::trigger_api::handlers::query::list_triggers_handler,
        super::trigger_api::handlers::command::create_trigger_handler,
        super::trigger_api::handlers::query::get_trigger_handler,
        super::trigger_api::handlers::command::update_trigger_handler,
        super::trigger_api::handlers::query::delete_trigger_handler,
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
        CreateJobResponse,
        ApiError,
        HealthResponse,
        MessageResponse,
        StatusResponse,
        TriggerErrorResponse,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteSpec {
    pub path: &'static str,
    pub methods: &'static [&'static str],
}

pub const ROUTE_SPECS: &[RouteSpec] = &[
    RouteSpec {
        path: "/health",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/tasks/{id}",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/tasks/{id}/log",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/jobs",
        methods: &["GET", "POST"],
    },
    RouteSpec {
        path: "/jobs/{id}",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/jobs/{id}/log",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/jobs/{id}/cancel",
        methods: &["POST", "PUT"],
    },
    RouteSpec {
        path: "/jobs/{id}/restart",
        methods: &["PUT"],
    },
    RouteSpec {
        path: "/scheduled-jobs",
        methods: &["GET", "POST"],
    },
    RouteSpec {
        path: "/scheduled-jobs/{id}",
        methods: &["DELETE", "GET"],
    },
    RouteSpec {
        path: "/scheduled-jobs/{id}/pause",
        methods: &["PUT"],
    },
    RouteSpec {
        path: "/scheduled-jobs/{id}/resume",
        methods: &["PUT"],
    },
    RouteSpec {
        path: "/queues",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/queues/{name}",
        methods: &["DELETE", "GET"],
    },
    RouteSpec {
        path: "/nodes",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/nodes/{id}",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/metrics",
        methods: &["GET"],
    },
    RouteSpec {
        path: "/users",
        methods: &["POST"],
    },
    RouteSpec {
        path: "/api/v1/triggers",
        methods: &["GET", "POST"],
    },
    RouteSpec {
        path: "/api/v1/triggers/{id}",
        methods: &["DELETE", "GET", "PUT"],
    },
];

#[must_use]
pub fn generate_spec() -> utoipa::openapi::OpenApi {
    let base = ApiDoc::openapi();
    match serde_json::to_value(&base) {
        Ok(mut value) => {
            let _ = value
                .pointer_mut("/paths/~1jobs~1{id}~1cancel/post")
                .and_then(serde_json::Value::as_object_mut)
                .map(|operation| {
                    operation.insert("deprecated".to_string(), serde_json::Value::Bool(true))
                });
            match serde_json::from_value(value) {
                Ok(spec) => spec,
                Err(_) => base,
            }
        }
        Err(_) => base,
    }
}

/// Serialize the generated `OpenAPI` document to pretty JSON.
///
/// # Errors
/// Returns an error when the generated `OpenAPI` document cannot be serialized.
pub fn generate_json() -> anyhow::Result<String> {
    serde_json::to_string_pretty(&generate_spec()).context("failed to serialize OpenAPI JSON")
}

/// Serialize the generated `OpenAPI` document to YAML.
///
/// # Errors
/// Returns an error when the generated `OpenAPI` document cannot be serialized.
pub fn generate_yaml() -> anyhow::Result<String> {
    serde_yaml::to_string(&generate_spec()).context("failed to serialize OpenAPI YAML")
}

#[must_use]
pub fn mounted_route_specs() -> BTreeMap<String, BTreeSet<String>> {
    ROUTE_SPECS
        .iter()
        .map(|spec| {
            (
                spec.path.to_string(),
                spec.methods
                    .iter()
                    .map(|method| (*method).to_string())
                    .collect(),
            )
        })
        .collect()
}

#[must_use]
pub fn documented_route_specs() -> BTreeMap<String, BTreeSet<String>> {
    serde_json::to_value(generate_spec())
        .ok()
        .and_then(|value| value.get("paths").cloned())
        .and_then(|value| value.as_object().cloned())
        .map_or_else(BTreeMap::new, |paths| {
            paths
                .into_iter()
                .map(|(path, item)| {
                    let methods = item.as_object().map_or_else(BTreeSet::new, |entry| {
                        ["delete", "get", "post", "put"]
                            .into_iter()
                            .filter(|method| entry.contains_key(*method))
                            .map(str::to_uppercase)
                            .collect()
                    });
                    (path, methods)
                })
                .collect()
        })
}

fn tracked_paths(root: &Path) -> [PathBuf; 3] {
    [
        root.join("docs/openapi.yaml"),
        root.join("docs/openapi.json"),
        root.join("crates/twerk-web/openapi.json"),
    ]
}

/// Write the tracked `OpenAPI` artifacts into the repository.
///
/// # Errors
/// Returns an error when the spec cannot be serialized, a target directory cannot be created,
/// or any tracked artifact cannot be written.
pub fn sync_tracked_artifacts(root: &Path) -> anyhow::Result<()> {
    let [yaml_path, docs_json_path, web_json_path] = tracked_paths(root);
    let json = generate_json()?;
    let yaml = generate_yaml()?;

    tracked_paths(root)
        .into_iter()
        .filter_map(|path| path.parent().map(Path::to_path_buf))
        .try_for_each(|dir| {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("failed to create {}", dir.display()))
        })?;

    std::fs::write(&yaml_path, yaml)
        .with_context(|| format!("failed to write {}", yaml_path.display()))?;
    std::fs::write(&docs_json_path, &json)
        .with_context(|| format!("failed to write {}", docs_json_path.display()))?;
    std::fs::write(&web_json_path, json)
        .with_context(|| format!("failed to write {}", web_json_path.display()))?;

    Ok(())
}
