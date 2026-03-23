//! Job domain types for the tork runtime.
//!
//! This module contains core job scheduling and execution types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

// ============================================================================
// State Constants
// ============================================================================

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

// ============================================================================
// Supporting Types (inlined for self-contained domain types)
// ============================================================================

/// User represents a user in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip)]
    pub password_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub disabled: bool,
}

impl User {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            username: self.username.clone(),
            password_hash: self.password_hash.clone(),
            password: self.password.clone(),
            created_at: self.created_at,
            disabled: self.disabled,
        }
    }
}

/// Role represents a role in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<OffsetDateTime>,
}

impl Role {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            slug: self.slug.clone(),
            name: self.name.clone(),
            created_at: self.created_at,
        }
    }
}

/// TaskRetry defines retry configuration for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRetry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts: Option<i32>,
}

impl TaskRetry {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            limit: self.limit,
            attempts: self.attempts,
        }
    }
}

/// TaskLimits defines resource limits for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLimits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

impl TaskLimits {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            cpus: self.cpus.clone(),
            memory: self.memory.clone(),
        }
    }
}

/// Task represents a unit of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cmd: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoint: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Box<Registry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,
    #[serde(default)]
    pub redelivered: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidecars: Vec<Task>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mounts: Vec<Mount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<Box<TaskRetry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Box<TaskLimits>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel: Option<Box<ParallelTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub each: Option<Box<EachTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjob: Option<Box<SubJobTask>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<Box<Probe>>,
}

impl Task {
    #[must_use]
    pub fn clone(&self) -> Self {
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
            registry: self.registry.as_ref().map(|r| Box::new((**r).clone())),
            env: self.env.as_ref().map(HashMap::clone),
            files: self.files.as_ref().map(HashMap::clone),
            queue: self.queue.clone(),
            redelivered: self.redelivered,
            error: self.error.clone(),
            pre: clone_tasks(&self.pre),
            post: clone_tasks(&self.post),
            sidecars: clone_tasks(&self.sidecars),
            mounts: clone_mounts(&self.mounts),
            networks: self.networks.clone(),
            node_id: self.node_id.clone(),
            retry: self.retry.as_ref().map(|r| Box::new((**r).clone())),
            limits: self.limits.as_ref().map(|l| Box::new((**l).clone())),
            timeout: self.timeout.clone(),
            result: self.result.clone(),
            var: self.var.clone(),
            r#if: self.r#if.clone(),
            parallel: self.parallel.as_ref().map(|p| Box::new((**p).clone())),
            each: self.each.as_ref().map(|e| Box::new((**e).clone())),
            subjob: self.subjob.as_ref().map(|s| Box::new((**s).clone())),
            gpus: self.gpus.clone(),
            tags: self.tags.clone(),
            workdir: self.workdir.clone(),
            priority: self.priority,
            progress: self.progress,
            probe: self.probe.as_ref().map(|p| Box::new((**p).clone())),
        }
    }
}

/// Registry holds container registry credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl Registry {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

/// Mount represents a filesystem mount.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

impl Mount {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            target: self.target.clone(),
            read_only: self.read_only,
        }
    }
}

/// Probe defines health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

impl Probe {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            port: self.port,
            timeout: self.timeout.clone(),
        }
    }
}

/// ParallelTask defines parallel task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelTask {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub completions: i32,
}

impl ParallelTask {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            tasks: clone_tasks(&self.tasks),
            completions: self.completions,
        }
    }
}

/// EachTask defines foreach task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EachTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Box<Task>>,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub completions: i32,
    #[serde(default)]
    pub concurrency: i32,
    #[serde(default)]
    pub index: i32,
}

impl EachTask {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            var: self.var.clone(),
            list: self.list.clone(),
            task: self.task.as_ref().map(|t| Box::new((**t).clone())),
            size: self.size,
            completions: self.completions,
            concurrency: self.concurrency,
            index: self.index,
        }
    }
}

/// SubJobTask defines a subjob task configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubJobTask {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<Box<AutoDelete>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default)]
    pub detached: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub webhooks: Vec<Webhook>,
}

impl SubJobTask {
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            inputs: self.inputs.as_ref().map(HashMap::clone),
            secrets: self.secrets.as_ref().map(HashMap::clone),
            auto_delete: self.auto_delete.as_ref().map(|a| Box::new((**a).clone())),
            tasks: clone_tasks(&self.tasks),
            output: self.output.clone(),
            detached: self.detached,
            webhooks: clone_webhooks(&self.webhooks),
        }
    }
}

// ============================================================================
// Main Job Types
// ============================================================================

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
            tasks: clone_tasks(&self.tasks),
            execution: clone_tasks(&self.execution),
            position: self.position,
            inputs: self.inputs.as_ref().map(HashMap::clone),
            secrets: self.secrets.as_ref().map(HashMap::clone),
            context: self.context.as_ref().map(JobContext::clone),
            task_count: self.task_count,
            output: self.output.clone(),
            result: self.result.clone(),
            error: self.error.clone(),
            defaults: self.defaults.as_ref().map(|d| Box::new((**d).clone())),
            webhooks: clone_webhooks(&self.webhooks),
            permissions: clone_permissions(&self.permissions),
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
            tasks: clone_tasks(&self.tasks),
            inputs: self.inputs.as_ref().map(HashMap::clone),
            secrets: self.secrets.as_ref().map(HashMap::clone),
            output: self.output.clone(),
            defaults: self.defaults.as_ref().map(|d| Box::new((**d).clone())),
            webhooks: clone_webhooks(&self.webhooks),
            permissions: clone_permissions(&self.permissions),
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

// ============================================================================
// Clone Helper Functions
// ============================================================================

#[must_use]
pub fn clone_webhooks(webhooks: &[Webhook]) -> Vec<Webhook> {
    webhooks.iter().map(Webhook::clone).collect()
}

#[must_use]
pub fn clone_permissions(perms: &[Permission]) -> Vec<Permission> {
    perms.iter().map(Permission::clone).collect()
}

#[must_use]
pub fn clone_tasks(tasks: &[Task]) -> Vec<Task> {
    tasks.iter().map(Task::clone).collect()
}

#[must_use]
fn clone_mounts(mounts: &[Mount]) -> Vec<Mount> {
    mounts.iter().map(Mount::clone).collect()
}

// ============================================================================
// Constructor Functions
// ============================================================================

#[must_use]
pub fn new_job_summary(j: &Job) -> JobSummary {
    JobSummary {
        id: j.id.clone(),
        created_by: j.created_by.as_ref().map(|u| Box::new((**u).clone())),
        parent_id: j.parent_id.clone(),
        name: j.name.clone(),
        description: j.description.clone(),
        tags: j.tags.clone(),
        inputs: j.inputs.as_ref().map(HashMap::clone),
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
        schedule: j.schedule.as_ref().map(|s| Box::new((**s).clone())),
    }
}

#[must_use]
pub fn new_scheduled_job_summary(sj: &ScheduledJob) -> ScheduledJobSummary {
    ScheduledJobSummary {
        id: sj.id.clone(),
        created_by: sj.created_by.as_ref().map(|u| Box::new((**u).clone())),
        name: sj.name.clone(),
        state: sj.state.clone(),
        description: sj.description.clone(),
        tags: sj.tags.clone(),
        inputs: sj.inputs.as_ref().map(HashMap::clone),
        cron: sj.cron.clone(),
        created_at: sj.created_at,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_state_constants() {
        assert_eq!(JOB_STATE_PENDING, "PENDING");
        assert_eq!(JOB_STATE_SCHEDULED, "SCHEDULED");
        assert_eq!(JOB_STATE_RUNNING, "RUNNING");
        assert_eq!(JOB_STATE_CANCELLED, "CANCELLED");
        assert_eq!(JOB_STATE_COMPLETED, "COMPLETED");
        assert_eq!(JOB_STATE_FAILED, "FAILED");
        assert_eq!(JOB_STATE_RESTART, "RESTART");
    }

    #[test]
    fn test_scheduled_job_state_constants() {
        assert_eq!(SCHEDULED_JOB_STATE_ACTIVE, "ACTIVE");
        assert_eq!(SCHEDULED_JOB_STATE_PAUSED, "PAUSED");
    }

    #[test]
    fn test_job_clone() {
        let job = Job {
            id: Some(String::from("job-1")),
            parent_id: Some(String::from("parent-1")),
            name: Some(String::from("Test Job")),
            description: Some(String::from("A test job")),
            tags: Some(vec![String::from("tag1"), String::from("tag2")]),
            state: Some(JOB_STATE_RUNNING.to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            created_by: Some(Box::new(User {
                id: Some(String::from("user-1")),
                name: Some(String::from("Test User")),
                username: Some(String::from("testuser")),
                password_hash: None,
                password: None,
                created_at: None,
                disabled: false,
            })),
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            failed_at: None,
            tasks: vec![],
            execution: vec![],
            position: 1,
            inputs: Some(HashMap::from([(
                String::from("key"),
                String::from("value"),
            )])),
            context: None,
            task_count: Some(5),
            output: None,
            result: None,
            error: None,
            defaults: None,
            webhooks: vec![],
            permissions: vec![],
            auto_delete: Some(Box::new(AutoDelete {
                after: Some(String::from("1h")),
            })),
            delete_at: None,
            secrets: None,
            progress: Some(0.5),
            schedule: None,
        };

        let cloned = job.clone();
        assert_eq!(job.id, cloned.id);
        assert_eq!(job.parent_id, cloned.parent_id);
        assert_eq!(job.name, cloned.name);
        assert_eq!(job.state, cloned.state);
        assert!(cloned.created_by.is_some());
        assert!(cloned.auto_delete.is_some());
        assert_eq!(
            cloned.inputs.as_ref().unwrap().get("key"),
            Some(&String::from("value"))
        );
    }

    #[test]
    fn test_scheduled_job_clone() {
        let sj = ScheduledJob {
            id: Some(String::from("sj-1")),
            name: Some(String::from("Scheduled Job")),
            description: Some(String::from("A scheduled job")),
            cron: Some(String::from("0 * * * *")),
            state: Some(SCHEDULED_JOB_STATE_ACTIVE.to_string()),
            inputs: Some(HashMap::new()),
            tasks: vec![],
            created_by: None,
            defaults: None,
            auto_delete: None,
            webhooks: vec![],
            permissions: vec![],
            created_at: Some(OffsetDateTime::now_utc()),
            tags: Some(vec![String::from("scheduled")]),
            secrets: None,
            output: None,
        };

        let cloned = sj.clone();
        assert_eq!(sj.id, cloned.id);
        assert_eq!(sj.cron, cloned.cron);
        assert_eq!(sj.state, cloned.state);
    }

    #[test]
    fn test_job_schedule_clone() {
        let schedule = JobSchedule {
            id: Some(String::from("sched-1")),
            cron: Some(String::from("0 0 * * *")),
        };
        let cloned = schedule.clone();
        assert_eq!(schedule.id, cloned.id);
        assert_eq!(schedule.cron, cloned.cron);
    }

    #[test]
    fn test_webhook_clone() {
        let webhook = Webhook {
            url: Some(String::from("https://example.com/hook")),
            headers: Some(HashMap::from([(
                String::from("Authorization"),
                String::from("Bearer token"),
            )])),
            event: Some(String::from("job.completed")),
            r#if: Some(String::from("state == 'completed'")),
        };
        let cloned = webhook.clone();
        assert_eq!(webhook.url, cloned.url);
        assert_eq!(webhook.event, cloned.event);
        assert!(cloned.headers.is_some());
    }

    #[test]
    fn test_permission_clone_role() {
        let perm = Permission {
            role: Some(Box::new(Role {
                id: Some(String::from("role-1")),
                slug: Some(String::from("admin")),
                name: Some(String::from("Administrator")),
                created_at: None,
            })),
            user: None,
        };
        let cloned = perm.clone();
        assert!(cloned.role.is_some());
        assert!(cloned.user.is_none());
    }

    #[test]
    fn test_permission_clone_user() {
        let perm = Permission {
            role: None,
            user: Some(Box::new(User {
                id: Some(String::from("user-1")),
                name: Some(String::from("Test User")),
                username: Some(String::from("testuser")),
                password_hash: None,
                password: None,
                created_at: None,
                disabled: false,
            })),
        };
        let cloned = perm.clone();
        assert!(cloned.role.is_none());
        assert!(cloned.user.is_some());
    }

    #[test]
    fn test_auto_delete_clone() {
        let ad = AutoDelete {
            after: Some(String::from("24h")),
        };
        let cloned = ad.clone();
        assert_eq!(ad.after, cloned.after);
    }

    #[test]
    fn test_job_context_clone_and_as_map() {
        let ctx = JobContext {
            job: Some(HashMap::from([(
                String::from("key1"),
                String::from("val1"),
            )])),
            inputs: Some(HashMap::from([(
                String::from("input1"),
                String::from("val2"),
            )])),
            secrets: Some(HashMap::from([(
                String::from("secret1"),
                String::from("val3"),
            )])),
            tasks: Some(HashMap::from([(
                String::from("task1"),
                String::from("val4"),
            )])),
        };
        let cloned = ctx.clone();
        assert_eq!(
            cloned.inputs.as_ref().unwrap().get("input1"),
            Some(&String::from("val2"))
        );

        let map = ctx.as_map();
        assert!(map.contains_key("inputs"));
        assert!(map.contains_key("secrets"));
        assert!(map.contains_key("tasks"));
        assert!(map.contains_key("job"));
    }

    #[test]
    fn test_job_defaults_clone() {
        let defaults = JobDefaults {
            retry: Some(Box::new(TaskRetry {
                limit: Some(3),
                attempts: Some(1),
            })),
            limits: Some(Box::new(TaskLimits {
                cpus: Some(String::from("2")),
                memory: Some(String::from("1Gi")),
            })),
            timeout: Some(String::from("1h")),
            queue: Some(String::from("default")),
            priority: Some(10),
        };
        let cloned = defaults.clone();
        assert!(cloned.retry.is_some());
        assert!(cloned.limits.is_some());
        assert_eq!(cloned.timeout, Some(String::from("1h")));
        assert_eq!(cloned.queue, Some(String::from("default")));
        assert_eq!(cloned.priority, Some(10));
    }

    #[test]
    fn test_new_job_summary() {
        let job = Job {
            id: Some(String::from("job-1")),
            parent_id: Some(String::from("parent-1")),
            name: Some(String::from("Test Job")),
            description: Some(String::from("A test job")),
            tags: Some(vec![String::from("tag1")]),
            state: Some(JOB_STATE_COMPLETED.to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            created_by: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: Some(OffsetDateTime::now_utc()),
            failed_at: None,
            tasks: vec![],
            execution: vec![],
            position: 0,
            inputs: Some(HashMap::from([(
                String::from("key"),
                String::from("value"),
            )])),
            context: None,
            task_count: Some(10),
            output: None,
            result: Some(String::from("success")),
            error: None,
            defaults: None,
            webhooks: vec![],
            permissions: vec![],
            auto_delete: None,
            delete_at: None,
            secrets: None,
            progress: Some(1.0),
            schedule: None,
        };

        let summary = new_job_summary(&job);
        assert_eq!(summary.id, job.id);
        assert_eq!(summary.name, job.name);
        assert_eq!(summary.state, job.state);
        assert_eq!(summary.task_count, job.task_count);
        assert_eq!(summary.result, job.result);
    }

    #[test]
    fn test_new_scheduled_job_summary() {
        let sj = ScheduledJob {
            id: Some(String::from("sj-1")),
            name: Some(String::from("Scheduled Job")),
            description: Some(String::from("A scheduled job")),
            cron: Some(String::from("0 * * * *")),
            state: Some(SCHEDULED_JOB_STATE_ACTIVE.to_string()),
            inputs: Some(HashMap::from([(
                String::from("key"),
                String::from("value"),
            )])),
            tasks: vec![],
            created_by: None,
            defaults: None,
            auto_delete: None,
            webhooks: vec![],
            permissions: vec![],
            created_at: Some(OffsetDateTime::now_utc()),
            tags: Some(vec![String::from("scheduled")]),
            secrets: None,
            output: None,
        };

        let summary = new_scheduled_job_summary(&sj);
        assert_eq!(summary.id, sj.id);
        assert_eq!(summary.name, sj.name);
        assert_eq!(summary.cron, sj.cron);
        assert_eq!(summary.state, sj.state);
    }

    #[test]
    fn test_clone_webhooks() {
        let webhooks = vec![
            Webhook {
                url: Some(String::from("https://example.com/1")),
                headers: None,
                event: Some(String::from("start")),
                r#if: None,
            },
            Webhook {
                url: Some(String::from("https://example.com/2")),
                headers: None,
                event: Some(String::from("end")),
                r#if: None,
            },
        ];
        let cloned = clone_webhooks(&webhooks);
        assert_eq!(cloned.len(), 2);
        assert_eq!(cloned[0].url, Some(String::from("https://example.com/1")));
        assert_eq!(cloned[1].url, Some(String::from("https://example.com/2")));
    }

    #[test]
    fn test_clone_permissions() {
        let perms = vec![
            Permission {
                role: Some(Box::new(Role {
                    id: Some(String::from("r1")),
                    slug: Some(String::from("admin")),
                    name: Some(String::from("Admin")),
                    created_at: None,
                })),
                user: None,
            },
            Permission {
                role: None,
                user: Some(Box::new(User {
                    id: Some(String::from("u1")),
                    name: Some(String::from("User")),
                    username: Some(String::from("user")),
                    password_hash: None,
                    password: None,
                    created_at: None,
                    disabled: false,
                })),
            },
        ];
        let cloned = clone_permissions(&perms);
        assert_eq!(cloned.len(), 2);
        assert!(cloned[0].role.is_some());
        assert!(cloned[1].user.is_some());
    }

    #[test]
    fn test_job_serde() {
        let job = Job {
            id: Some(String::from("job-1")),
            parent_id: None,
            name: Some(String::from("Test Job")),
            description: None,
            tags: Some(vec![String::from("tag1")]),
            state: Some(JOB_STATE_PENDING.to_string()),
            created_at: Some(OffsetDateTime::now_utc()),
            created_by: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            tasks: vec![],
            execution: vec![],
            position: 0,
            inputs: None,
            context: None,
            task_count: None,
            output: None,
            result: None,
            error: None,
            defaults: None,
            webhooks: vec![],
            permissions: vec![],
            auto_delete: None,
            delete_at: None,
            secrets: None,
            progress: None,
            schedule: None,
        };

        let json = serde_json::to_string(&job).expect("serialization should succeed");
        assert!(json.contains("\"id\":\"job-1\""));
        assert!(json.contains("\"name\":\"Test Job\""));
        assert!(json.contains("\"state\":\"PENDING\""));
    }

    #[test]
    fn test_job_deser() {
        let json = r#"{"id":"job-1","name":"Test Job","state":"RUNNING","position":5}"#;
        let job: Job = serde_json::from_str(json).expect("deserialization should succeed");
        assert_eq!(job.id, Some(String::from("job-1")));
        assert_eq!(job.name, Some(String::from("Test Job")));
        assert_eq!(job.state, Some(String::from("RUNNING")));
        assert_eq!(job.position, 5);
    }

    #[test]
    fn test_task_clone() {
        let task = Task {
            id: Some(String::from("task-1")),
            job_id: Some(String::from("job-1")),
            parent_id: None,
            position: 0,
            name: Some(String::from("Test Task")),
            description: Some(String::from("A test task")),
            state: Some(String::from("PENDING")),
            created_at: Some(OffsetDateTime::now_utc()),
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            cmd: vec![String::from("echo"), String::from("hello")],
            entrypoint: vec![],
            run: None,
            image: Some(String::from("alpine")),
            registry: None,
            env: Some(HashMap::from([(String::from("FOO"), String::from("bar"))])),
            files: None,
            queue: None,
            redelivered: 0,
            error: None,
            pre: vec![],
            post: vec![],
            sidecars: vec![],
            mounts: vec![],
            networks: vec![],
            node_id: None,
            retry: Some(Box::new(TaskRetry {
                limit: Some(3),
                attempts: Some(0),
            })),
            limits: Some(Box::new(TaskLimits {
                cpus: Some(String::from("1")),
                memory: Some(String::from("512Mi")),
            })),
            timeout: None,
            result: None,
            var: None,
            r#if: None,
            parallel: None,
            each: None,
            subjob: None,
            gpus: None,
            tags: Some(vec![String::from("test")]),
            workdir: None,
            priority: 0,
            progress: None,
            probe: None,
        };

        let cloned = task.clone();
        assert_eq!(task.id, cloned.id);
        assert_eq!(task.name, cloned.name);
        assert_eq!(task.cmd, cloned.cmd);
        assert!(cloned.retry.is_some());
        assert!(cloned.limits.is_some());
        assert_eq!(
            cloned.env.as_ref().unwrap().get("FOO"),
            Some(&String::from("bar"))
        );
    }

    #[test]
    fn test_user_clone() {
        let user = User {
            id: Some(String::from("user-1")),
            name: Some(String::from("Test User")),
            username: Some(String::from("testuser")),
            password_hash: Some(String::from("hash")),
            password: Some(String::from("secret")),
            created_at: Some(OffsetDateTime::now_utc()),
            disabled: false,
        };

        let cloned = user.clone();
        assert_eq!(user.id, cloned.id);
        assert_eq!(user.username, cloned.username);
        assert_eq!(user.password_hash, cloned.password_hash);
    }

    #[test]
    fn test_role_clone() {
        let role = Role {
            id: Some(String::from("role-1")),
            slug: Some(String::from("admin")),
            name: Some(String::from("Administrator")),
            created_at: Some(OffsetDateTime::now_utc()),
        };

        let cloned = role.clone();
        assert_eq!(role.id, cloned.id);
        assert_eq!(role.slug, cloned.slug);
    }

    #[test]
    fn test_subjob_task_clone() {
        let subjob = SubJobTask {
            id: Some(String::from("subjob-1")),
            name: Some(String::from("SubJob")),
            description: Some(String::from("A subjob")),
            tasks: vec![],
            inputs: Some(HashMap::from([(
                String::from("key"),
                String::from("value"),
            )])),
            secrets: Some(HashMap::from([(
                String::from("SECRET"),
                String::from("shh"),
            )])),
            auto_delete: Some(Box::new(AutoDelete {
                after: Some(String::from("1h")),
            })),
            output: None,
            detached: true,
            webhooks: vec![],
        };

        let cloned = subjob.clone();
        assert_eq!(subjob.id, cloned.id);
        assert_eq!(subjob.detached, cloned.detached);
        assert!(cloned.auto_delete.is_some());
        assert!(cloned.inputs.is_some());
    }

    #[test]
    fn test_parallel_task_clone() {
        let parallel = ParallelTask {
            tasks: vec![Task {
                id: Some(String::from("task-1")),
                job_id: None,
                parent_id: None,
                position: 0,
                name: Some(String::from("Task 1")),
                description: None,
                state: None,
                created_at: None,
                scheduled_at: None,
                started_at: None,
                completed_at: None,
                failed_at: None,
                cmd: vec![],
                entrypoint: vec![],
                run: None,
                image: None,
                registry: None,
                env: None,
                files: None,
                queue: None,
                redelivered: 0,
                error: None,
                pre: vec![],
                post: vec![],
                sidecars: vec![],
                mounts: vec![],
                networks: vec![],
                node_id: None,
                retry: None,
                limits: None,
                timeout: None,
                result: None,
                var: None,
                r#if: None,
                parallel: None,
                each: None,
                subjob: None,
                gpus: None,
                tags: None,
                workdir: None,
                priority: 0,
                progress: None,
                probe: None,
            }],
            completions: 3,
        };

        let cloned = parallel.clone();
        assert_eq!(cloned.completions, 3);
        assert_eq!(cloned.tasks.len(), 1);
    }

    #[test]
    fn test_each_task_clone() {
        let each = EachTask {
            var: Some(String::from("item")),
            list: Some(String::from("{{inputs.items}}")),
            task: Some(Box::new(Task {
                id: Some(String::from("task-1")),
                job_id: None,
                parent_id: None,
                position: 0,
                name: Some(String::from("Loop Task")),
                description: None,
                state: None,
                created_at: None,
                scheduled_at: None,
                started_at: None,
                completed_at: None,
                failed_at: None,
                cmd: vec![String::from("echo"), String::from("{{item}}")],
                entrypoint: vec![],
                run: None,
                image: Some(String::from("alpine")),
                registry: None,
                env: None,
                files: None,
                queue: None,
                redelivered: 0,
                error: None,
                pre: vec![],
                post: vec![],
                sidecars: vec![],
                mounts: vec![],
                networks: vec![],
                node_id: None,
                retry: None,
                limits: None,
                timeout: None,
                result: None,
                var: None,
                r#if: None,
                parallel: None,
                each: None,
                subjob: None,
                gpus: None,
                tags: None,
                workdir: None,
                priority: 0,
                progress: None,
                probe: None,
            })),
            size: 10,
            completions: 5,
            concurrency: 2,
            index: 0,
        };

        let cloned = each.clone();
        assert_eq!(cloned.var, Some(String::from("item")));
        assert_eq!(cloned.size, 10);
        assert!(cloned.task.is_some());
    }

    #[test]
    fn test_registry_clone() {
        let reg = Registry {
            username: Some(String::from("user")),
            password: Some(String::from("pass")),
        };
        let cloned = reg.clone();
        assert_eq!(reg.username, cloned.username);
        assert_eq!(reg.password, cloned.password);
    }

    #[test]
    fn test_mount_clone() {
        let mount = Mount {
            source: Some(String::from("/data")),
            target: Some(String::from("/mnt")),
            read_only: Some(true),
        };
        let cloned = mount.clone();
        assert_eq!(mount.source, cloned.source);
        assert_eq!(mount.target, cloned.target);
        assert_eq!(mount.read_only, cloned.read_only);
    }

    #[test]
    fn test_probe_clone() {
        let probe = Probe {
            path: Some(String::from("/health")),
            port: 8080,
            timeout: Some(String::from("5s")),
        };
        let cloned = probe.clone();
        assert_eq!(probe.path, cloned.path);
        assert_eq!(probe.port, cloned.port);
    }

    #[test]
    fn test_task_retry_clone() {
        let retry = TaskRetry {
            limit: Some(3),
            attempts: Some(1),
        };
        let cloned = retry.clone();
        assert_eq!(retry.limit, cloned.limit);
        assert_eq!(retry.attempts, cloned.attempts);
    }

    #[test]
    fn test_task_limits_clone() {
        let limits = TaskLimits {
            cpus: Some(String::from("4")),
            memory: Some(String::from("8Gi")),
        };
        let cloned = limits.clone();
        assert_eq!(limits.cpus, cloned.cpus);
        assert_eq!(limits.memory, cloned.memory);
    }
}
