//! Task record types and conversions to domain types.

use sqlx::FromRow;

use crate::datastore::Error as DatastoreError;
use twerk_core::{
    id::{JobId, TaskId},
    task::Task,
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
    pub created_at: time::OffsetDateTime,
    pub scheduled_at: Option<time::OffsetDateTime>,
    pub started_at: Option<time::OffsetDateTime>,
    pub completed_at: Option<time::OffsetDateTime>,
    pub failed_at: Option<time::OffsetDateTime>,
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

/// Extension trait for TaskRecord conversions
pub trait TaskRecordExt {
    /// Converts the database record to a Task domain object.
    fn to_task(&self) -> Result<Task, DatastoreError>;
}

impl TaskRecordExt for TaskRecord {
    fn to_task(&self) -> Result<Task, DatastoreError> {
        let parse_bytes = |label: &'static str,
                           bytes: Option<&Vec<u8>>|
         -> std::result::Result<Option<_>, DatastoreError> {
            bytes
                .map(|b| {
                    serde_json::from_slice(b)
                        .map_err(|e| DatastoreError::Serialization(format!("task.{label}: {e}")))
                })
                .transpose()
        };

        let env = parse_bytes("env", self.env.as_ref())?.flatten();
        let files = parse_bytes("files", self.files_.as_ref())?.flatten();
        let pre = parse_bytes("pre_tasks", self.pre_tasks.as_ref())?.flatten();
        let post = parse_bytes("post_tasks", self.post_tasks.as_ref())?.flatten();
        let sidecars = parse_bytes("sidecars", self.sidecars.as_ref())?.flatten();
        let retry = parse_bytes("retry", self.retry.as_ref())?;
        let limits = parse_bytes("limits", self.limits.as_ref())?;
        let parallel = parse_bytes("parallel", self.parallel.as_ref())?;
        let each = parse_bytes("each", self.each_.as_ref())?;
        let subjob = parse_bytes("subjob", self.subjob.as_ref())?;
        let registry = parse_bytes("registry", self.registry.as_ref())?;
        let mounts = parse_bytes("mounts", self.mounts.as_ref())?.flatten();

        Ok(Task {
            id: Some(TaskId::new(self.id.clone())),
            job_id: Some(JobId::new(self.job_id.clone())),
            parent_id: self.parent_id.as_ref().map(|id| TaskId::new(id.clone())),
            position: self.position,
            name: self.name.clone(),
            description: self.description.clone(),
            state: twerk_core::task::TaskState::from(self.state.clone()),
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
            node_id: self
                .node_id
                .as_ref()
                .map(|id| twerk_core::id::NodeId::new(id.clone())),
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
            priority: self.priority.map_or(0, |p| p),
            progress: self.progress.map_or(0.0, |p| p),
            probe: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use twerk_core::task::{ParallelTask, Registry, TaskLimits, TaskRetry};

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Creates a fixed-point timestamp for deterministic tests.
    fn fixed_now() -> time::OffsetDateTime {
        time::OffsetDateTime::new_utc(
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
        assert_eq!(task.state.as_str(), "PENDING");
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
        assert_eq!(task.state.as_str(), "CREATED");
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
}
