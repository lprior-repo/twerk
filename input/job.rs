//! Job-related input types for Tork
//!
//! These types represent the input format for creating jobs and scheduled jobs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::task::{to_tasks, Limits, Retry, Task};
use tork::job::{
    Job as TorkJob, JobContext, JobDefaults as TorkJobDefaults, ScheduledJob as TorkScheduledJob,
    JOB_STATE_PENDING, SCHEDULED_JOB_STATE_ACTIVE,
};
use tork::task::Permission as TorkPermission;
use tork::{Role, User};

/// Job input type for creating a new job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Job {
    /// Job name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Job description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tags for the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Tasks to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,

    /// Input parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,

    /// Secrets (not serialized in all contexts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,

    /// Output configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,

    /// Default task configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<JobDefaults>,

    /// Webhooks to notify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<Webhook>>,

    /// Permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<Permission>>,

    /// Auto-delete configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<AutoDelete>,

    /// Wait configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<Wait>,
}

/// ScheduledJob input type for creating a scheduled job
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScheduledJob {
    /// Job name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Job description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tags for the job
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Tasks to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,

    /// Input parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,

    /// Secrets
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,

    /// Output configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,

    /// Default task configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<JobDefaults>,

    /// Webhooks to notify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<Webhook>>,

    /// Permissions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<Permission>>,

    /// Auto-delete configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<AutoDelete>,

    /// Schedule configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
}

/// Default configuration for job tasks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct JobDefaults {
    /// Retry configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<Retry>,

    /// Resource limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,

    /// Timeout duration string (e.g., "6h")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,

    /// Queue name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,

    /// Priority (0-9)
    #[serde(default)]
    pub priority: i64,
}

/// Schedule configuration for scheduled jobs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    /// Cron expression
    pub cron: String,
}

/// Auto-delete configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutoDelete {
    /// Duration after which to delete
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// Webhook configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Webhook {
    /// Webhook URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// HTTP headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,

    /// Event type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,

    /// Conditional expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,
}

/// Permission configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Permission {
    /// Username
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Role slug
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Wait configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wait {
    /// Timeout duration string
    pub timeout: String,
}

impl Job {
    /// Creates a new Job with the given name and tasks
    #[must_use]
    pub fn new(name: impl Into<String>, tasks: Vec<Task>) -> Self {
        Self {
            name: Some(name.into()),
            tasks: Some(tasks),
            ..Default::default()
        }
    }

    /// Returns true if the job has no tasks
    #[must_use]
    pub fn is_taskless(&self) -> bool {
        self.tasks.as_ref().is_none_or(|t| t.is_empty())
    }

    /// Returns the task count
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.as_ref().map_or(0, |t| t.len())
    }
}

impl ScheduledJob {
    /// Creates a new ScheduledJob with the given name, schedule, and tasks
    #[must_use]
    pub fn new(name: impl Into<String>, schedule: Schedule, tasks: Vec<Task>) -> Self {
        Self {
            name: Some(name.into()),
            schedule: Some(schedule),
            tasks: Some(tasks),
            ..Default::default()
        }
    }
}

impl Job {
    /// Convert to domain Job type
    #[must_use]
    pub fn to_job(&self) -> TorkJob {
        let now = time::OffsetDateTime::now_utc();
        let id = Uuid::new_v4().to_string().replace('-', "");

        let tasks = to_tasks(self.tasks.as_deref()).unwrap_or_default();

        let job_context = {
            let mut job_map = HashMap::new();
            job_map.insert("id".to_string(), id.clone());
            if let Some(name) = &self.name {
                job_map.insert("name".to_string(), name.clone());
            }
            Some(job_map)
        };

        let mut job = TorkJob::default();
        job.id = Some(id);
        job.description = self.description.clone();
        job.inputs = self.inputs.clone();
        job.secrets = self.secrets.clone();
        job.tags = self.tags.clone();
        job.name = self.name.clone();
        job.tasks = tasks.clone();
        job.state = JOB_STATE_PENDING.to_string();
        job.created_at = now;
        job.context = JobContext {
            job: job_context,
            inputs: self.inputs.clone(),
            secrets: self.secrets.clone(),
            tasks: None,
        };
        job.task_count = tasks.len() as i64;
        job.output = self.output.clone();
        job.defaults = self.defaults.as_ref().map(|d| d.to_tork());
        job.webhooks = self
            .webhooks
            .as_ref()
            .map(|whs| whs.iter().map(|wh| wh.to_tork()).collect());
        job.permissions = self
            .permissions
            .as_ref()
            .map(|ps| ps.iter().map(|p| p.to_tork()).collect());
        job.auto_delete = self.auto_delete.as_ref().map(|ad| ad.to_tork());

        job
    }
}

impl ScheduledJob {
    /// Convert to domain ScheduledJob type
    #[must_use]
    pub fn to_scheduled_job(&self) -> TorkScheduledJob {
        let now = time::OffsetDateTime::now_utc();
        let id = Uuid::new_v4().to_string().replace('-', "");

        let tasks = to_tasks(self.tasks.as_deref()).unwrap_or_default();

        let mut job = TorkScheduledJob::default();
        job.id = Some(id);
        job.description = self.description.clone();
        job.inputs = self.inputs.clone();
        job.secrets = self.secrets.clone();
        job.tags = self.tags.clone();
        job.name = self.name.clone();
        job.tasks = tasks;
        job.state = SCHEDULED_JOB_STATE_ACTIVE.to_string();
        job.created_at = now;
        job.output = self.output.clone();
        job.defaults = self.defaults.as_ref().map(|d| d.to_tork());
        job.webhooks = self
            .webhooks
            .as_ref()
            .map(|whs| whs.iter().map(|wh| wh.to_tork()).collect());
        job.permissions = self
            .permissions
            .as_ref()
            .map(|ps| ps.iter().map(|p| p.to_tork()).collect());
        job.auto_delete = self.auto_delete.as_ref().map(|ad| ad.to_tork());
        job.cron = self.schedule.as_ref().map(|s| s.cron.clone());

        job
    }
}

impl JobDefaults {
    /// Convert to domain JobDefaults type
    #[must_use]
    pub fn to_tork(&self) -> TorkJobDefaults {
        TorkJobDefaults {
            retry: self.retry.as_ref().map(|r| r.to_tork()),
            limits: self.limits.as_ref().map(|l| l.to_tork()),
            timeout: self.timeout.clone(),
            queue: self.queue.clone(),
            priority: self.priority,
        }
    }
}

impl Permission {
    /// Convert to domain Permission type
    #[must_use]
    pub fn to_tork(&self) -> TorkPermission {
        if let Some(role) = &self.role {
            TorkPermission {
                role: Some(Role {
                    id: None,
                    slug: Some(role.clone()),
                    name: None,
                    created_at: None,
                }),
                user: None,
            }
        } else {
            TorkPermission {
                role: None,
                user: Some(User {
                    id: None,
                    name: None,
                    username: self.user.clone(),
                    password_hash: None,
                    password: None,
                    created_at: None,
                    disabled: false,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_new() {
        let task = Task::new("test-task", "some:image");
        let job = Job::new("test-job", vec![task.clone()]);
        assert_eq!(job.name, Some("test-job".to_string()));
        assert_eq!(job.task_count(), 1);
    }

    #[test]
    fn test_job_default() {
        let job: Job = Default::default();
        assert!(job.name.is_none());
        assert!(job.is_taskless());
    }

    #[test]
    fn test_scheduled_job_new() {
        let task = Task::new("test-task", "some:image");
        let schedule = Schedule {
            cron: "0 * * * *".to_string(),
        };
        let job = ScheduledJob::new("scheduled-job", schedule.clone(), vec![task]);
        assert_eq!(job.name, Some("scheduled-job".to_string()));
        assert!(job.schedule.is_some());
    }
}
