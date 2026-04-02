//! Job operations for `PostgresDatastore`.

use sqlx::Postgres;
use twerk_core::job::{new_job_summary, Job, JobSummary};
use twerk_core::task::Task;
use twerk_core::uuid::new_uuid;

use crate::datastore::postgres::encrypt;
use crate::datastore::postgres::records::{JobPermRecord, JobRecord, JobRecordExt};
use crate::datastore::postgres::{
    Datastore, DatastoreError, DatastoreResult, Executor, Page, PostgresDatastore,
};

impl PostgresDatastore {
    pub(super) async fn create_job_impl(&self, job: &Job) -> DatastoreResult<()> {
        let id = job.id.as_ref().ok_or(DatastoreError::InvalidInput(
            "job ID is required".to_string(),
        ))?;
        let encryption_key = self.encryption_key.clone();
        let tasks = serde_json::to_vec(&job.tasks)
            .map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
        let inputs = serde_json::to_vec(&job.inputs)
            .map_err(|e| DatastoreError::Serialization(format!("job.inputs: {e}")))?;
        let context = serde_json::to_vec(&job.context)
            .map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;
        let defaults: Option<Vec<u8>> = job
            .defaults
            .as_ref()
            .map(|d| {
                serde_json::to_vec(d)
                    .map_err(|e| DatastoreError::Serialization(format!("job.defaults: {e}")))
            })
            .transpose()?;
        let webhooks: Option<Vec<u8>> = job
            .webhooks
            .as_ref()
            .filter(|w| !w.is_empty())
            .map(|w| {
                serde_json::to_vec(w)
                    .map_err(|e| DatastoreError::Serialization(format!("job.webhooks: {e}")))
            })
            .transpose()?;
        let auto_delete: Option<Vec<u8>> = job
            .auto_delete
            .as_ref()
            .map(|d| {
                serde_json::to_vec(d)
                    .map_err(|e| DatastoreError::Serialization(format!("job.auto_delete: {e}")))
            })
            .transpose()?;
        let mut secrets_bytes = None;
        if let Some(secrets) = &job.secrets {
            let encrypted = encrypt::encrypt_secrets(secrets, encryption_key.as_deref())?;
            secrets_bytes = Some(
                serde_json::to_vec(&encrypted)
                    .map_err(|e| DatastoreError::Serialization(format!("job.secrets: {e}")))?,
            );
        }
        let created_by = job.created_by.as_ref().and_then(|u| u.id.clone()).ok_or(
            DatastoreError::InvalidInput("job.created_by.id is required".to_string()),
        )?;

        let q = r"INSERT INTO jobs (id, name, description, tags, state, created_at, created_by, tasks, position, inputs, context, task_count, output_, defaults, webhooks, auto_delete, secrets, progress, scheduled_job_id, started_at, completed_at, failed_at, delete_at, parent_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)";
        let query = sqlx::query(q)
            .bind(&**id)
            .bind(&job.name)
            .bind(&job.description)
            .bind(job.tags.clone().unwrap_or_default())
            .bind(&job.state)
            .bind(job.created_at)
            .bind(&*created_by)
            .bind(&tasks)
            .bind(job.position)
            .bind(&inputs)
            .bind(&context)
            .bind(job.task_count)
            .bind(&job.output)
            .bind(&defaults)
            .bind(&webhooks)
            .bind(&auto_delete)
            .bind(&secrets_bytes)
            .bind(job.progress)
            .bind(
                job.schedule
                    .as_ref()
                    .and_then(|s| s.id.as_ref().map(|s_id| s_id.to_string())),
            )
            .bind(job.started_at)
            .bind(job.completed_at)
            .bind(job.failed_at)
            .bind(job.delete_at)
            .bind(job.parent_id.as_ref().map(|p_id| p_id.as_str()));

        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p
                    .begin()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                query
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create job failed: {e}")))?;
                if let Some(permissions) = &job.permissions {
                    for perm in permissions {
                        let perm_id = new_uuid();
                        sqlx::query("INSERT INTO jobs_perms (id, job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut *tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create job failed: {e}")))?;
                if let Some(permissions) = &job.permissions {
                    for perm in permissions {
                        let perm_id = new_uuid();
                        sqlx::query("INSERT INTO jobs_perms (id, job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut **tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) async fn get_job_by_id_impl(&self, id: &str) -> DatastoreResult<Job> {
        let encryption_key = self.encryption_key.clone();
        let record: JobRecord = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, JobRecord>("SELECT * FROM jobs WHERE id = $1")
                    .bind(id)
                    .fetch_optional(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, JobRecord>("SELECT * FROM jobs WHERE id = $1")
                    .bind(id)
                    .fetch_optional(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))?
        .ok_or(DatastoreError::JobNotFound)?;

        let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
            .map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
        let user = self.get_user(&record.created_by).await?;
        let perms_records: Vec<JobPermRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, JobPermRecord>(
                    "SELECT * FROM jobs_perms WHERE job_id = $1",
                )
                .bind(id)
                .fetch_all(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, JobPermRecord>(
                    "SELECT * FROM jobs_perms WHERE job_id = $1",
                )
                .bind(id)
                .fetch_all(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get perms failed: {e}")))?;

        let mut perms = Vec::new();
        for pr in perms_records {
            let mut p = twerk_core::task::Permission {
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
        record.to_job(tasks, vec![], user, perms, encryption_key.as_deref())
    }

    pub(super) async fn update_job_impl(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Job) -> DatastoreResult<Job> + Send>,
    ) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p
                    .begin()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: JobRecord = sqlx::query_as::<Postgres, JobRecord>(
                    "SELECT * FROM jobs WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))?
                .ok_or(DatastoreError::JobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
                    .map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
                let user = self.get_user(&record.created_by).await?;
                let job =
                    record.to_job(tasks, vec![], user, vec![], self.encryption_key.as_deref())?;
                let job = modify(job)?;
                let context = serde_json::to_vec(&job.context)
                    .map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;
                sqlx::query(r"UPDATE jobs SET state = $1, started_at = $2, completed_at = $3, failed_at = $4, position = $5, context = $6, result = $7, error_ = $8, delete_at = $9, progress = $10, name = $11, description = $12, tags = $13 WHERE id = $14").bind(&job.state).bind(job.started_at).bind(job.completed_at).bind(job.failed_at).bind(job.position).bind(&context).bind(&job.result).bind(&job.error).bind(job.delete_at).bind(job.progress).bind(&job.name).bind(&job.description).bind(job.tags.clone().unwrap_or_default()).bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("update job failed: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: JobRecord = sqlx::query_as::<Postgres, JobRecord>(
                    "SELECT * FROM jobs WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))?
                .ok_or(DatastoreError::JobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks)
                    .map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
                let user = self.get_user(&record.created_by).await?;
                let job =
                    record.to_job(tasks, vec![], user, vec![], self.encryption_key.as_deref())?;
                let job = modify(job)?;
                let context = serde_json::to_vec(&job.context)
                    .map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;
                sqlx::query(r"UPDATE jobs SET state = $1, started_at = $2, completed_at = $3, failed_at = $4, position = $5, context = $6, result = $7, error_ = $8, delete_at = $9, progress = $10, name = $11, description = $12, tags = $13 WHERE id = $14").bind(&job.state).bind(job.started_at).bind(job.completed_at).bind(job.failed_at).bind(job.position).bind(&context).bind(&job.result).bind(&job.error).bind(job.delete_at).bind(job.progress).bind(&job.name).bind(&job.description).bind(job.tags.clone().unwrap_or_default()).bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("update job failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn get_jobs_impl(
        &self,
        current_user: &str,
        q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<JobSummary>> {
        let (search_term, tags) = parse_query_impl(q);
        let offset = (page - 1) * size;
        let records: Vec<JobRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, JobRecord>(
                r#"
                WITH user_info AS (SELECT id AS user_id FROM users WHERE username_ = $3),
                role_info AS (SELECT role_id FROM users_roles ur JOIN user_info ui ON ur.user_id = ui.user_id),
                job_perms_info AS (SELECT job_id FROM jobs_perms jp WHERE jp.user_id = (SELECT user_id FROM user_info) OR jp.role_id IN (SELECT role_id FROM role_info)),
                no_job_perms AS (SELECT j.id as job_id FROM jobs j where not exists (select 1 from jobs_perms jp where j.id = jp.job_id))
                SELECT j.* FROM jobs j WHERE ($1 = '' OR ts @@ plainto_tsquery('english', $1)) AND (coalesce(array_length($2::text[], 1),0) = 0 OR j.tags && $2) AND ($3 = '' OR EXISTS (select 1 from no_job_perms njp where njp.job_id=j.id) OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))
                ORDER BY created_at DESC LIMIT $4 OFFSET $5"#,
            ).bind(&search_term).bind(&tags).bind(current_user).bind(size).bind(offset).fetch_all(p).await,
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, JobRecord>(
                r#"
                WITH user_info AS (SELECT id AS user_id FROM users WHERE username_ = $3),
                role_info AS (SELECT role_id FROM users_roles ur JOIN user_info ui ON ur.user_id = ui.user_id),
                job_perms_info AS (SELECT job_id FROM jobs_perms jp WHERE jp.user_id = (SELECT user_id FROM user_info) OR jp.role_id IN (SELECT role_id FROM role_info)),
                no_job_perms AS (SELECT j.id as job_id FROM jobs j where not exists (select 1 from jobs_perms jp where j.id = jp.job_id))
                SELECT j.* FROM jobs j WHERE ($1 = '' OR ts @@ plainto_tsquery('english', $1)) AND (coalesce(array_length($2::text[], 1),0) = 0 OR j.tags && $2) AND ($3 = '' OR EXISTS (select 1 from no_job_perms njp where njp.job_id=j.id) OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))
                ORDER BY created_at DESC LIMIT $4 OFFSET $5"#,
            ).bind(&search_term).bind(&tags).bind(current_user).bind(size).bind(offset).fetch_all(&mut **tx).await
            }
        }.map_err(|e| DatastoreError::Database(format!("get jobs failed: {e}")))?;

        let mut items = Vec::new();
        for record in records {
            let user = self.get_user(&record.created_by).await?;
            let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).unwrap_or_default();
            let job = record.to_job(tasks, vec![], user, vec![], self.encryption_key.as_deref())?;
            items.push(new_job_summary(&job));
        }

        let total: i64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar(
                r#"
                WITH user_info AS (SELECT id AS user_id FROM users WHERE username_ = $3),
                role_info AS (SELECT role_id FROM users_roles ur JOIN user_info ui ON ur.user_id = ui.user_id),
                job_perms_info AS (SELECT job_id FROM jobs_perms jp WHERE jp.user_id = (SELECT user_id FROM user_info) OR jp.role_id IN (SELECT role_id FROM role_info)),
                no_job_perms AS (SELECT j.id as job_id FROM jobs j where not exists (select 1 from jobs_perms jp where j.id = jp.job_id))
                SELECT count(*) FROM jobs j WHERE ($1 = '' OR ts @@ plainto_tsquery('english', $1)) AND (coalesce(array_length($2::text[], 1),0) = 0 OR j.tags && $2) AND ($3 = '' OR EXISTS (select 1 from no_job_perms njp where njp.job_id=j.id) OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))"#,
            ).bind(&search_term).bind(&tags).bind(current_user).fetch_one(p).await,
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar(
                r#"
                WITH user_info AS (SELECT id AS user_id FROM users WHERE username_ = $3),
                role_info AS (SELECT role_id FROM users_roles ur JOIN user_info ui ON ur.user_id = ui.user_id),
                job_perms_info AS (SELECT job_id FROM jobs_perms jp WHERE jp.user_id = (SELECT user_id FROM user_info) OR jp.role_id IN (SELECT role_id FROM role_info)),
                no_job_perms AS (SELECT j.id as job_id FROM jobs j where not exists (select 1 from jobs_perms jp where j.id = jp.job_id))
                SELECT count(*) FROM jobs j WHERE ($1 = '' OR ts @@ plainto_tsquery('english', $1)) AND (coalesce(array_length($2::text[], 1),0) = 0 OR j.tags && $2) AND ($3 = '' OR EXISTS (select 1 from no_job_perms njp where njp.job_id=j.id) OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))"#,
            ).bind(&search_term).bind(&tags).bind(current_user).fetch_one(&mut **tx).await
            }
        }.map_err(|e| DatastoreError::Database(format!("count jobs failed: {e}")))?;

        Ok(Page {
            items,
            number: page,
            size,
            total_pages: (total as f64 / size as f64).ceil() as i64,
            total_items: total,
        })
    }
}

/// Parses a query string into search terms and tags.
fn parse_query_impl(q: &str) -> (String, Vec<String>) {
    let mut terms = Vec::new();
    let mut tags = Vec::new();
    for part in q.split_whitespace() {
        if let Some(tag) = part.strip_prefix("tag:") {
            tags.push(tag.to_string());
        } else if let Some(tags_str) = part.strip_prefix("tags:") {
            tags.extend(tags_str.split(',').map(|t| t.to_string()));
        } else {
            terms.push(part.to_string());
        }
    }
    (terms.join(" "), tags)
}
