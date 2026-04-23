//! Scheduled job record types and conversions to domain types.

use sqlx::FromRow;

use crate::datastore::postgres::encrypt;
use crate::datastore::Error as DatastoreError;
use twerk_core::{
    id::ScheduledJobId,
    job::ScheduledJob,
    task::{Permission, Task},
    user::User,
    webhook::Webhook,
};

/// Scheduled job record from the database
#[derive(Debug, Clone, FromRow)]
pub struct ScheduledJobRecord {
    pub id: String,
    pub cron_expr: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub state: String,
    pub created_at: time::OffsetDateTime,
    pub created_by: String,
    pub tasks: Vec<u8>,
    pub inputs: Vec<u8>,
    pub output_: Option<String>,
    pub defaults: Option<Vec<u8>>,
    pub webhooks: Option<Vec<u8>>,
    pub auto_delete: Option<Vec<u8>>,
    pub secrets: Option<Vec<u8>>,
}

/// Extension trait for ScheduledJobRecord conversions
pub trait ScheduledJobRecordExt {
    /// Converts the database record to a `ScheduledJob` domain object.
    fn to_scheduled_job(
        &self,
        tasks: Vec<Task>,
        created_by: User,
        perms: Vec<Permission>,
        encryption_key: Option<&str>,
    ) -> Result<ScheduledJob, DatastoreError>;
}

impl ScheduledJobRecordExt for ScheduledJobRecord {
    fn to_scheduled_job(
        &self,
        tasks: Vec<Task>,
        created_by: User,
        perms: Vec<Permission>,
        encryption_key: Option<&str>,
    ) -> Result<ScheduledJob, DatastoreError> {
        let inputs: Option<std::collections::HashMap<String, String>> =
            serde_json::from_slice(&self.inputs)
                .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.inputs: {e}")))?;

        let defaults: Option<twerk_core::job::JobDefaults> = self
            .defaults
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.defaults: {e}"))
                })
            })
            .transpose()?;

        let auto_delete: Option<twerk_core::task::AutoDelete> = self
            .auto_delete
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.auto_delete: {e}"))
                })
            })
            .transpose()?
            .unwrap_or_default();

        let webhooks: Vec<Webhook> = self
            .webhooks
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.webhooks: {e}"))
                })
            })
            .transpose()?
            .unwrap_or_default();

        let mut secrets: std::collections::HashMap<String, String> = self
            .secrets
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.secrets: {e}"))
                })
            })
            .transpose()?
            .unwrap_or_default();

        if !secrets.is_empty() {
            secrets = encrypt::decrypt_secrets(&secrets, encryption_key)?;
        }

        Ok(ScheduledJob {
            id: Some(ScheduledJobId::new(self.id.clone())?),
            name: self.name.clone(),
            description: self.description.clone(),
            cron: self.cron_expr.clone(),
            state: self.state.parse().unwrap_or_default(),
            inputs,
            tasks: Some(tasks),
            created_by: Some(created_by),
            defaults,
            auto_delete,
            webhooks: if webhooks.is_empty() {
                None
            } else {
                Some(webhooks)
            },
            permissions: if perms.is_empty() { None } else { Some(perms) },
            created_at: Some(self.created_at),
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
