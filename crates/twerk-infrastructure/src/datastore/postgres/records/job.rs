//! Job record types and conversions to domain types.

use sqlx::FromRow;

use crate::datastore::postgres::encrypt;
use crate::datastore::Error as DatastoreError;
use twerk_core::{
    id::{JobId, ScheduledJobId},
    job::{Job, JobContext, JobSchedule},
    task::{Permission, Task},
    user::User,
    webhook::Webhook,
};

/// Job record from the database
#[derive(Debug, Clone, FromRow)]
pub struct JobRecord {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub state: String,
    pub created_at: time::OffsetDateTime,
    pub created_by: String,
    pub started_at: Option<time::OffsetDateTime>,
    pub completed_at: Option<time::OffsetDateTime>,
    pub failed_at: Option<time::OffsetDateTime>,
    pub delete_at: Option<time::OffsetDateTime>,
    pub tasks: Vec<u8>,
    pub position: i64,
    pub inputs: Vec<u8>,
    pub context: Vec<u8>,
    pub parent_id: Option<String>,
    pub task_count: i64,
    pub output_: Option<String>,
    pub result: Option<String>,
    pub error_: Option<String>,
    pub defaults: Option<Vec<u8>>,
    pub webhooks: Option<Vec<u8>>,
    pub auto_delete: Option<Vec<u8>>,
    pub secrets: Option<Vec<u8>>,
    pub progress: Option<f64>,
    pub scheduled_job_id: Option<String>,
}

/// Extension trait for JobRecord conversions
pub trait JobRecordExt {
    /// Converts the database record to a Job domain object.
    fn to_job(
        &self,
        tasks: Vec<Task>,
        execution: Vec<Task>,
        created_by: User,
        perms: Vec<Permission>,
        encryption_key: Option<&str>,
    ) -> Result<Job, DatastoreError>;
}

impl JobRecordExt for JobRecord {
    fn to_job(
        &self,
        tasks: Vec<Task>,
        execution: Vec<Task>,
        created_by: User,
        perms: Vec<Permission>,
        encryption_key: Option<&str>,
    ) -> Result<Job, DatastoreError> {
        let context: Option<JobContext> =
            if self.context.is_empty() || self.context.as_slice() == b"null" {
                None
            } else {
                Some(
                    serde_json::from_slice(&self.context)
                        .map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?,
                )
            };

        let inputs: Option<std::collections::HashMap<String, String>> =
            serde_json::from_slice(&self.inputs)
                .map_err(|e| DatastoreError::Serialization(format!("job.inputs: {e}")))?;

        let defaults: Option<twerk_core::job::JobDefaults> = self
            .defaults
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes)
                    .map_err(|e| DatastoreError::Serialization(format!("job.defaults: {e}")))
            })
            .transpose()?;

        let auto_delete: Option<twerk_core::task::AutoDelete> = self
            .auto_delete
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes)
                    .map_err(|e| DatastoreError::Serialization(format!("job.auto_delete: {e}")))
            })
            .transpose()?;

        let webhooks: Vec<Webhook> = match self
            .webhooks
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice::<Vec<Webhook>>(bytes)
                    .map_err(|e| DatastoreError::Serialization(format!("job.webhooks: {e}")))
            })
            .transpose()
        {
            Ok(opt) => opt.unwrap_or_else(Vec::new),
            Err(e) => return Err(e),
        };

        let mut secrets: std::collections::HashMap<String, String> = match self
            .secrets
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice::<std::collections::HashMap<String, String>>(bytes)
                    .map_err(|e| DatastoreError::Serialization(format!("job.secrets: {e}")))
            })
            .transpose()
        {
            Ok(opt) => opt.unwrap_or_else(std::collections::HashMap::new),
            Err(e) => return Err(e),
        };

        if !secrets.is_empty() {
            secrets = encrypt::decrypt_secrets(&secrets, encryption_key)?;
        }

        let state = self
            .state
            .parse()
            .map_err(|e| DatastoreError::Serialization(format!("job.state: {e}")))?;

        let schedule = match &self.scheduled_job_id {
            Some(id) => Some(JobSchedule {
                id: Some(ScheduledJobId::new(id.clone())?),
                cron: None,
            }),
            None => None,
        };

        let parent_id = match &self.parent_id {
            Some(id) => Some(JobId::new(id.clone())?),
            None => None,
        };

        Ok(Job {
            id: Some(JobId::new(self.id.clone())?),
            parent_id,
            name: self.name.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            state,
            created_at: Some(self.created_at),
            created_by: Some(created_by),
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            tasks: Some(tasks),
            execution: Some(execution),
            position: self.position,
            inputs,
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
            progress: self.progress.map_or(0.0, |p| p),
            schedule,
        })
    }
}
