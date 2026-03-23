//! Main job types for scheduling and execution.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

use super::state::{JobState, ScheduledJobState};
use super::types::{
    AutoDelete, EachTask, JobDefaults, ParallelTask, Permission, Registry, Role, SubJobTask,
    Task, TaskLimits, TaskRetry, User, Webhook,
};

/// Job represents a job in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<JobState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<Box<User>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution: Vec<Task>,
    #[serde(default)]
    pub position: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<JobContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<Box<JobDefaults>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub webhooks: Vec<Webhook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<Permission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<Box<AutoDelete>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Box<JobSchedule>>,
}

impl Job {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            parent_id: self.parent_id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            state: self.state.clone(),
            created_at: self.created_at,
            created_by: self.created_by.as_ref().map(|u| Box::new((**u).clone())),
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            tasks: self.tasks.iter().map(Task::clone).collect(),
            execution: self.execution.iter().map(Task::clone).collect(),
            position: self.position,
            inputs: self.inputs.as_ref().map(HashMap::clone),
            secrets: self.secrets.as_ref().map(HashMap::clone),
            context: self.context.as_ref().map(JobContext::clone),
            task_count: self.task_count,
            output: self.output.clone(),
            result: self.result.clone(),
            error: self.error.clone(),
            defaults: self.defaults.as_ref().map(|d| Box::new((**d).clone())),
            webhooks: self.webhooks.iter().map(Webhook::clone).collect(),
            permissions: self.permissions.iter().map(Permission::clone).collect(),
            auto_delete: self.auto_delete.as_ref().map(|a| Box::new((**a).clone())),
            delete_at: self.delete_at,
            progress: self.progress,
            schedule: self.schedule.as_ref().map(|s| Box::new((**s).clone())),
        }
    }
}

/// ScheduledJob represents a scheduled job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJob {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ScheduledJobState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<Box<User>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<Box<JobDefaults>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<Box<AutoDelete>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub webhooks: Vec<Webhook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<Permission>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl ScheduledJob {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            cron: self.cron.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            created_at: self.created_at,
            created_by: self.created_by.as_ref().map(|u| Box::new((**u).clone())),
            tasks: self.tasks.iter().map(Task::clone).collect(),
            inputs: self.inputs.as_ref().map(HashMap::clone),
            secrets: self.secrets.as_ref().map(HashMap::clone),
            output: self.output.clone(),
            defaults: self.defaults.as_ref().map(|d| Box::new((**d).clone())),
            webhooks: self.webhooks.iter().map(Webhook::clone).collect(),
            permissions: self.permissions.iter().map(Permission::clone).collect(),
            auto_delete: self.auto_delete.as_ref().map(|a| Box::new((**a).clone())),
            state: self.state.clone(),
        }
    }
}

/// JobSchedule defines a job schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSchedule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
}

impl JobSchedule {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            cron: self.cron.clone(),
        }
    }
}

/// JobSummary provides a summary view of a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<Box<User>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<JobState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub position: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Box<JobSchedule>>,
}

/// ScheduledJobSummary provides a summary view of a scheduled job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledJobSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<Box<User>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ScheduledJobState>,
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

/// Permission defines access permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Box<Role>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<Box<User>>,
}

impl Permission {
    #[must_use]
    pub fn clone(&self) -> Self {
        let role = self.role.as_ref().map(|r| Box::new((**r).clone()));
        let user = role
            .as_ref()
            .is_none()
            .then(|| self.user.as_ref().map(|u| Box::new((**u).clone())))
            .flatten();
        Self { role, user }
    }
}

/// AutoDelete defines automatic deletion configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDelete {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

impl AutoDelete {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            after: self.after.clone(),
        }
    }
}

/// JobContext holds contextual information for a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.as_ref().map(HashMap::clone),
            secrets: self.secrets.as_ref().map(HashMap::clone),
            tasks: self.tasks.as_ref().map(HashMap::clone),
            job: self.job.as_ref().map(HashMap::clone),
        }
    }

    #[must_use]
    pub fn as_map(&self) -> HashMap<String, serde_json::Value> {
        let mut m = HashMap::new();
        if let Some(ref inputs) = self.inputs {
            m.insert(
                String::from("inputs"),
                serde_json::to_value(inputs).unwrap_or_default(),
            );
        }
        if let Some(ref secrets) = self.secrets {
            m.insert(
                String::from("secrets"),
                serde_json::to_value(secrets).unwrap_or_default(),
            );
        }
        if let Some(ref tasks) = self.tasks {
            m.insert(
                String::from("tasks"),
                serde_json::to_value(tasks).unwrap_or_default(),
            );
        }
        if let Some(ref job) = self.job {
            m.insert(
                String::from("job"),
                serde_json::to_value(job).unwrap_or_default(),
            );
        }
        m
    }
}

/// JobDefaults defines default values for job tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<Box<TaskRetry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Box<TaskLimits>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

impl JobDefaults {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            limits: self.limits.as_ref().map(|l| Box::new((**l).clone())),
            retry: self.retry.as_ref().map(|r| Box::new((**r).clone())),
            queue: self.queue.clone(),
            timeout: self.timeout.clone(),
            priority: self.priority,
        }
    }
}

/// Webhook defines a webhook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Webhook {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            headers: self.headers.as_ref().map(HashMap::clone),
            event: self.event.clone(),
            r#if: self.r#if.clone(),
        }
    }
}
