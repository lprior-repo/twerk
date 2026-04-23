//! Task record types and conversions to domain types.

use serde::de::DeserializeOwned;
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
        fn parse_bytes<T: DeserializeOwned>(
            label: &'static str,
            bytes: Option<&Vec<u8>>,
        ) -> std::result::Result<Option<T>, DatastoreError> {
            bytes
                .map(|b| {
                    serde_json::from_slice(b)
                        .map_err(|e| DatastoreError::Serialization(format!("task.{label}: {e}")))
                })
                .transpose()
        }

        let env =
            parse_bytes::<std::collections::HashMap<String, String>>("env", self.env.as_ref())?;
        let files = parse_bytes::<std::collections::HashMap<String, String>>(
            "files",
            self.files_.as_ref(),
        )?;
        let pre = parse_bytes::<Vec<Task>>("pre_tasks", self.pre_tasks.as_ref())?;
        let post = parse_bytes::<Vec<Task>>("post_tasks", self.post_tasks.as_ref())?;
        let sidecars = parse_bytes::<Vec<Task>>("sidecars", self.sidecars.as_ref())?;
        let retry = parse_bytes::<twerk_core::task::TaskRetry>("retry", self.retry.as_ref())?;
        let limits = parse_bytes::<twerk_core::task::TaskLimits>("limits", self.limits.as_ref())?;
        let parallel =
            parse_bytes::<twerk_core::task::ParallelTask>("parallel", self.parallel.as_ref())?;
        let each = parse_bytes::<Box<twerk_core::task::EachTask>>("each", self.each_.as_ref())?;
        let subjob = parse_bytes::<twerk_core::task::SubJobTask>("subjob", self.subjob.as_ref())?;
        let registry =
            parse_bytes::<twerk_core::task::Registry>("registry", self.registry.as_ref())?;
        let mounts = parse_bytes::<Vec<twerk_core::mount::Mount>>("mounts", self.mounts.as_ref())?;
        let state = self
            .state
            .parse()
            .map_err(|e| DatastoreError::Serialization(format!("task.state: {e}")))?;

        let task_id = TaskId::new(self.id.clone())?;
        let job_id = JobId::new(self.job_id.clone())?;
        let parent_id = match &self.parent_id {
            Some(id) => Some(TaskId::new(id.clone())?),
            None => None,
        };
        let node_id = match &self.node_id {
            Some(id) => Some(twerk_core::id::NodeId::new(id.clone())?),
            None => None,
        };

        Ok(Task {
            id: Some(task_id),
            job_id: Some(job_id),
            parent_id,
            position: self.position,
            name: self.name.clone(),
            description: self.description.clone(),
            state,
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
            node_id,
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
