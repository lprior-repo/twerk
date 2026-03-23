//! Database record types and conversions to domain types.

#![allow(clippy::missing_errors_doc)]

use sqlx::FromRow;
use std::collections::HashMap;
use time::OffsetDateTime;

use super::super::Error as DatastoreError;
use super::encrypt;
use tork::{
    job::{Job, JobContext, JobDefaults, JobSchedule, JobState, ScheduledJob, ScheduledJobState},
    role::Role,
    task::{AutoDelete, Permission, Task, TaskLogPart, TaskState, Webhook},
    user::User,
    Node, NodeStatus,
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
        let env = self
            .env
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let files = self
            .files_
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let pre = self
            .pre_tasks
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let post = self
            .post_tasks
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let sidecars = self
            .sidecars
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .flatten();

        let retry = self
            .retry
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let limits = self
            .limits
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let parallel = self
            .parallel
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let each = self
            .each_
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let subjob = self
            .subjob
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let registry = self
            .registry
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let mounts = self
            .mounts
            .as_ref()
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
            .map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;

        let inputs: HashMap<String, String> = serde_json::from_slice(&self.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("job.inputs: {e}")))?;

        let defaults: Option<JobDefaults> = self
            .defaults
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let auto_delete: Option<AutoDelete> = self
            .auto_delete
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let webhooks: Vec<Webhook> = self
            .webhooks
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or_default();

        let mut secrets: HashMap<String, String> = self
            .secrets
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or_default();

        if !secrets.is_empty() {
            secrets = encrypt::decrypt_secrets(&secrets, encryption_key)?;
        }

        let schedule = self.scheduled_job_id.as_ref().map(|id| JobSchedule {
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
            webhooks: if webhooks.is_empty() {
                None
            } else {
                Some(webhooks)
            },
            permissions: if perms.is_empty() { None } else { Some(perms) },
            auto_delete,
            delete_at: self.delete_at,
            secrets: if secrets.is_empty() {
                None
            } else {
                Some(secrets)
            },
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
    /// Converts the database record to a `ScheduledJob` domain object.
    pub fn to_scheduled_job(
        &self,
        tasks: Vec<Task>,
        created_by: User,
        perms: Vec<Permission>,
        encryption_key: Option<&str>,
    ) -> Result<ScheduledJob, DatastoreError> {
        let inputs: HashMap<String, String> = serde_json::from_slice(&self.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.inputs: {e}")))?;

        let defaults: Option<JobDefaults> = self
            .defaults
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let auto_delete: Option<AutoDelete> = self
            .auto_delete
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let webhooks: Vec<Webhook> = self
            .webhooks
            .as_ref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or_default();

        let mut secrets: HashMap<String, String> = self
            .secrets
            .as_ref()
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
            webhooks: if webhooks.is_empty() {
                None
            } else {
                Some(webhooks)
            },
            permissions: if perms.is_empty() { None } else { Some(perms) },
            created_at: self.created_at,
            tags: self.tags.clone(),
            secrets: if secrets.is_empty() {
                None
            } else {
                Some(secrets)
            },
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
    #[must_use]
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
    /// Converts the database record to a `TaskLogPart` domain object.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn to_role(&self) -> Role {
        Role {
            id: Some(self.id.clone()),
            slug: Some(self.slug.clone()),
            name: Some(self.name.clone()),
            created_at: Some(self.created_at),
        }
    }
}

/// Helper to convert a string slice to `TaskState`
fn str_to_task_state(s: &str) -> TaskState {
    TaskState::from(std::borrow::Cow::Owned(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tork::{
        node::NODE_STATUS_UP,
        task::{ParallelTask, Registry, TaskLimits, TaskRetry},
    };

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Creates a fixed-point timestamp for deterministic tests.
    fn fixed_now() -> OffsetDateTime {
        OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::March, 22).unwrap_or_else(|_| {
                time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap()
            }),
            time::Time::from_hms(12, 0, 0).unwrap_or_else(|_| time::Time::MIDNIGHT),
        )
    }

    fn base_task_record() -> TaskRecord {
        let now = fixed_now();
        TaskRecord {
            id: "task-001".to_string(),
            job_id: "job-001".to_string(),
            position: 0,
            name: Some("build".to_string()),
            description: Some("build the project".to_string()),
            state: "PENDING".to_string(),
            created_at: now,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            cmd: Some(vec!["cargo".to_string(), "build".to_string()]),
            entrypoint: None,
            run_script: Some("cargo build".to_string()),
            image: Some("rust:latest".to_string()),
            registry: None,
            env: None,
            files_: None,
            queue: Some("default".to_string()),
            error_: None,
            pre_tasks: None,
            post_tasks: None,
            sidecars: None,
            mounts: None,
            networks: None,
            node_id: None,
            retry: None,
            limits: None,
            timeout: Some("30s".to_string()),
            var: Some("result".to_string()),
            result: None,
            parallel: None,
            parent_id: None,
            each_: None,
            subjob: None,
            gpus: None,
            if_: None,
            tags: None,
            priority: Some(5),
            workdir: Some("/src".to_string()),
            progress: Some(0.0),
        }
    }

    // ── TaskRecord → Task conversion tests ──────────────────────────────

    #[test]
    fn task_record_to_task_basic_fields() {
        let record = base_task_record();
        let task = record.to_task().expect("conversion should succeed");

        assert_eq!(task.id.as_deref(), Some("task-001"));
        assert_eq!(task.job_id.as_deref(), Some("job-001"));
        assert_eq!(task.position, 0);
        assert_eq!(task.name.as_deref(), Some("build"));
        assert_eq!(task.description.as_deref(), Some("build the project"));
        assert_eq!(task.state.as_ref(), "PENDING");
        assert_eq!(task.run.as_deref(), Some("cargo build"));
        assert_eq!(task.image.as_deref(), Some("rust:latest"));
        assert_eq!(task.queue.as_deref(), Some("default"));
        assert_eq!(task.timeout.as_deref(), Some("30s"));
        assert_eq!(task.var.as_deref(), Some("result"));
        assert_eq!(task.priority, 5);
        assert_eq!(task.workdir.as_deref(), Some("/src"));
        assert_eq!(task.progress, 0.0);
        assert!(task.probe.is_none());
        assert_eq!(task.redelivered, 0);
        assert!(task.parent_id.is_none());
        assert!(task.error.is_none());
        assert!(task.result.is_none());
    }

    #[test]
    fn task_record_to_task_with_cmd_and_entrypoint() {
        let record = TaskRecord {
            cmd: Some(vec![
                "sh".to_string(),
                "-c".to_string(),
                "echo hi".to_string(),
            ]),
            entrypoint: Some(vec!["/entrypoint.sh".to_string()]),
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        let cmd = task.cmd.as_ref().expect("cmd should be present");
        assert_eq!(cmd.len(), 3);
        assert_eq!(cmd[0], "sh");
        assert_eq!(cmd[1], "-c");
        assert_eq!(cmd[2], "echo hi");

        let entry = task
            .entrypoint
            .as_ref()
            .expect("entrypoint should be present");
        assert_eq!(entry.len(), 1);
        assert_eq!(entry[0], "/entrypoint.sh");
    }

    #[test]
    fn task_record_to_task_with_env_and_files() {
        let mut env_map = HashMap::new();
        env_map.insert("RUST_LOG".to_string(), "debug".to_string());
        env_map.insert("HOME".to_string(), "/root".to_string());
        let env = serde_json::to_vec(&env_map).ok();

        let mut files_map = HashMap::new();
        files_map.insert("config.yml".to_string(), "key: val".to_string());
        let files = serde_json::to_vec(&files_map).ok();

        let record = TaskRecord {
            env,
            files_: files,
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        let env_result = task.env.as_ref().expect("env should be present");
        assert_eq!(
            env_result.get("RUST_LOG").map(String::as_str),
            Some("debug")
        );
        assert_eq!(env_result.get("HOME").map(String::as_str), Some("/root"));

        let files_result = task.files.as_ref().expect("files should be present");
        assert_eq!(
            files_result.get("config.yml").map(String::as_str),
            Some("key: val")
        );
    }

    #[test]
    fn task_record_to_task_with_registry() {
        let registry = Registry {
            username: Some("admin".to_string()),
            password: Some("s3cret".to_string()),
        };
        let registry_bytes = serde_json::to_vec(&registry).ok();

        let record = TaskRecord {
            registry: registry_bytes,
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        let reg = task.registry.as_ref().expect("registry should be present");
        assert_eq!(reg.username.as_deref(), Some("admin"));
        assert_eq!(reg.password.as_deref(), Some("s3cret"));
    }

    #[test]
    fn task_record_to_task_with_retry_and_limits() {
        let retry = TaskRetry {
            limit: 3,
            attempts: 0,
        };
        let limits = TaskLimits {
            cpus: Some("0.5".to_string()),
            memory: Some("256MB".to_string()),
        };
        let record = TaskRecord {
            retry: serde_json::to_vec(&retry).ok(),
            limits: serde_json::to_vec(&limits).ok(),
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        let r = task.retry.as_ref().expect("retry should be present");
        assert_eq!(r.limit, 3);
        assert_eq!(r.attempts, 0);

        let l = task.limits.as_ref().expect("limits should be present");
        assert_eq!(l.cpus.as_deref(), Some("0.5"));
        assert_eq!(l.memory.as_deref(), Some("256MB"));
    }

    #[test]
    fn task_record_to_task_with_parallel() {
        let parallel = ParallelTask {
            tasks: Some(vec![]),
            completions: 0,
        };
        let record = TaskRecord {
            parallel: serde_json::to_vec(&parallel).ok(),
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        assert!(task.parallel.is_some());
        let p = task.parallel.as_ref().unwrap();
        assert_eq!(p.completions, 0);
        assert!(p.tasks.is_some());
    }

    #[test]
    fn task_record_to_task_with_networks_and_tags() {
        let record = TaskRecord {
            networks: Some(vec!["bridge".to_string(), "host".to_string()]),
            tags: Some(vec!["ci".to_string(), "rust".to_string()]),
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        let nets = task.networks.as_ref().expect("networks should be present");
        assert_eq!(nets, &["bridge".to_string(), "host".to_string()]);

        let tags = task.tags.as_ref().expect("tags should be present");
        assert_eq!(tags, &["ci".to_string(), "rust".to_string()]);
    }

    #[test]
    fn task_record_to_task_with_parent_id() {
        let record = TaskRecord {
            parent_id: Some("parent-task-001".to_string()),
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        assert_eq!(task.parent_id.as_deref(), Some("parent-task-001"));
    }

    #[test]
    fn task_record_to_task_with_error_and_result() {
        let record = TaskRecord {
            error_: Some("oom killed".to_string()),
            result: Some("success".to_string()),
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        assert_eq!(task.error.as_deref(), Some("oom killed"));
        assert_eq!(task.result.as_deref(), Some("success"));
    }

    #[test]
    fn task_record_to_task_default_priority_and_progress() {
        // When priority and progress are None, defaults to 0
        let record = TaskRecord {
            priority: None,
            progress: None,
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        assert_eq!(task.priority, 0);
        assert_eq!(task.progress, 0.0);
    }

    #[test]
    fn task_record_to_task_all_timestamps() {
        let now = fixed_now();
        let record = TaskRecord {
            scheduled_at: Some(now),
            started_at: Some(now),
            completed_at: Some(now),
            failed_at: Some(now),
            ..base_task_record()
        };
        let task = record.to_task().expect("conversion should succeed");

        assert!(task.scheduled_at.is_some());
        assert!(task.started_at.is_some());
        assert!(task.completed_at.is_some());
        assert!(task.failed_at.is_some());
    }

    #[test]
    fn task_record_to_task_empty_optional_fields() {
        // Everything optional is None
        let now = fixed_now();
        let record = TaskRecord {
            id: "task-minimal".to_string(),
            job_id: "job-001".to_string(),
            state: "CREATED".to_string(),
            created_at: now,
            position: 0,
            name: None,
            description: None,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            cmd: None,
            entrypoint: None,
            run_script: None,
            image: None,
            registry: None,
            env: None,
            files_: None,
            queue: None,
            error_: None,
            pre_tasks: None,
            post_tasks: None,
            sidecars: None,
            mounts: None,
            networks: None,
            node_id: None,
            retry: None,
            limits: None,
            timeout: None,
            var: None,
            result: None,
            parallel: None,
            parent_id: None,
            each_: None,
            subjob: None,
            gpus: None,
            if_: None,
            tags: None,
            priority: None,
            workdir: None,
            progress: None,
        };
        let task = record.to_task().expect("conversion should succeed");

        assert_eq!(task.id.as_deref(), Some("task-minimal"));
        assert_eq!(task.state.as_ref(), "CREATED");
        assert!(task.name.is_none());
        assert!(task.description.is_none());
        assert!(task.cmd.is_none());
        assert!(task.image.is_none());
        assert!(task.queue.is_none());
        assert!(task.timeout.is_none());
        assert!(task.var.is_none());
        assert!(task.result.is_none());
        assert!(task.error.is_none());
        assert!(task.networks.is_none());
        assert!(task.tags.is_none());
        assert!(task.gpus.is_none());
        assert!(task.r#if.is_none());
        assert!(task.workdir.is_none());
        assert!(task.parent_id.is_none());
        assert!(task.node_id.is_none());
        assert!(task.registry.is_none());
        assert!(task.env.is_none());
        assert!(task.files.is_none());
        assert!(task.retry.is_none());
        assert!(task.limits.is_none());
        assert!(task.parallel.is_none());
        assert!(task.each.is_none());
        assert!(task.subjob.is_none());
        assert!(task.mounts.is_none());
        assert!(task.pre.is_none());
        assert!(task.post.is_none());
        assert!(task.sidecars.is_none());
    }

    // ── NodeRecord → Node conversion tests ──────────────────────────────

    fn base_node_record() -> NodeRecord {
        let now = time::OffsetDateTime::now_utc();
        NodeRecord {
            id: "node-001".to_string(),
            name: "worker-1".to_string(),
            started_at: now,
            last_heartbeat_at: now, // recent heartbeat
            cpu_percent: 45.5,
            queue: "default".to_string(),
            status: NODE_STATUS_UP.to_string(),
            hostname: "worker-1.local".to_string(),
            port: 8080,
            task_count: 3,
            version_: "1.0.0".to_string(),
        }
    }

    #[test]
    fn node_record_to_node_basic_fields() {
        let record = base_node_record();
        let node = record.to_node();

        assert_eq!(node.id.as_deref(), Some("node-001"));
        assert_eq!(node.name.as_deref(), Some("worker-1"));
        assert_eq!(node.hostname.as_deref(), Some("worker-1.local"));
        assert_eq!(node.port, 8080);
        assert_eq!(node.task_count, 3);
        assert_eq!(node.version, "1.0.0");
        assert_eq!(node.queue.as_deref(), Some("default"));
    }

    #[test]
    fn node_record_to_node_recent_heartbeat_stays_up() {
        // Node with recent heartbeat should keep its UP status
        let record = base_node_record();
        let node = record.to_node();

        assert_eq!(node.status, "UP");
    }

    #[test]
    fn node_record_to_node_stale_heartbeat_goes_offline() {
        // Node with stale heartbeat (>60s old) and UP status should become OFFLINE
        let stale = fixed_now() - time::Duration::seconds(120);
        let record = NodeRecord {
            last_heartbeat_at: stale,
            status: NODE_STATUS_UP.to_string(),
            ..base_node_record()
        };
        let node = record.to_node();

        assert_eq!(node.status, "OFFLINE");
    }

    #[test]
    fn node_record_to_node_non_up_status_preserved() {
        // A DOWN node should stay DOWN regardless of heartbeat
        let stale = fixed_now() - time::Duration::seconds(120);
        let record = NodeRecord {
            last_heartbeat_at: stale,
            status: "DOWN".to_string(),
            ..base_node_record()
        };
        let node = record.to_node();

        // Only UP transitions to OFFLINE on stale heartbeat
        assert_eq!(node.status, "DOWN");
    }

    #[test]
    fn node_record_to_node_cpu_percent_preserved() {
        let record = NodeRecord {
            cpu_percent: 99.9,
            ..base_node_record()
        };
        let node = record.to_node();
        assert!((node.cpu_percent - 99.9).abs() < f64::EPSILON);
    }

    // ── UserRecord → User conversion tests ──────────────────────────────

    #[test]
    fn user_record_to_user_basic_fields() {
        let now = fixed_now();
        let record = UserRecord {
            id: "user-001".to_string(),
            name: "Test User".to_string(),
            username_: "testuser".to_string(),
            password_: "$2b$12$hashed".to_string(),
            created_at: now,
            is_disabled: false,
        };
        let user = record.to_user();

        assert_eq!(user.id.as_deref(), Some("user-001"));
        assert_eq!(user.name.as_deref(), Some("Test User"));
        assert_eq!(user.username.as_deref(), Some("testuser"));
        assert_eq!(user.password_hash.as_deref(), Some("$2b$12$hashed"));
        assert!(user.password.is_none()); // password should never be set from record
        assert!(!user.disabled);
    }

    #[test]
    fn user_record_to_user_disabled() {
        let now = fixed_now();
        let record = UserRecord {
            id: "user-002".to_string(),
            name: "Banned".to_string(),
            username_: "banned".to_string(),
            password_: "".to_string(),
            created_at: now,
            is_disabled: true,
        };
        let user = record.to_user();

        assert!(user.disabled);
    }

    // ── RoleRecord → Role conversion tests ──────────────────────────────

    #[test]
    fn role_record_to_role_basic_fields() {
        let now = fixed_now();
        let record = RoleRecord {
            id: "role-001".to_string(),
            slug: "admin".to_string(),
            name: "Administrator".to_string(),
            created_at: now,
        };
        let role = record.to_role();

        assert_eq!(role.id.as_deref(), Some("role-001"));
        assert_eq!(role.slug.as_deref(), Some("admin"));
        assert_eq!(role.name.as_deref(), Some("Administrator"));
    }

    // ── TaskLogPartRecord → TaskLogPart conversion tests ────────────────

    #[test]
    fn task_log_part_record_to_task_log_part() {
        let now = fixed_now();
        let record = TaskLogPartRecord {
            id: "log-001".to_string(),
            number_: 1,
            task_id: "task-001".to_string(),
            created_at: now,
            contents: "line 1\nline 2\n".to_string(),
        };
        let part = record.to_task_log_part();

        assert_eq!(part.id.as_deref(), Some("log-001"));
        assert_eq!(part.number, 1);
        assert_eq!(part.task_id.as_deref(), Some("task-001"));
        assert_eq!(part.contents.as_deref(), Some("line 1\nline 2\n"));
        assert!(part.created_at.is_some());
    }

    // ── JobRecord → Job conversion tests ────────────────────────────────

    fn base_job_record() -> JobRecord {
        let now = fixed_now();
        JobRecord {
            id: "job-001".to_string(),
            name: Some("Build Job".to_string()),
            description: Some("Build the project".to_string()),
            tags: Some(vec!["ci".to_string()]),
            state: "PENDING".to_string(),
            created_at: now,
            created_by: "user-001".to_string(),
            started_at: None,
            completed_at: None,
            failed_at: None,
            delete_at: None,
            tasks: serde_json::to_vec(&Vec::<Task>::new()).unwrap_or_default(),
            position: 1,
            inputs: serde_json::to_vec(&HashMap::<String, String>::new()).unwrap_or_default(),
            context: serde_json::to_vec(&JobContext::default()).unwrap_or_default(),
            parent_id: None,
            task_count: 5,
            output_: None,
            result: None,
            error_: None,
            ts: None,
            defaults: None,
            webhooks: None,
            auto_delete: None,
            secrets: None,
            progress: Some(0.0),
            scheduled_job_id: None,
        }
    }

    fn base_user() -> User {
        User {
            id: Some("user-001".to_string()),
            name: Some("Test User".to_string()),
            username: Some("testuser".to_string()),
            password_hash: Some("hashed".to_string()),
            password: None,
            created_at: Some(fixed_now()),
            disabled: false,
        }
    }

    #[test]
    fn job_record_to_job_basic_fields() {
        let record = base_job_record();
        let user = base_user();
        let job = record
            .to_job(vec![], vec![], user, vec![], None)
            .expect("conversion should succeed");

        assert_eq!(job.id.as_deref(), Some("job-001"));
        assert_eq!(job.name.as_deref(), Some("Build Job"));
        assert_eq!(job.description.as_deref(), Some("Build the project"));
        assert_eq!(job.state, "PENDING");
        assert_eq!(job.position, 1);
        assert_eq!(job.task_count, 5);
        assert_eq!(job.progress, 0.0);
    }

    #[test]
    fn job_record_to_job_with_tags() {
        let record = JobRecord {
            tags: Some(vec![
                "ci".to_string(),
                "rust".to_string(),
                "release".to_string(),
            ]),
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let tags = job.tags.as_ref().expect("tags should be present");
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], "ci");
        assert_eq!(tags[1], "rust");
        assert_eq!(tags[2], "release");
    }

    #[test]
    fn job_record_to_job_with_created_by() {
        let record = base_job_record();
        let user = base_user();
        let job = record
            .to_job(vec![], vec![], user, vec![], None)
            .expect("conversion should succeed");

        let created_by = job
            .created_by
            .as_ref()
            .expect("created_by should be present");
        assert_eq!(created_by.id.as_deref(), Some("user-001"));
        assert_eq!(created_by.username.as_deref(), Some("testuser"));
    }

    #[test]
    fn job_record_to_job_with_inputs() {
        let mut inputs = HashMap::new();
        inputs.insert("var1".to_string(), "val1".to_string());
        inputs.insert("var2".to_string(), "val2".to_string());
        let record = JobRecord {
            inputs: serde_json::to_vec(&inputs).unwrap_or_default(),
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let job_inputs = job.inputs.as_ref().expect("inputs should be present");
        assert_eq!(job_inputs.len(), 2);
        assert_eq!(job_inputs.get("var1").map(String::as_str), Some("val1"));
        assert_eq!(job_inputs.get("var2").map(String::as_str), Some("val2"));
    }

    #[test]
    fn job_record_to_job_with_defaults() {
        let defaults = JobDefaults {
            timeout: Some("30s".to_string()),
            retry: Some(TaskRetry {
                limit: 3,
                attempts: 0,
            }),
            limits: Some(TaskLimits {
                cpus: Some("1.0".to_string()),
                memory: Some("512MB".to_string()),
            }),
            queue: None,
            priority: 0,
        };
        let record = JobRecord {
            defaults: serde_json::to_vec(&defaults).ok(),
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let d = job.defaults.as_ref().expect("defaults should be present");
        assert_eq!(d.timeout.as_deref(), Some("30s"));
        let r = d.retry.as_ref().expect("retry should be present");
        assert_eq!(r.limit, 3);
        let l = d.limits.as_ref().expect("limits should be present");
        assert_eq!(l.cpus.as_deref(), Some("1.0"));
    }

    #[test]
    fn job_record_to_job_with_auto_delete() {
        let auto_delete = AutoDelete {
            after: Some("5h".to_string()),
        };
        let record = JobRecord {
            auto_delete: serde_json::to_vec(&auto_delete).ok(),
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let ad = job
            .auto_delete
            .as_ref()
            .expect("auto_delete should be present");
        assert_eq!(ad.after.as_deref(), Some("5h"));
    }

    #[test]
    fn job_record_to_job_with_webhooks() {
        let webhooks = vec![
            Webhook {
                url: Some("http://example.com/1".to_string()),
                headers: None,
                event: None,
                r#if: None,
            },
            Webhook {
                url: Some("http://example.com/2".to_string()),
                headers: Some({
                    let mut m = HashMap::new();
                    m.insert("Auth".to_string(), "Bearer token".to_string());
                    m
                }),
                event: Some("job.StatusChange".to_string()),
                r#if: None,
            },
        ];
        let record = JobRecord {
            webhooks: serde_json::to_vec(&webhooks).ok(),
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let wh = job.webhooks.as_ref().expect("webhooks should be present");
        assert_eq!(wh.len(), 2);
        assert_eq!(wh[0].url.as_deref(), Some("http://example.com/1"));
        assert_eq!(wh[1].event.as_deref(), Some("job.StatusChange"));
    }

    #[test]
    fn job_record_to_job_with_permissions() {
        let perms = vec![
            Permission {
                user: Some(base_user()),
                role: None,
            },
            Permission {
                user: None,
                role: Some(Role {
                    id: Some("role-pub".to_string()),
                    slug: Some("public".to_string()),
                    name: Some("Public".to_string()),
                    created_at: Some(fixed_now()),
                }),
            },
        ];
        let record = base_job_record();
        let job = record
            .to_job(vec![], vec![], base_user(), perms, None)
            .expect("conversion should succeed");

        let job_perms = job
            .permissions
            .as_ref()
            .expect("permissions should be present");
        assert_eq!(job_perms.len(), 2);
        assert!(job_perms[0].user.is_some());
        assert!(job_perms[1].role.is_some());
    }

    #[test]
    fn job_record_to_job_with_schedule() {
        let record = JobRecord {
            scheduled_job_id: Some("sched-001".to_string()),
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let sched = job.schedule.as_ref().expect("schedule should be present");
        assert_eq!(sched.id.as_deref(), Some("sched-001"));
        assert!(sched.cron.is_none());
    }

    #[test]
    fn job_record_to_job_with_delete_at() {
        let delete_at = fixed_now() + time::Duration::days(7);
        let record = JobRecord {
            delete_at: Some(delete_at),
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert!(job.delete_at.is_some());
    }

    #[test]
    fn job_record_to_job_without_encryption_secrets_pass_through() {
        let mut secrets = HashMap::new();
        secrets.insert("key".to_string(), "value".to_string());
        let record = JobRecord {
            secrets: serde_json::to_vec(&secrets).ok(),
            ..base_job_record()
        };
        // No encryption key → secrets should be returned as-is (not encrypted)
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let job_secrets = job.secrets.as_ref().expect("secrets should be present");
        assert_eq!(job_secrets.get("key").map(String::as_str), Some("value"));
    }

    #[test]
    fn job_record_to_job_no_secrets() {
        let record = base_job_record();
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert!(job.secrets.is_none());
    }

    #[test]
    fn job_record_to_job_empty_webhooks_and_perms_yield_none() {
        // Empty webhooks JSON and empty perms → None
        let record_with_empty_webhooks = JobRecord {
            webhooks: Some(serde_json::to_vec(&Vec::<Webhook>::new()).unwrap_or_default()),
            ..base_job_record()
        };
        let job = record_with_empty_webhooks
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert!(job.webhooks.is_none()); // empty vec → None
        assert!(job.permissions.is_none()); // empty perms → None
    }

    #[test]
    fn job_record_to_job_progress_defaults() {
        let record = JobRecord {
            progress: None,
            ..base_job_record()
        };
        let job = record
            .to_job(vec![], vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert_eq!(job.progress, 0.0);
    }

    // ── ScheduledJobRecord → ScheduledJob conversion tests ──────────────

    fn base_scheduled_job_record() -> ScheduledJobRecord {
        let now = fixed_now();
        ScheduledJobRecord {
            id: "sched-001".to_string(),
            cron_expr: Some("0 0 * * *".to_string()),
            name: Some("Nightly Build".to_string()),
            description: Some("Build every night".to_string()),
            tags: Some(vec!["nightly".to_string()]),
            state: "ACTIVE".to_string(),
            created_at: now,
            created_by: "user-001".to_string(),
            tasks: serde_json::to_vec(&Vec::<Task>::new()).unwrap_or_default(),
            inputs: serde_json::to_vec(&HashMap::<String, String>::new()).unwrap_or_default(),
            output_: None,
            defaults: None,
            webhooks: None,
            auto_delete: None,
            secrets: None,
        }
    }

    #[test]
    fn scheduled_job_record_to_scheduled_job_basic() {
        let record = base_scheduled_job_record();
        let sj = record
            .to_scheduled_job(vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert_eq!(sj.id.as_deref(), Some("sched-001"));
        assert_eq!(sj.name.as_deref(), Some("Nightly Build"));
        assert_eq!(sj.description.as_deref(), Some("Build every night"));
        assert_eq!(sj.state, "ACTIVE");
    }

    #[test]
    fn scheduled_job_record_to_scheduled_job_with_cron() {
        let record = ScheduledJobRecord {
            cron_expr: Some("0 0 * * *".to_string()),
            ..base_scheduled_job_record()
        };
        let sj = record
            .to_scheduled_job(vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert_eq!(sj.cron.as_deref(), Some("0 0 * * *"));
    }

    #[test]
    fn scheduled_job_record_to_scheduled_job_with_inputs() {
        let mut inputs = HashMap::new();
        inputs.insert("BRANCH".to_string(), "main".to_string());
        let record = ScheduledJobRecord {
            inputs: serde_json::to_vec(&inputs).unwrap_or_default(),
            ..base_scheduled_job_record()
        };
        let sj = record
            .to_scheduled_job(vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let sj_inputs = sj.inputs.as_ref().expect("inputs should be present");
        assert_eq!(sj_inputs.get("BRANCH").map(String::as_str), Some("main"));
    }

    #[test]
    fn scheduled_job_record_to_scheduled_job_with_tags() {
        let record = ScheduledJobRecord {
            tags: Some(vec!["nightly".to_string(), "prod".to_string()]),
            ..base_scheduled_job_record()
        };
        let sj = record
            .to_scheduled_job(vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        let tags = sj.tags.as_ref().expect("tags should be present");
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn scheduled_job_record_to_scheduled_job_no_secrets() {
        let record = base_scheduled_job_record();
        let sj = record
            .to_scheduled_job(vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert!(sj.secrets.is_none());
    }

    #[test]
    fn scheduled_job_record_to_scheduled_job_empty_webhooks_and_perms_yield_none() {
        let record = ScheduledJobRecord {
            webhooks: Some(serde_json::to_vec(&Vec::<Webhook>::new()).unwrap_or_default()),
            ..base_scheduled_job_record()
        };
        let sj = record
            .to_scheduled_job(vec![], base_user(), vec![], None)
            .expect("conversion should succeed");

        assert!(sj.webhooks.is_none());
        assert!(sj.permissions.is_none());
    }

    // ── str_to_task_state helper tests ─────────────────────────────────

    #[test]
    fn str_to_task_state_converts_all_states() {
        let states = [
            "CREATED",
            "PENDING",
            "SCHEDULED",
            "RUNNING",
            "CANCELLED",
            "STOPPED",
            "COMPLETED",
            "FAILED",
            "SKIPPED",
        ];
        for state in &states {
            let converted = str_to_task_state(state);
            assert_eq!(converted.as_ref(), *state);
        }
    }
}
