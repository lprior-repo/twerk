//! Scheduled job operations for `PostgresDatastore`.

use sqlx::Postgres;
use twerk_core::job::{ScheduledJob, ScheduledJobSummary};
use twerk_core::task::{Permission, Task};
use twerk_core::uuid::new_uuid;

use crate::datastore::postgres::encrypt;
use crate::datastore::postgres::records::{ScheduledJobRecord, ScheduledJobRecordExt};
use crate::datastore::postgres::{
    Datastore, DatastoreError, DatastoreResult, Executor, Page, PostgresDatastore,
};

impl PostgresDatastore {
    pub(super) async fn create_scheduled_job_impl(&self, sj: &ScheduledJob) -> DatastoreResult<()> {
        let id = sj.id.as_ref().ok_or(DatastoreError::InvalidInput(
            "scheduled job ID is required".to_string(),
        ))?;
        let encryption_key = self.encryption_key.clone();
        let tasks = serde_json::to_vec(&sj.tasks)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
        let inputs = serde_json::to_vec(&sj.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.inputs: {e}")))?;
        let defaults: Option<Vec<u8>> = sj
            .defaults
            .as_ref()
            .map(|d| {
                serde_json::to_vec(d).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.defaults: {e}"))
                })
            })
            .transpose()?;
        let webhooks: Option<Vec<u8>> = sj
            .webhooks
            .as_ref()
            .filter(|w| !w.is_empty())
            .map(|w| {
                serde_json::to_vec(w).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.webhooks: {e}"))
                })
            })
            .transpose()?;
        let auto_delete: Option<Vec<u8>> = sj
            .auto_delete
            .as_ref()
            .map(|d| {
                serde_json::to_vec(d).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.auto_delete: {e}"))
                })
            })
            .transpose()?;
        let mut secrets_bytes = None;
        if let Some(secrets) = &sj.secrets {
            let encrypted = encrypt::encrypt_secrets(secrets, encryption_key.as_deref())?;
            secrets_bytes = Some(serde_json::to_vec(&encrypted).map_err(|e| {
                DatastoreError::Serialization(format!("scheduled_job.secrets: {e}"))
            })?);
        }
        let created_by = sj.created_by.as_ref().and_then(|u| u.id.clone()).ok_or(
            DatastoreError::InvalidInput("scheduled_job.created_by.id is required".to_string()),
        )?;

        let q = r"INSERT INTO scheduled_jobs (id, cron_expr, name, description, tags, state, created_at, created_by, tasks, inputs, output_, defaults, webhooks, auto_delete, secrets) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)";
        let query = sqlx::query(q)
            .bind(&**id)
            .bind(&sj.cron)
            .bind(&sj.name)
            .bind(&sj.description)
            .bind(sj.tags.clone().unwrap_or_default())
            .bind(sj.state.to_string())
            .bind(sj.created_at.unwrap_or_else(time::OffsetDateTime::now_utc))
            .bind(&*created_by)
            .bind(&tasks)
            .bind(&inputs)
            .bind(&sj.output)
            .bind(&defaults)
            .bind(&webhooks)
            .bind(&auto_delete)
            .bind(&secrets_bytes);

        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p
                    .begin()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                query.execute(&mut *tx).await.map_err(|e| {
                    DatastoreError::Database(format!("create scheduled job failed: {e}"))
                })?;
                if let Some(permissions) = &sj.permissions {
                    for perm in permissions {
                        let perm_id = new_uuid();
                        sqlx::query("INSERT INTO scheduled_jobs_perms (id, scheduled_job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut *tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query.execute(&mut **tx).await.map_err(|e| {
                    DatastoreError::Database(format!("create scheduled job failed: {e}"))
                })?;
                if let Some(permissions) = &sj.permissions {
                    for perm in permissions {
                        let perm_id = new_uuid();
                        sqlx::query("INSERT INTO scheduled_jobs_perms (id, scheduled_job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut **tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) async fn get_active_scheduled_jobs_impl(
        &self,
    ) -> DatastoreResult<Vec<ScheduledJob>> {
        let records: Vec<ScheduledJobRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    "SELECT * FROM scheduled_jobs WHERE state = 'ACTIVE' ORDER BY created_at ASC",
                )
                .fetch_all(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    "SELECT * FROM scheduled_jobs WHERE state = 'ACTIVE' ORDER BY created_at ASC",
                )
                .fetch_all(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get active scheduled jobs failed: {e}")))?;

        let mut jobs = Vec::new();
        for record in records {
            let user = self.get_user(&record.created_by).await?;
            let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
                .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
            let perms_records: Vec<ScheduledPermRecord> = match &self.executor {
                Executor::Pool(p) => {
                    sqlx::query_as::<Postgres, ScheduledPermRecord>(
                        "SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id = $1",
                    )
                    .bind(&record.id)
                    .fetch_all(p)
                    .await
                }
                Executor::Tx(tx) => {
                    let mut tx = tx.lock().await;
                    sqlx::query_as::<Postgres, ScheduledPermRecord>(
                        "SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id = $1",
                    )
                    .bind(&record.id)
                    .fetch_all(&mut **tx)
                    .await
                }
            }
            .map_err(|e| DatastoreError::Database(format!("get perms failed: {e}")))?;

            let mut perms = Vec::new();
            for pr in perms_records {
                let mut p = Permission {
                    user: None,
                    role: None,
                };
                if let Some(uid) = &pr.user_id {
                    let u = self.get_user(uid).await?;
                    p.user = Some(u);
                }
                if let Some(rid) = &pr.role_id {
                    let r = self.get_role(rid).await?;
                    p.role = Some(r);
                }
                perms.push(p);
            }

            let job =
                record.to_scheduled_job(tasks, user, perms, self.encryption_key.as_deref())?;
            jobs.push(job);
        }
        Ok(jobs)
    }

    pub(super) async fn get_scheduled_jobs_impl(
        &self,
        current_user: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<ScheduledJobSummary>> {
        let offset = (page - 1) * size;
        let records: Vec<ScheduledJobRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    r#"
                    SELECT sj.* FROM scheduled_jobs sj
                    WHERE NOT EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                    )
                    OR EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                        AND (
                            sjp.user_id = (SELECT id FROM users WHERE username_ = $3)
                            OR sjp.role_id IN (SELECT role_id FROM users_roles WHERE user_id = (SELECT id FROM users WHERE username_ = $3))
                        )
                    )
                    ORDER BY created_at DESC
                    LIMIT $1 OFFSET $2
                    "#,
                )
                .bind(size)
                .bind(offset)
                .bind(current_user)
                .fetch_all(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    r#"
                    SELECT sj.* FROM scheduled_jobs sj
                    WHERE NOT EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                    )
                    OR EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                        AND (
                            sjp.user_id = (SELECT id FROM users WHERE username_ = $3)
                            OR sjp.role_id IN (SELECT role_id FROM users_roles WHERE user_id = (SELECT id FROM users WHERE username_ = $3))
                        )
                    )
                    ORDER BY created_at DESC
                    LIMIT $1 OFFSET $2
                    "#,
                )
                .bind(size)
                .bind(offset)
                .bind(current_user)
                .fetch_all(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get scheduled jobs failed: {e}")))?;

        let mut items = Vec::new();
        for record in records {
            let user = self.get_user(&record.created_by).await?;
            let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
                .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
            let job =
                record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
            items.push(twerk_core::job::new_scheduled_job_summary(&job));
        }

        let total: i64 = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_scalar(
                    r#"
                    SELECT count(*) FROM scheduled_jobs sj
                    WHERE NOT EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                    )
                    OR EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                        AND (
                            sjp.user_id = (SELECT id FROM users WHERE username_ = $1)
                            OR sjp.role_id IN (SELECT role_id FROM users_roles WHERE user_id = (SELECT id FROM users WHERE username_ = $1))
                        )
                    )
                    "#,
                )
                .bind(current_user)
                .fetch_one(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar(
                    r#"
                    SELECT count(*) FROM scheduled_jobs sj
                    WHERE NOT EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                    )
                    OR EXISTS (
                        SELECT 1 FROM scheduled_jobs_perms sjp
                        WHERE sjp.scheduled_job_id = sj.id
                        AND (
                            sjp.user_id = (SELECT id FROM users WHERE username_ = $1)
                            OR sjp.role_id IN (SELECT role_id FROM users_roles WHERE user_id = (SELECT id FROM users WHERE username_ = $1))
                        )
                    )
                    "#,
                )
                .bind(current_user)
                .fetch_one(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("count scheduled jobs failed: {e}")))?;

        Ok(Page {
            items,
            number: page,
            size,
            total_pages: (total as f64 / size as f64).ceil() as i64,
            total_items: total,
        })
    }

    pub(super) async fn get_scheduled_job_by_id_impl(
        &self,
        id: &str,
    ) -> DatastoreResult<ScheduledJob> {
        let encryption_key = self.encryption_key.clone();
        let record: ScheduledJobRecord = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    "SELECT * FROM scheduled_jobs WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    "SELECT * FROM scheduled_jobs WHERE id = $1",
                )
                .bind(id)
                .fetch_optional(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get scheduled job failed: {e}")))?
        .ok_or(DatastoreError::ScheduledJobNotFound)?;

        let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
            .map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
        let user = self.get_user(&record.created_by).await?;
        let perms_records: Vec<ScheduledPermRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, ScheduledPermRecord>(
                    "SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id = $1",
                )
                .bind(id)
                .fetch_all(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, ScheduledPermRecord>(
                    "SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id = $1",
                )
                .bind(id)
                .fetch_all(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get perms failed: {e}")))?;

        let mut perms = Vec::new();
        for pr in perms_records {
            let mut p = Permission {
                user: None,
                role: None,
            };
            if let Some(uid) = &pr.user_id {
                let u = self.get_user(uid).await?;
                p.user = Some(u);
            }
            if let Some(rid) = &pr.role_id {
                let r = self.get_role(rid).await?;
                p.role = Some(r);
            }
            perms.push(p);
        }

        record.to_scheduled_job(tasks, user, perms, encryption_key.as_deref())
    }

    pub(super) async fn update_scheduled_job_impl(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(ScheduledJob) -> DatastoreResult<ScheduledJob> + Send>,
    ) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p
                    .begin()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: ScheduledJobRecord = sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    "SELECT * FROM scheduled_jobs WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get scheduled job failed: {e}")))?
                .ok_or(DatastoreError::ScheduledJobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.tasks: {e}"))
                })?;
                let user = self.get_user(&record.created_by).await?;
                let sj =
                    record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
                let sj = modify(sj)?;
                let cron_expr = &sj.cron;
                let name = &sj.name;
                let description = &sj.description;
                let tags = sj.tags.clone().unwrap_or_default();
                let state = sj.state.to_string();
                let defaults: Option<Vec<u8>> = sj
                    .defaults
                    .as_ref()
                    .map(|d| {
                        serde_json::to_vec(d).map_err(|e| {
                            DatastoreError::Serialization(format!("scheduled_job.defaults: {e}"))
                        })
                    })
                    .transpose()?;
                let webhooks: Option<Vec<u8>> = sj
                    .webhooks
                    .as_ref()
                    .filter(|w| !w.is_empty())
                    .map(|w| {
                        serde_json::to_vec(w).map_err(|e| {
                            DatastoreError::Serialization(format!("scheduled_job.webhooks: {e}"))
                        })
                    })
                    .transpose()?;
                let auto_delete: Option<Vec<u8>> = sj
                    .auto_delete
                    .as_ref()
                    .map(|d| {
                        serde_json::to_vec(d).map_err(|e| {
                            DatastoreError::Serialization(format!("scheduled_job.auto_delete: {e}"))
                        })
                    })
                    .transpose()?;
                sqlx::query(r"UPDATE scheduled_jobs SET cron_expr = $1, name = $2, description = $3, tags = $4, state = $5, defaults = $6, webhooks = $7, auto_delete = $8 WHERE id = $9")
                    .bind(cron_expr)
                    .bind(name)
                    .bind(description)
                    .bind(&tags)
                    .bind(state)
                    .bind(&defaults)
                    .bind(&webhooks)
                    .bind(&auto_delete)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("update scheduled job failed: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: ScheduledJobRecord = sqlx::query_as::<Postgres, ScheduledJobRecord>(
                    "SELECT * FROM scheduled_jobs WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get scheduled job failed: {e}")))?
                .ok_or(DatastoreError::ScheduledJobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| {
                    DatastoreError::Serialization(format!("scheduled_job.tasks: {e}"))
                })?;
                let user = self.get_user(&record.created_by).await?;
                let sj =
                    record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
                let sj = modify(sj)?;
                let cron_expr = &sj.cron;
                let name = &sj.name;
                let description = &sj.description;
                let tags = sj.tags.clone().unwrap_or_default();
                let state = sj.state.to_string();
                let defaults: Option<Vec<u8>> = sj
                    .defaults
                    .as_ref()
                    .map(|d| {
                        serde_json::to_vec(d).map_err(|e| {
                            DatastoreError::Serialization(format!("scheduled_job.defaults: {e}"))
                        })
                    })
                    .transpose()?;
                let webhooks: Option<Vec<u8>> = sj
                    .webhooks
                    .as_ref()
                    .filter(|w| !w.is_empty())
                    .map(|w| {
                        serde_json::to_vec(w).map_err(|e| {
                            DatastoreError::Serialization(format!("scheduled_job.webhooks: {e}"))
                        })
                    })
                    .transpose()?;
                let auto_delete: Option<Vec<u8>> = sj
                    .auto_delete
                    .as_ref()
                    .map(|d| {
                        serde_json::to_vec(d).map_err(|e| {
                            DatastoreError::Serialization(format!("scheduled_job.auto_delete: {e}"))
                        })
                    })
                    .transpose()?;
                sqlx::query(r"UPDATE scheduled_jobs SET cron_expr = $1, name = $2, description = $3, tags = $4, state = $5, defaults = $6, webhooks = $7, auto_delete = $8 WHERE id = $9")
                    .bind(cron_expr)
                    .bind(name)
                    .bind(description)
                    .bind(&tags)
                    .bind(state)
                    .bind(&defaults)
                    .bind(&webhooks)
                    .bind(&auto_delete)
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("update scheduled job failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn delete_scheduled_job_impl(&self, id: &str) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p
                    .begin()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                sqlx::query("DELETE FROM scheduled_jobs_perms WHERE scheduled_job_id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        DatastoreError::Database(format!("delete scheduled job perms failed: {e}"))
                    })?;
                sqlx::query("DELETE FROM scheduled_jobs WHERE id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        DatastoreError::Database(format!("delete scheduled job failed: {e}"))
                    })?;
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query("DELETE FROM scheduled_jobs_perms WHERE scheduled_job_id = $1")
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| {
                        DatastoreError::Database(format!("delete scheduled job perms failed: {e}"))
                    })?;
                sqlx::query("DELETE FROM scheduled_jobs WHERE id = $1")
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| {
                        DatastoreError::Database(format!("delete scheduled job failed: {e}"))
                    })?;
            }
        }
        Ok(())
    }
}

/// Scheduled permission record from the database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScheduledPermRecord {
    pub id: String,
    pub scheduled_job_id: String,
    pub user_id: Option<String>,
    pub role_id: Option<String>,
    pub created_at: Option<time::OffsetDateTime>,
}
