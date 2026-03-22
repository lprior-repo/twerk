//! Database record types and conversions to domain types.

use sqlx::FromRow;
use std::collections::HashMap;
use time::OffsetDateTime;

use super::super::Error as DatastoreError;
use super::encrypt;
use tork::{
    job::{Job, JobContext, JobDefaults, JobSchedule, JobState,
        ScheduledJob, ScheduledJobState},
    task::{AutoDelete, Permission, Task, TaskLogPart, TaskState, Webhook},
    Node, NodeStatus,
    user::User,
    role::Role,
};

/// Task record from the database
#[derive(Debug, Clone, FromRow)]
pub struct TaskRecord {
    pub id: String,
    pub job_id: String,
    pub position: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub state: String,
    pub created_at: OffsetDateTime,
    pub scheduled_at: Option<OffsetDateTime>,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub failed_at: Option<OffsetDateTime>,
    pub cmd: Option<Vec<String>>,
    pub entrypoint: Option<Vec<String>>,
    pub run_script: Option<String>,
    pub image: Option<String>,
    pub registry: Option<Vec<u8>>,
    pub env: Option<Vec<u8>>,
    pub files_: Option<Vec<u8>>,
    pub queue: Option<String>,
    pub error_: Option<String>,
    pub pre_tasks: Option<Vec<u8>>,
    pub post_tasks: Option<Vec<u8>>,
    pub sidecars: Option<Vec<u8>>,
    pub mounts: Option<Vec<u8>>,
    pub networks: Option<Vec<String>>,
    pub node_id: Option<String>,
    pub retry: Option<Vec<u8>>,
    pub limits: Option<Vec<u8>>,
    pub timeout: Option<String>,
    pub var: Option<String>,
    pub result: Option<String>,
    pub parallel: Option<Vec<u8>>,
    pub parent_id: Option<String>,
    pub each_: Option<Vec<u8>>,
    pub subjob: Option<Vec<u8>>,
    pub subjob_id: Option<String>,
    pub gpus: Option<String>,
    pub if_: Option<String>,
    pub tags: Option<Vec<String>>,
    pub priority: Option<i64>,
    pub workdir: Option<String>,
    pub progress: Option<f64>,
}

impl TaskRecord {
    /// Converts the database record to a Task domain object.
    pub fn to_task(&self) -> Result<Task, DatastoreError> {
        let env = self.env.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let files = self.files_.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let pre = self.pre_tasks.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let post = self.post_tasks.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let sidecars = self.sidecars.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let retry = self.retry.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let limits = self.limits.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let parallel = self.parallel.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let each = self.each_.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let subjob = self.subjob.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let registry = self.registry.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let mounts = self.mounts.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        Ok(Task {
            id: Some(self.id.clone()),
            job_id: Some(self.job_id.clone()),
            parent_id: self.parent_id.clone(),
            position: self.position,
            name: self.name.clone(),
            description: self.description.clone(),
            state: str_to_task_state(&self.state),
            created_at: Some(self.created_at),
            scheduled_at: self.scheduled_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run_script.clone(),
            image: self.image.clone(),
            registry,
            env,
            files,
            queue: self.queue.clone(),
            redelivered: 0,
            error: self.error_.clone(),
            pre,
            post,
            sidecars,
            mounts,
            networks: self.networks.clone(),
            node_id: self.node_id.clone(),
            retry,
            limits,
            timeout: self.timeout.clone(),
            result: self.result.clone(),
            var: self.var.clone(),
            r#if: self.if_.clone(),
            parallel,
            each,
            subjob,
            gpus: self.gpus.clone(),
            tags: self.tags.clone(),
            workdir: self.workdir.clone(),
            priority: self.priority.unwrap_or(0),
            progress: self.progress.unwrap_or(0.0),
            probe: None,
        })
    }
}

/// Job record from the database
#[derive(Debug, Clone, FromRow)]
pub struct JobRecord {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub state: String,
    pub created_at: OffsetDateTime,
    pub created_by: String,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub failed_at: Option<OffsetDateTime>,
    pub delete_at: Option<OffsetDateTime>,
    pub tasks: Vec<u8>,
    pub position: i64,
    pub inputs: Vec<u8>,
    pub context: Vec<u8>,
    pub parent_id: Option<String>,
    pub task_count: i64,
    pub output_: Option<String>,
    pub result: Option<String>,
    pub error_: Option<String>,
    pub ts: Option<String>,
    pub defaults: Option<Vec<u8>>,
    pub webhooks: Option<Vec<u8>>,
    pub auto_delete: Option<Vec<u8>>,
    pub secrets: Option<Vec<u8>>,
    pub progress: Option<f64>,
    pub scheduled_job_id: Option<String>,
}

impl JobRecord {
    /// Converts the database record to a Job domain object.
    pub fn to_job(
        &self,
        tasks: Vec<Task>,
        execution: Vec<Task>,
        created_by: User,
        perms: Vec<Permission>,
        encryption_key: Option<&str>,
    ) -> Result<Job, DatastoreError> {
        let context: JobContext = serde_json::from_slice(&self.context)
            .map_err(|e| DatastoreError::Serialization(format!("job.context: {}", e)))?;

        let inputs: HashMap<String, String> = serde_json::from_slice(&self.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("job.inputs: {}", e)))?;

        let defaults: Option<JobDefaults> = self.defaults.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let auto_delete: Option<AutoDelete> = self.auto_delete.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let webhooks: Vec<Webhook> = self.webhooks.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or_default();

        let mut secrets: HashMap<String, String> = self.secrets.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or_default();

        if !secrets.is_empty() {
            secrets = encrypt::decrypt_secrets(&secrets, encryption_key)?;
        }

        let schedule = self.scheduled_job_id.as_ref()
            .map(|id| JobSchedule {
                id: Some(id.clone()),
                cron: None,
            });

        Ok(Job {
            id: Some(self.id.clone()),
            parent_id: self.parent_id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            state: JobState::from(self.state.as_str()),
            created_at: self.created_at,
            created_by: Some(created_by),
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            tasks,
            execution,
            position: self.position,
            inputs: Some(inputs),
            context,
            task_count: self.task_count,
            output: self.output_.clone(),
            result: self.result.clone(),
            error: self.error_.clone(),
            defaults,
            webhooks: if webhooks.is_empty() { None } else { Some(webhooks) },
            permissions: if perms.is_empty() { None } else { Some(perms) },
            auto_delete,
            delete_at: self.delete_at,
            secrets: if secrets.is_empty() { None } else { Some(secrets) },
            progress: self.progress.unwrap_or(0.0),
            schedule,
        })
    }
}

/// Scheduled job record from the database
#[derive(Debug, Clone, FromRow)]
pub struct ScheduledJobRecord {
    pub id: String,
    pub cron_expr: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub state: String,
    pub created_at: OffsetDateTime,
    pub created_by: String,
    pub tasks: Vec<u8>,
    pub inputs: Vec<u8>,
    pub output_: Option<String>,
    pub defaults: Option<Vec<u8>>,
    pub webhooks: Option<Vec<u8>>,
    pub auto_delete: Option<Vec<u8>>,
    pub secrets: Option<Vec<u8>>,
}

impl ScheduledJobRecord {
    /// Converts the database record to a ScheduledJob domain object.
    pub fn to_scheduled_job(
        &self,
        tasks: Vec<Task>,
        created_by: User,
        perms: Vec<Permission>,
        encryption_key: Option<&str>,
    ) -> Result<ScheduledJob, DatastoreError> {
        let inputs: HashMap<String, String> = serde_json::from_slice(&self.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.inputs: {}", e)))?;

        let defaults: Option<JobDefaults> = self.defaults.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let auto_delete: Option<AutoDelete> = self.auto_delete.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let webhooks: Vec<Webhook> = self.webhooks.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or_default();

        let mut secrets: HashMap<String, String> = self.secrets.as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or_default();

        if !secrets.is_empty() {
            secrets = encrypt::decrypt_secrets(&secrets, encryption_key)?;
        }

        Ok(ScheduledJob {
            id: Some(self.id.clone()),
            name: self.name.clone(),
            description: self.description.clone(),
            cron: self.cron_expr.clone(),
            state: ScheduledJobState::from(self.state.as_str()),
            inputs: Some(inputs),
            tasks,
            created_by: Some(created_by),
            defaults,
            auto_delete,
            webhooks: if webhooks.is_empty() { None } else { Some(webhooks) },
            permissions: if perms.is_empty() { None } else { Some(perms) },
            created_at: self.created_at,
            tags: self.tags.clone(),
            secrets: if secrets.is_empty() { None } else { Some(secrets) },
            output: self.output_.clone(),
        })
    }
}

/// Job permission record from the database
#[derive(Debug, Clone, FromRow)]
pub struct JobPermRecord {
    pub id: String,
    pub job_id: String,
    pub user_id: Option<String>,
    pub role_id: Option<String>,
    pub created_at: Option<OffsetDateTime>,
}

/// Scheduled job permission record from the database
#[derive(Debug, Clone, FromRow)]
pub struct ScheduledPermRecord {
    pub id: String,
    pub scheduled_job_id: String,
    pub user_id: Option<String>,
    pub role_id: Option<String>,
    pub created_at: Option<OffsetDateTime>,
}

/// Node record from the database
#[derive(Debug, Clone, FromRow)]
pub struct NodeRecord {
    pub id: String,
    pub name: String,
    pub started_at: OffsetDateTime,
    pub last_heartbeat_at: OffsetDateTime,
    pub cpu_percent: f64,
    pub queue: String,
    pub status: String,
    pub hostname: String,
    pub port: i64,
    pub task_count: i64,
    pub version_: String,
}

impl NodeRecord {
    /// Converts the database record to a Node domain object.
    pub fn to_node(&self) -> Node {
        let now = time::OffsetDateTime::now_utc();
        let heartbeat_timeout = now - time::Duration::seconds(2 * 30); // 2 * HEARTBEAT_RATE
        let status = if self.last_heartbeat_at < heartbeat_timeout && self.status == "UP" {
            NodeStatus::from("OFFLINE")
        } else {
            NodeStatus::from(self.status.as_str())
        };

        Node {
            id: Some(self.id.clone()),
            name: Some(self.name.clone()),
            started_at: self.started_at,
            cpu_percent: self.cpu_percent,
            last_heartbeat_at: self.last_heartbeat_at,
            queue: Some(self.queue.clone()),
            status,
            hostname: Some(self.hostname.clone()),
            port: self.port,
            task_count: self.task_count,
            version: self.version_.clone(),
        }
    }
}

/// Task log part record from the database
#[derive(Debug, Clone, FromRow)]
pub struct TaskLogPartRecord {
    pub id: String,
    pub number_: i64,
    pub task_id: String,
    pub created_at: OffsetDateTime,
    pub contents: String,
}

impl TaskLogPartRecord {
    /// Converts the database record to a TaskLogPart domain object.
    pub fn to_task_log_part(&self) -> TaskLogPart {
        TaskLogPart {
            id: Some(self.id.clone()),
            number: self.number_,
            task_id: Some(self.task_id.clone()),
            contents: Some(self.contents.clone()),
            created_at: Some(self.created_at),
        }
    }
}

/// User record from the database
#[derive(Debug, Clone, FromRow)]
pub struct UserRecord {
    pub id: String,
    pub name: String,
    pub username_: String,
    pub password_: String,
    pub created_at: OffsetDateTime,
    pub is_disabled: bool,
}

impl UserRecord {
    /// Converts the database record to a User domain object.
    pub fn to_user(&self) -> User {
        User {
            id: Some(self.id.clone()),
            name: Some(self.name.clone()),
            username: Some(self.username_.clone()),
            password_hash: Some(self.password_.clone()),
            password: None,
            created_at: Some(self.created_at),
            disabled: self.is_disabled,
        }
    }
}

/// Role record from the database
#[derive(Debug, Clone, FromRow)]
pub struct RoleRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub created_at: OffsetDateTime,
}

impl RoleRecord {
    /// Converts the database record to a Role domain object.
    pub fn to_role(&self) -> Role {
        Role {
            id: Some(self.id.clone()),
            slug: Some(self.slug.clone()),
            name: Some(self.name.clone()),
            created_at: Some(self.created_at),
        }
    }
}

/// Helper to convert a string slice to TaskState
fn str_to_task_state(s: &str) -> TaskState {
    TaskState::from(std::borrow::Cow::Owned(s.to_string()))
}
