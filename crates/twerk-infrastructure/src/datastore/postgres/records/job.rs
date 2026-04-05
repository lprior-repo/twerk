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

        let webhooks: Vec<Webhook> = self
            .webhooks
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes)
                    .map_err(|e| DatastoreError::Serialization(format!("job.webhooks: {e}")))
            })
            .transpose()?
            .unwrap_or_default();

        let mut secrets: std::collections::HashMap<String, String> = self
            .secrets
            .as_ref()
            .map(|bytes| {
                serde_json::from_slice(bytes)
                    .map_err(|e| DatastoreError::Serialization(format!("job.secrets: {e}")))
            })
            .transpose()?
            .unwrap_or_default();

        if !secrets.is_empty() {
            secrets = encrypt::decrypt_secrets(&secrets, encryption_key)?;
        }

        let schedule = self.scheduled_job_id.as_ref().map(|id| JobSchedule {
            id: Some(ScheduledJobId::new(id.clone())),
            cron: None,
        });

        Ok(Job {
            id: Some(JobId::new(self.id.clone())),
            parent_id: self.parent_id.as_ref().map(|id| JobId::new(id.clone())),
            name: self.name.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            state: self.state.parse().unwrap_or_default(),
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    use super::super::helpers::fixed_now;
    use super::*;
    use std::collections::HashMap;
    use twerk_core::job::{JobDefaults as CoreJobDefaults, JobState};
    use twerk_core::task::{AutoDelete, TaskLimits, TaskRetry};

    // ── Helpers ──────────────────────────────────────────────────────────

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
            tasks: serde_json::to_vec(&Vec::<twerk_core::task::Task>::new()).unwrap_or_default(),
            position: 1,
            inputs: serde_json::to_vec(&std::collections::HashMap::<String, String>::new())
                .unwrap_or_default(),
            context: serde_json::to_vec(&JobContext::default()).unwrap_or_default(),
            parent_id: None,
            task_count: 5,
            output_: None,
            result: None,
            error_: None,
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
            id: Some(twerk_core::id::UserId::new("user-001")),
            name: Some("Test User".to_string()),
            username: Some("testuser".to_string()),
            password_hash: Some("hashed".to_string()),
            password: None,
            created_at: Some(fixed_now()),
            disabled: false,
        }
    }

    // ── JobRecord → Job conversion tests ────────────────────────────────

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
        assert_eq!(job.state, JobState::Pending);
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
        let defaults = CoreJobDefaults {
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
                role: Some(twerk_core::role::Role {
                    id: Some(twerk_core::id::RoleId::new("role-pub")),
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
}
