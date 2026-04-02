//! Scheduled job record types and conversions to domain types.

use sqlx::FromRow;

use crate::datastore::postgres::encrypt;
use crate::datastore::Error as DatastoreError;
use twerk_core::{
    id::ScheduledJobId,
    job::{ScheduledJob, ScheduledJobState},
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
            id: Some(ScheduledJobId::new(self.id.clone())),
            name: self.name.clone(),
            description: self.description.clone(),
            cron: self.cron_expr.clone(),
            state: ScheduledJobState::from(self.state.as_str()),
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

#[cfg(test)]
mod tests {
    use super::super::helpers::fixed_now;
    use super::*;
    use std::collections::HashMap;

    // ── Helpers ──────────────────────────────────────────────────────────

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
            tasks: serde_json::to_vec(&Vec::<twerk_core::task::Task>::new()).unwrap_or_default(),
            inputs: serde_json::to_vec(&std::collections::HashMap::<String, String>::new())
                .unwrap_or_default(),
            output_: None,
            defaults: None,
            webhooks: None,
            auto_delete: None,
            secrets: None,
        }
    }

    fn base_user() -> User {
        User {
            id: Some(twerk_core::id::UserId::new("user-001")),
            name: Some("Test User".to_string()),
            username: Some("testuser".to_string()),
            password_hash: Some("hashed".to_string()),
            password: None,
            created_at: Some(fixed_now()),
            disabled: false,
        }
    }

    // ── ScheduledJobRecord → ScheduledJob conversion tests ──────────────

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
}
