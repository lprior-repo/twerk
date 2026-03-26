use crate::id::{JobId, ScheduledJobId};
use crate::task::{AutoDelete, Permission, Task, TaskLimits, TaskRetry};
use crate::user::User;
use crate::webhook::Webhook;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

pub type JobState = String;

pub const JOB_STATE_PENDING: &str = "PENDING";
pub const JOB_STATE_SCHEDULED: &str = "SCHEDULED";
pub const JOB_STATE_RUNNING: &str = "RUNNING";
pub const JOB_STATE_CANCELLED: &str = "CANCELLED";
pub const JOB_STATE_COMPLETED: &str = "COMPLETED";
pub const JOB_STATE_FAILED: &str = "FAILED";
pub const JOB_STATE_RESTART: &str = "RESTART";

pub type ScheduledJobState = String;

pub const SCHEDULED_JOB_STATE_ACTIVE: &str = "ACTIVE";
pub const SCHEDULED_JOB_STATE_PAUSED: &str = "PAUSED";

/// Job represents a job in the system.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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

/// ScheduledJob represents a scheduled job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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

/// JobSchedule defines a job schedule.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JobSchedule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ScheduledJobId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
}

/// JobSummary provides a summary view of a job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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

/// ScheduledJobSummary provides a summary view of a scheduled job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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

/// JobContext holds contextual information for a job.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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
                serde_json::to_value(inputs).unwrap_or(serde_json::Value::Null),
            );
        }
        if let Some(ref secrets) = self.secrets {
            m.insert(
                String::from("secrets"),
                serde_json::to_value(secrets).unwrap_or(serde_json::Value::Null),
            );
        }
        if let Some(ref tasks) = self.tasks {
            m.insert(
                String::from("tasks"),
                serde_json::to_value(tasks).unwrap_or(serde_json::Value::Null),
            );
        }
        if let Some(ref job) = self.job {
            m.insert(
                String::from("job"),
                serde_json::to_value(job).unwrap_or(serde_json::Value::Null),
            );
        }
        m
    }
}

/// JobDefaults defines default values for job tasks.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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

#[must_use]
pub fn new_scheduled_job_summary(sj: &ScheduledJob) -> ScheduledJobSummary {
    ScheduledJobSummary {
        id: sj.id.clone(),
        created_by: sj.created_by.clone(),
        name: sj.name.clone(),
        state: sj.state.clone(),
        description: sj.description.clone(),
        tags: sj.tags.clone(),
        inputs: sj.inputs.clone(),
        cron: sj.cron.clone(),
        created_at: sj.created_at,
    }
}
