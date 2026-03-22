//! Job-related domain types and operations

use crate::task::{self, Task};
use crate::user::User;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

/// JobState represents the state of a job
pub type JobState = String;

/// Job is pending execution
pub const JOB_STATE_PENDING: &str = "PENDING";
/// Job has been scheduled
pub const JOB_STATE_SCHEDULED: &str = "SCHEDULED";
/// Job is currently running
pub const JOB_STATE_RUNNING: &str = "RUNNING";
/// Job has been cancelled
pub const JOB_STATE_CANCELLED: &str = "CANCELLED";
/// Job has completed successfully
pub const JOB_STATE_COMPLETED: &str = "COMPLETED";
/// Job has failed
pub const JOB_STATE_FAILED: &str = "FAILED";
/// Job is restarting
pub const JOB_STATE_RESTART: &str = "RESTART";

/// ScheduledJobState represents the state of a scheduled job
pub type ScheduledJobState = String;

/// Scheduled job is active
pub const SCHEDULED_JOB_STATE_ACTIVE: &str = "ACTIVE";
/// Scheduled job is paused
pub const SCHEDULED_JOB_STATE_PAUSED: &str = "PAUSED";

/// Job represents a job containing multiple tasks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    /// Unique identifier
    pub id: Option<String>,
    /// Parent job ID (for sub-jobs)
    pub parent_id: Option<String>,
    /// Job name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Job description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags for the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Current state
    #[serde(default)]
    pub state: JobState,
    /// When the job was created
    pub created_at: OffsetDateTime,
    /// User who created the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<User>,
    /// When the job started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    /// When the job completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    /// When the job failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<OffsetDateTime>,
    /// Tasks in the job
    #[serde(default)]
    pub tasks: Vec<Task>,
    /// Execution history (completed tasks)
    #[serde(default)]
    pub execution: Vec<Task>,
    /// Position in parent's task list
    #[serde(default)]
    pub position: i64,
    /// Input parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    /// Job context
    #[serde(default)]
    pub context: JobContext,
    /// Total task count
    #[serde(default)]
    pub task_count: i64,
    /// Output from the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Result from the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Default task configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<JobDefaults>,
    /// Webhooks to notify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<task::Webhook>>,
    /// Permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<task::Permission>>,
    /// Auto-delete configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<task::AutoDelete>,
    /// When to delete the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_at: Option<OffsetDateTime>,
    /// Secrets for the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    /// Progress (0.0 to 1.0)
    #[serde(default)]
    pub progress: f64,
    /// Schedule configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<JobSchedule>,
}

impl Default for Job {
    fn default() -> Self {
        Self {
            id: None,
            parent_id: None,
            name: None,
            description: None,
            tags: None,
            state: String::new(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            created_by: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            tasks: Vec::new(),
            execution: Vec::new(),
            position: 0,
            inputs: None,
            context: JobContext::default(),
            task_count: 0,
            output: None,
            result: None,
            error: None,
            defaults: None,
            webhooks: None,
            permissions: None,
            auto_delete: None,
            delete_at: None,
            secrets: None,
            progress: 0.0,
            schedule: None,
        }
    }
}

impl Job {
    /// Creates a clone of this job with deep copies of nested structures
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            parent_id: self.parent_id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            state: self.state.clone(),
            created_at: self.created_at,
            created_by: self.created_by.clone(),
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            tasks: task::clone_tasks(&self.tasks),
            execution: task::clone_tasks(&self.execution),
            position: self.position,
            inputs: self.inputs.clone(),
            context: self.context.clone(),
            task_count: self.task_count,
            output: self.output.clone(),
            result: self.result.clone(),
            error: self.error.clone(),
            defaults: self.defaults.clone(),
            webhooks: self.webhooks.as_ref().map(|v| task::clone_webhooks(v)),
            permissions: self
                .permissions
                .as_ref()
                .map(|v| task::clone_permissions(v)),
            auto_delete: self.auto_delete.clone(),
            delete_at: self.delete_at,
            secrets: self.secrets.clone(),
            progress: self.progress,
            schedule: self.schedule.clone(),
        }
    }
}

/// ScheduledJob represents a job scheduled for periodic execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJob {
    /// Unique identifier
    pub id: Option<String>,
    /// Job name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Job description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Cron expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    /// Current state
    #[serde(default)]
    pub state: ScheduledJobState,
    /// Input parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    /// Tasks in the job
    #[serde(default)]
    pub tasks: Vec<Task>,
    /// User who created the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<User>,
    /// Default task configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<JobDefaults>,
    /// Auto-delete configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<task::AutoDelete>,
    /// Webhooks to notify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<task::Webhook>>,
    /// Permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<task::Permission>>,
    /// When the job was created
    pub created_at: OffsetDateTime,
    /// Tags for the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Secrets for the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    /// Output from the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl Default for ScheduledJob {
    fn default() -> Self {
        Self {
            id: None,
            name: None,
            description: None,
            cron: None,
            state: String::new(),
            inputs: None,
            tasks: Vec::new(),
            created_by: None,
            defaults: None,
            auto_delete: None,
            webhooks: None,
            permissions: None,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            tags: None,
            secrets: None,
            output: None,
        }
    }
}

impl ScheduledJob {
    /// Creates a deep clone of this scheduled job
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            cron: self.cron.clone(),
            state: self.state.clone(),
            inputs: self.inputs.clone(),
            tasks: task::clone_tasks(&self.tasks),
            created_by: self.created_by.clone(),
            defaults: self.defaults.clone(),
            auto_delete: self.auto_delete.clone(),
            webhooks: self.webhooks.as_ref().map(|v| task::clone_webhooks(v)),
            permissions: self
                .permissions
                .as_ref()
                .map(|v| task::clone_permissions(v)),
            created_at: self.created_at,
            tags: self.tags.clone(),
            secrets: self.secrets.clone(),
            output: self.output.clone(),
        }
    }
}

/// JobSchedule holds schedule configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSchedule {
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
}

/// JobSummary is a condensed view of a job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<User>,
    pub parent_id: Option<String>,
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
    pub created_at: OffsetDateTime,
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

/// ScheduledJobSummary is a condensed view of a scheduled job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJobSummary {
    pub id: Option<String>,
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
    pub created_at: OffsetDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
}

/// JobContext holds context data for job execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    /// Converts the context to a map
    #[must_use]
    pub fn as_map(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        if let Some(job) = &self.job {
            map.insert(
                "job".to_string(),
                serde_json::to_value(job).unwrap_or_default(),
            );
        }
        if let Some(inputs) = &self.inputs {
            map.insert(
                "inputs".to_string(),
                serde_json::to_value(inputs).unwrap_or_default(),
            );
        }
        if let Some(secrets) = &self.secrets {
            map.insert(
                "secrets".to_string(),
                serde_json::to_value(secrets).unwrap_or_default(),
            );
        }
        if let Some(tasks) = &self.tasks {
            map.insert(
                "tasks".to_string(),
                serde_json::to_value(tasks).unwrap_or_default(),
            );
        }
        map
    }
}

/// JobDefaults holds default configuration for tasks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<task::TaskRetry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<task::TaskLimits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(default)]
    pub priority: i64,
}

/// Creates a new JobSummary from a Job
#[must_use]
pub fn new_job_summary(j: &Job) -> JobSummary {
    JobSummary {
        id: j.id.clone(),
        created_by: j.created_by.clone(),
        parent_id: j.parent_id.clone(),
        inputs: j.inputs.clone(),
        name: j.name.clone(),
        description: j.description.clone(),
        tags: j.tags.clone(),
        state: j.state.clone(),
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

/// Creates a new ScheduledJobSummary from a ScheduledJob
#[must_use]
pub fn new_scheduled_job_summary(sj: &ScheduledJob) -> ScheduledJobSummary {
    ScheduledJobSummary {
        id: sj.id.clone(),
        created_by: sj.created_by.clone(),
        inputs: sj.inputs.clone(),
        state: sj.state.clone(),
        name: sj.name.clone(),
        description: sj.description.clone(),
        tags: sj.tags.clone(),
        cron: sj.cron.clone(),
        created_at: sj.created_at,
    }
}
