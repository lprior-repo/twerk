//! Task-related domain types and operations

use crate::mount::Mount;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use time::OffsetDateTime;

/// TaskState represents the state of a task
pub type TaskState = Cow<'static, str>;

/// Task has been created
pub const TASK_STATE_CREATED: TaskState = Cow::Borrowed("CREATED");
/// Task is pending execution
pub const TASK_STATE_PENDING: TaskState = Cow::Borrowed("PENDING");
/// Task has been scheduled
pub const TASK_STATE_SCHEDULED: TaskState = Cow::Borrowed("SCHEDULED");
/// Task is currently running
pub const TASK_STATE_RUNNING: TaskState = Cow::Borrowed("RUNNING");
/// Task has been cancelled
pub const TASK_STATE_CANCELLED: TaskState = Cow::Borrowed("CANCELLED");
/// Task has been stopped
pub const TASK_STATE_STOPPED: TaskState = Cow::Borrowed("STOPPED");
/// Task has completed successfully
pub const TASK_STATE_COMPLETED: TaskState = Cow::Borrowed("COMPLETED");
/// Task has failed
pub const TASK_STATE_FAILED: TaskState = Cow::Borrowed("FAILED");
/// Task has been skipped
pub const TASK_STATE_SKIPPED: TaskState = Cow::Borrowed("SKIPPED");

/// List of active task states
pub const TASK_STATE_ACTIVE: &[TaskState] = &[
    TASK_STATE_CREATED,
    TASK_STATE_PENDING,
    TASK_STATE_SCHEDULED,
    TASK_STATE_RUNNING,
];

/// Task is the basic unit of work that a Worker can handle
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique identifier
    pub id: Option<String>,
    /// Job ID this task belongs to
    pub job_id: Option<String>,
    /// Parent task ID (for sub-tasks)
    pub parent_id: Option<String>,
    /// Position in the job's task list
    #[serde(default)]
    pub position: i64,
    /// Task name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Task description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Current state
    #[serde(default)]
    pub state: TaskState,
    /// When the task was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    /// When the task was scheduled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<OffsetDateTime>,
    /// When the task started executing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    /// When the task completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    /// When the task failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<OffsetDateTime>,
    /// Command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
    /// Entrypoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    /// Run specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Docker image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Registry credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,
    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Files to mount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,
    /// Queue to execute on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    /// Number of times the task has been redelivered
    #[serde(default)]
    pub redelivered: i64,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Pre-requisite tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<Vec<Task>>,
    /// Post-requisite tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Vec<Task>>,
    /// Sidecar tasks
    #[serde(skip_serializing_if = "Option::is_none", rename = "sidecars")]
    pub sidecars: Option<Vec<Task>>,
    /// Mounts for this task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<Mount>>,
    /// Networks to attach to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<String>>,
    /// Node ID where the task is running
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    /// Retry configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<TaskRetry>,
    /// Resource limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<TaskLimits>,
    /// Timeout duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    /// Result output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Variable name for result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    /// Conditional execution expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,
    /// Parallel task configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel: Option<ParallelTask>,
    /// Each-task (loop) configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub each: Option<EachTask>,
    /// Subjob configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjob: Option<SubJobTask>,
    /// GPU specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,
    /// Tags for the task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Working directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    /// Priority (higher = more priority)
    #[serde(default)]
    pub priority: i64,
    /// Progress (0.0 to 1.0)
    #[serde(default)]
    pub progress: f64,
    /// Health probe configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<Probe>,
}

impl Task {
    /// Returns true if the task is in an active state
    #[must_use]
    pub fn is_active(&self) -> bool {
        TASK_STATE_ACTIVE.contains(&self.state)
    }

    /// Creates a deep clone of this task
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            job_id: self.job_id.clone(),
            parent_id: self.parent_id.clone(),
            position: self.position,
            name: self.name.clone(),
            description: self.description.clone(),
            state: self.state.clone(),
            created_at: self.created_at,
            scheduled_at: self.scheduled_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run.clone(),
            image: self.image.clone(),
            registry: self.registry.clone(),
            env: self
                .env
                .as_ref()
                .map(|e| e.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
            files: self
                .files
                .as_ref()
                .map(|f| f.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
            queue: self.queue.clone(),
            redelivered: self.redelivered,
            error: self.error.clone(),
            pre: self.pre.as_ref().map(|t| crate::task::clone_tasks(t)),
            post: self.post.as_ref().map(|t| crate::task::clone_tasks(t)),
            sidecars: self.sidecars.as_ref().map(|t| crate::task::clone_tasks(t)),
            mounts: self.mounts.as_ref().map(|m| crate::mount::clone_mounts(m)),
            networks: self.networks.clone(),
            node_id: self.node_id.clone(),
            retry: self.retry.clone(),
            limits: self.limits.clone(),
            timeout: self.timeout.clone(),
            result: self.result.clone(),
            var: self.var.clone(),
            r#if: self.r#if.clone(),
            parallel: self.parallel.clone(),
            each: self.each.clone(),
            subjob: self.subjob.clone(),
            gpus: self.gpus.clone(),
            tags: self.tags.clone(),
            workdir: self.workdir.clone(),
            priority: self.priority,
            progress: self.progress,
            probe: self.probe.clone(),
        }
    }
}

/// Creates a deep clone of a slice of tasks
#[must_use]
pub fn clone_tasks(tasks: &[Task]) -> Vec<Task> {
    tasks.to_vec()
}

/// TaskSummary is a condensed view of a task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: Option<String>,
    pub job_id: Option<String>,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// TaskLogPart represents a part of a task's log output
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLogPart {
    pub id: Option<String>,
    #[serde(default)]
    pub number: i64,
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

/// SubJobTask represents a sub-job task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SubJobTask {
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<AutoDelete>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default)]
    pub detached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<Webhook>>,
}

/// ParallelTask represents a task that runs subtasks in parallel
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParallelTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,
    #[serde(default)]
    pub completions: i64,
}

/// EachTask represents a for-each style loop task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EachTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Box<Task>>,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub completions: i64,
    #[serde(default)]
    pub concurrency: i64,
    #[serde(default)]
    pub index: i64,
}

/// TaskRetry holds retry configuration for a task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetry {
    #[serde(default)]
    pub limit: i64,
    #[serde(default)]
    pub attempts: i64,
}

/// TaskLimits holds resource limits for a task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// Registry holds Docker registry credentials
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Probe holds health check configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub port: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// Webhook holds webhook configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,
}

/// AutoDelete holds auto-deletion configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDelete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// Permission holds permission configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<crate::role::Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<crate::user::User>,
}

impl Permission {
    /// Creates a deep clone of this permission.
    ///
    /// Matches Go's `Permission.Clone()` which only clones the non-nil field:
    /// if `role` is set, `user` is cleared (and vice versa).
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        if self.role.is_some() {
            Self {
                role: self.role.as_ref().map(|r| r.deep_clone()),
                user: None,
            }
        } else {
            Self {
                role: None,
                user: self.user.as_ref().map(|u| u.deep_clone()),
            }
        }
    }
}

/// Creates a deep clone of a slice of webhooks
#[must_use]
pub fn clone_webhooks(webhooks: &[Webhook]) -> Vec<Webhook> {
    webhooks.to_vec()
}

/// Creates a deep clone of a slice of permissions
#[must_use]
pub fn clone_permissions(perms: &[Permission]) -> Vec<Permission> {
    perms.iter().map(|p| p.deep_clone()).collect()
}

/// Creates a new TaskSummary from a Task
#[must_use]
pub fn new_task_summary(t: &Task) -> TaskSummary {
    TaskSummary {
        id: t.id.clone(),
        job_id: t.job_id.clone(),
        position: t.position,
        progress: t.progress,
        name: t.name.clone(),
        description: t.description.clone(),
        state: t.state.clone(),
        created_at: t.created_at,
        scheduled_at: t.scheduled_at,
        started_at: t.started_at,
        completed_at: t.completed_at,
        error: t.error.clone(),
        result: t.result.clone(),
        var: t.var.clone(),
        tags: t.tags.clone(),
    }
}
