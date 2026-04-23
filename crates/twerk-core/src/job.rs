use crate::id::{JobId, ScheduledJobId};
use crate::task::{AutoDelete, Permission, Task, TaskLimits, TaskRetry};
use crate::user::User;
use crate::webhook::Webhook;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use time::OffsetDateTime;
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// JobState enum
// ---------------------------------------------------------------------------

/// State machine for a [`Job`].
///
/// Valid transitions:
/// - `Pending` -> `Scheduled`
/// - `Scheduled` -> `Running`
/// - `Running` -> `Completed` | `Failed` | `Cancelled`
/// - `Failed` | `Cancelled` -> `Restart`
/// - `Restart` -> `Pending`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobState {
    #[default]
    Pending,
    Scheduled,
    Running,
    Cancelled,
    Completed,
    Failed,
    Restart,
}

impl fmt::Display for JobState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pending => "PENDING",
            Self::Scheduled => "SCHEDULED",
            Self::Running => "RUNNING",
            Self::Cancelled => "CANCELLED",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Restart => "RESTART",
        };
        f.write_str(s)
    }
}

/// Error returned when a string cannot be parsed as a [`JobState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseJobStateError(String);

impl fmt::Display for ParseJobStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown JobState: {}", self.0)
    }
}

impl std::error::Error for ParseJobStateError {}

impl FromStr for JobState {
    type Err = ParseJobStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "SCHEDULED" => Ok(Self::Scheduled),
            "RUNNING" => Ok(Self::Running),
            "CANCELLED" => Ok(Self::Cancelled),
            "COMPLETED" => Ok(Self::Completed),
            "FAILED" => Ok(Self::Failed),
            "RESTART" => Ok(Self::Restart),
            other => Err(ParseJobStateError(other.to_owned())),
        }
    }
}

impl JobState {
    /// Returns `true` if the transition from `self` to `target` is valid.
    #[must_use]
    pub fn can_transition_to(&self, target: &JobState) -> bool {
        matches!(
            (self, target),
            (Self::Pending, Self::Scheduled)
                | (Self::Scheduled, Self::Running)
                | (
                    Self::Running,
                    Self::Completed | Self::Failed | Self::Cancelled
                )
                | (Self::Failed | Self::Cancelled, Self::Restart)
                | (Self::Restart, Self::Pending)
        )
    }

    /// Returns `true` if this state can be cancelled.
    #[must_use]
    pub fn can_cancel(&self) -> bool {
        matches!(self, Self::Pending | Self::Scheduled | Self::Running)
    }

    /// Returns `true` if this state can be restarted.
    #[must_use]
    pub fn can_restart(&self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled)
    }
}

// Backwards-compatible string constants (will be removed in migration bead).
pub const JOB_STATE_PENDING: &str = "PENDING";
pub const JOB_STATE_SCHEDULED: &str = "SCHEDULED";
pub const JOB_STATE_RUNNING: &str = "RUNNING";
pub const JOB_STATE_CANCELLED: &str = "CANCELLED";
pub const JOB_STATE_COMPLETED: &str = "COMPLETED";
pub const JOB_STATE_FAILED: &str = "FAILED";
pub const JOB_STATE_RESTART: &str = "RESTART";

/// Typed events emitted by the broker when a job changes state.
///
/// This replaces the raw `serde_json::Value` + callback pattern with a
/// typed stream that consumers can filter on directly.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JobEvent {
    /// A job transitioned to a new state.
    StateChanged {
        job_id: JobId,
        old_state: JobState,
        new_state: JobState,
    },
    /// A job completed successfully.
    Completed(Job),
    /// A job failed.
    Failed(Job),
    /// A job was cancelled.
    Cancelled(Job),
}

impl JobEvent {
    /// Returns the job ID associated with this event, if available.
    #[must_use]
    pub fn job_id(&self) -> Option<&JobId> {
        match self {
            JobEvent::StateChanged { job_id, .. } => Some(job_id),
            JobEvent::Completed(job) | JobEvent::Failed(job) | JobEvent::Cancelled(job) => {
                job.id.as_ref()
            }
        }
    }

    /// Returns the inner `Job` for terminal-state events (Completed, Failed, Cancelled).
    #[must_use]
    pub fn into_job(self) -> Option<Job> {
        match self {
            JobEvent::StateChanged { .. } => None,
            JobEvent::Completed(job) | JobEvent::Failed(job) | JobEvent::Cancelled(job) => {
                Some(job)
            }
        }
    }
}

/// Constructs a `JobEvent` from a job's state.
///
/// This is a convenience for publishers that already know the job state
/// and want to emit the correct typed variant.
#[must_use]
pub fn job_event_from_state(job: &Job) -> Option<JobEvent> {
    match job.state {
        JobState::Completed => Some(JobEvent::Completed(job.clone())),
        JobState::Failed => Some(JobEvent::Failed(job.clone())),
        JobState::Cancelled => Some(JobEvent::Cancelled(job.clone())),
        JobState::Pending | JobState::Scheduled | JobState::Running | JobState::Restart => None,
    }
}

/// State for a [`ScheduledJob`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScheduledJobState {
    #[default]
    Active,
    Paused,
}

impl fmt::Display for ScheduledJobState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
        };
        f.write_str(s)
    }
}

/// Error returned when a string cannot be parsed as a [`ScheduledJobState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseScheduledJobStateError(String);

impl fmt::Display for ParseScheduledJobStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown ScheduledJobState: {:?}", self.0)
    }
}

impl std::error::Error for ParseScheduledJobStateError {}

impl FromStr for ScheduledJobState {
    type Err = ParseScheduledJobStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ACTIVE" => Ok(Self::Active),
            "PAUSED" => Ok(Self::Paused),
            other => Err(ParseScheduledJobStateError(other.to_owned())),
        }
    }
}

// Backwards-compatible string constants (will be removed in migration bead).
pub const SCHEDULED_JOB_STATE_ACTIVE: &str = "ACTIVE";
pub const SCHEDULED_JOB_STATE_PAUSED: &str = "PAUSED";

/// Job represents a job in the system.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<JobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<JobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub state: JobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<Vec<Task>>,
    #[serde(default)]
    pub position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<JobContext>,
    #[serde(default)]
    pub task_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<JobDefaults>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<Webhook>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<Permission>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<AutoDelete>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(default)]
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<JobSchedule>,
}

impl Job {
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        self.clone()
    }
}

/// `ScheduledJob` represents a scheduled job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJob {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ScheduledJobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(default)]
    pub state: ScheduledJobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<JobDefaults>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<AutoDelete>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<Webhook>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<Permission>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// `JobSchedule` defines a job schedule.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobSchedule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ScheduledJobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
}

/// `JobSummary` provides a summary view of a job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<JobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<JobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub state: JobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub task_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<JobSchedule>,
}

/// `ScheduledJobSummary` provides a summary view of a scheduled job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJobSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ScheduledJobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(default)]
    pub state: ScheduledJobState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
}

/// `JobContext` holds contextual information for a job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<HashMap<String, String>>,
}

impl JobContext {
    #[must_use]
    pub fn as_map(&self) -> HashMap<String, serde_json::Value> {
        let mut m = HashMap::new();
        if let Some(ref inputs) = self.inputs {
            m.insert(
                String::from("inputs"),
                serde_json::to_value(inputs).map_or(serde_json::Value::Null, |v| v),
            );
        }
        if let Some(ref secrets) = self.secrets {
            m.insert(
                String::from("secrets"),
                serde_json::to_value(secrets).map_or(serde_json::Value::Null, |v| v),
            );
        }
        if let Some(ref tasks) = self.tasks {
            m.insert(
                String::from("tasks"),
                serde_json::to_value(tasks).map_or(serde_json::Value::Null, |v| v),
            );
        }
        if let Some(ref job) = self.job {
            m.insert(
                String::from("job"),
                serde_json::to_value(job).map_or(serde_json::Value::Null, |v| v),
            );
        }
        m
    }
}

/// `JobDefaults` defines default values for job tasks.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<TaskRetry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<TaskLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(default)]
    pub priority: i64,
}

#[must_use]
pub fn new_job_summary(j: &Job) -> JobSummary {
    JobSummary {
        id: j.id.clone(),
        created_by: j.created_by.clone(),
        parent_id: j.parent_id.clone(),
        name: j.name.clone(),
        description: j.description.clone(),
        tags: j.tags.clone(),
        inputs: j.inputs.clone(),
        state: j.state,
        created_at: j.created_at,
        started_at: j.started_at,
        completed_at: j.completed_at,
        failed_at: j.failed_at,
        position: j.position,
        task_count: j.task_count,
        result: j.result.clone(),
        error: j.error.clone(),
        progress: j.progress,
        schedule: j.schedule.clone(),
    }
}

#[must_use]
pub fn new_scheduled_job_summary(sj: &ScheduledJob) -> ScheduledJobSummary {
    ScheduledJobSummary {
        id: sj.id.clone(),
        created_by: sj.created_by.clone(),
        name: sj.name.clone(),
        state: sj.state,
        description: sj.description.clone(),
        tags: sj.tags.clone(),
        inputs: sj.inputs.clone(),
        cron: sj.cron.clone(),
        created_at: sj.created_at,
    }
}
