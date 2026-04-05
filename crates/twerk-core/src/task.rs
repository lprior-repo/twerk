use crate::id::{JobId, NodeId, TaskId};
use crate::mount::Mount;
use crate::role::Role;
use crate::user::User;
pub use crate::webhook::Webhook;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use time::OffsetDateTime;

/// `TaskState` represents the list of states that a task can be in at any given moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    #[default]
    Created,
    Pending,
    Scheduled,
    Running,
    Cancelled,
    Stopped,
    Completed,
    Failed,
    Skipped,
}

impl TaskState {
    /// Returns true if this state represents an active (in-progress) task.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            TaskState::Created | TaskState::Pending | TaskState::Scheduled | TaskState::Running
        )
    }

    /// Returns true if a transition from `self` to `target` is valid.
    ///
    /// Valid transitions:
    /// - Created -> Pending
    /// - Pending -> Scheduled
    /// - Scheduled -> Running
    /// - Running -> Completed | Failed | Cancelled | Stopped
    /// - Failed -> Pending (retry)
    /// - Any active state -> Skipped
    #[must_use]
    pub fn can_transition_to(&self, target: &TaskState) -> bool {
        match (self, target) {
            // Terminal states cannot transition out
            (
                TaskState::Completed
                | TaskState::Cancelled
                | TaskState::Stopped
                | TaskState::Skipped,
                _,
            ) => false,

            // Linear chain: Created->Pending->Scheduled->Running and Failed->Pending
            (s, t) if Self::is_valid_linear_transition(*s, *t) => true,

            // Running -> Completed | Failed | Cancelled | Stopped
            (
                TaskState::Running,
                TaskState::Completed
                | TaskState::Failed
                | TaskState::Cancelled
                | TaskState::Stopped,
            ) => true,

            // Any active state -> Skipped
            (s, TaskState::Skipped) if s.is_active() => true,

            _ => false,
        }
    }

    /// Check if transition follows the linear path: Created->Pending->Scheduled->Running or Failed->Pending
    fn is_valid_linear_transition(from: TaskState, to: TaskState) -> bool {
        matches!((from, to), (TaskState::Created, TaskState::Pending))
            || matches!((from, to), (TaskState::Pending, TaskState::Scheduled))
            || matches!((from, to), (TaskState::Scheduled, TaskState::Running))
            || matches!((from, to), (TaskState::Failed, TaskState::Pending))
    }
}

/// Returns true if the given state represents an active (in-progress) task.
///
/// This is a convenience function that delegates to [`TaskState::is_active`].
///
/// # Arguments
/// * `state` - The task state to check
///
/// # Examples
/// ```
/// use twerk_core::task::{is_task_state_active, TaskState};
///
/// assert!(is_task_state_active(TaskState::Running));
/// assert!(!is_task_state_active(TaskState::Completed));
/// ```
#[inline]
#[must_use]
pub fn is_task_state_active(state: TaskState) -> bool {
    state.is_active()
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskState::Created => write!(f, "CREATED"),
            TaskState::Pending => write!(f, "PENDING"),
            TaskState::Scheduled => write!(f, "SCHEDULED"),
            TaskState::Running => write!(f, "RUNNING"),
            TaskState::Cancelled => write!(f, "CANCELLED"),
            TaskState::Stopped => write!(f, "STOPPED"),
            TaskState::Completed => write!(f, "COMPLETED"),
            TaskState::Failed => write!(f, "FAILED"),
            TaskState::Skipped => write!(f, "SKIPPED"),
        }
    }
}

impl FromStr for TaskState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "CREATED" => Ok(TaskState::Created),
            "PENDING" => Ok(TaskState::Pending),
            "SCHEDULED" => Ok(TaskState::Scheduled),
            "RUNNING" => Ok(TaskState::Running),
            "CANCELLED" => Ok(TaskState::Cancelled),
            "STOPPED" => Ok(TaskState::Stopped),
            "COMPLETED" => Ok(TaskState::Completed),
            "FAILED" => Ok(TaskState::Failed),
            "SKIPPED" => Ok(TaskState::Skipped),
            _ => Err(format!("unknown task state: {s}")),
        }
    }
}

// Backwards-compatible string constants for migration.
pub const TASK_STATE_CREATED: &str = "CREATED";
pub const TASK_STATE_PENDING: &str = "PENDING";
pub const TASK_STATE_SCHEDULED: &str = "SCHEDULED";
pub const TASK_STATE_RUNNING: &str = "RUNNING";
pub const TASK_STATE_CANCELLED: &str = "CANCELLED";
pub const TASK_STATE_STOPPED: &str = "STOPPED";
pub const TASK_STATE_COMPLETED: &str = "COMPLETED";
pub const TASK_STATE_FAILED: &str = "FAILED";
pub const TASK_STATE_SKIPPED: &str = "SKIPPED";

pub const TASK_STATE_ACTIVE: &[&str] = &[
    TASK_STATE_CREATED,
    TASK_STATE_PENDING,
    TASK_STATE_SCHEDULED,
    TASK_STATE_RUNNING,
];

/// Task is the basic unit of work that a Worker can handle.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<TaskId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<JobId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<TaskId>,

    #[serde(default)]
    pub position: i64,

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
    pub failed_at: Option<OffsetDateTime>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,

    #[serde(default)]
    pub redelivered: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidecars: Option<Vec<Task>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<Mount>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<NodeId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<TaskRetry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<TaskLimits>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel: Option<ParallelTask>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub each: Option<Box<EachTask>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjob: Option<SubJobTask>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,

    #[serde(default)]
    pub priority: i64,

    #[serde(default)]
    pub progress: f64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<Probe>,
}

impl Task {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    #[must_use]
    pub fn deep_clone(&self) -> Self {
        let mut cloned = self.clone();
        cloned.redelivered = 0;
        cloned
    }
}

/// `TaskSummary` provides a summary view of a task.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<TaskId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<JobId>,

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

#[must_use]
pub fn new_task_summary(t: &Task) -> TaskSummary {
    TaskSummary {
        id: t.id.clone(),
        job_id: t.job_id.clone(),
        position: t.position,
        progress: t.progress,
        name: t.name.clone(),
        description: t.description.clone(),
        state: t.state,
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

/// `TaskLogPart` represents a part of a task's log output.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskLogPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(default)]
    pub number: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

/// `AutoDelete` defines automatic cleanup configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutoDelete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// `SubJobTask` represents a sub-job task configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubJobTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<JobId>,

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

/// `ParallelTask` represents a task that runs other tasks in parallel.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParallelTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,

    #[serde(default)]
    pub completions: i64,
}

/// `EachTask` represents a task that iterates over a list.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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

/// `TaskRetry` defines retry configuration for a task.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetry {
    #[serde(default)]
    pub limit: i64,

    #[serde(default)]
    pub attempts: i64,
}

/// `TaskLimits` defines resource limits for a task.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// Registry defines container registry credentials.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Probe defines health check configuration for a task.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(default)]
    pub port: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// Permission defines access permissions.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
}

#[must_use]
pub fn clone_tasks(tasks: &[Task]) -> Vec<Task> {
    tasks.to_vec()
}
