//! Job operations for `PostgresDatastore`.

use sqlx::Postgres;
use time::OffsetDateTime;
use twerk_core::job::{new_job_summary, Job, JobSummary};
use twerk_core::task::{Permission, Task};
use twerk_core::uuid::new_uuid;

use crate::datastore::postgres::encrypt;
use crate::datastore::postgres::records::{JobPermRecord, JobRecord, JobRecordExt};
use crate::datastore::postgres::{
    DatastoreError, DatastoreResult, Executor, Page, PostgresDatastore,
};

// ── SQL constants ──────────────────────────────────────────────────────────

const SQL_INSERT_JOB: &str = r"
INSERT INTO jobs (
    id, name, description, tags, state, created_at, created_by,
    tasks, position, inputs, context, task_count, output_,
    defaults, webhooks, auto_delete, secrets, progress,
    scheduled_job_id, started_at, completed_at, failed_at,
    delete_at, parent_id
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
    $21, $22, $23, $24
)";

const SQL_INSERT_PERM: &str = r"
INSERT INTO jobs_perms (id, job_id, user_id, role_id)
VALUES (
    $1, $2,
    CASE WHEN $3::varchar IS NOT NULL
         THEN coalesce((SELECT id FROM users WHERE username_ = $3), $3)
         ELSE NULL
    END,
    CASE WHEN $4::varchar IS NOT NULL
         THEN coalesce((SELECT id FROM roles WHERE slug = $4), $4)
         ELSE NULL
    END
)";

const SQL_GET_JOB: &str = "SELECT * FROM jobs WHERE id = $1";
const SQL_GET_JOB_FOR_UPDATE: &str = "SELECT * FROM jobs WHERE id = $1 FOR UPDATE";
const SQL_GET_JOB_PERMS: &str = "SELECT * FROM jobs_perms WHERE job_id = $1";

const SQL_UPDATE_JOB: &str = r"
UPDATE jobs SET
    state = $1, started_at = $2, completed_at = $3, failed_at = $4,
    position = $5, context = $6, result = $7, error_ = $8,
    delete_at = $9, progress = $10, name = $11, description = $12,
    tags = $13
WHERE id = $14
";

const SQL_SEARCH_JOBS: &str = r#"
WITH user_info AS (SELECT id AS user_id FROM users WHERE username_ = $3),
role_info AS (
    SELECT role_id FROM users_roles ur
    JOIN user_info ui ON ur.user_id = ui.user_id
),
job_perms_info AS (
    SELECT job_id FROM jobs_perms jp
    WHERE jp.user_id = (SELECT user_id FROM user_info)
       OR jp.role_id IN (SELECT role_id FROM role_info)
),
no_job_perms AS (
    SELECT j.id as job_id FROM jobs j
    WHERE NOT EXISTS (SELECT 1 FROM jobs_perms jp WHERE j.id = jp.job_id)
)
SELECT j.* FROM jobs j
WHERE ($1 = '' OR ts @@ plainto_tsquery('english', $1))
  AND (coalesce(array_length($2::text[], 1), 0) = 0 OR j.tags && $2)
  AND ($3 = ''
       OR EXISTS (SELECT 1 FROM no_job_perms njp WHERE njp.job_id = j.id)
       OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))
ORDER BY created_at DESC
LIMIT $4 OFFSET $5
"#;

const SQL_COUNT_JOBS: &str = r#"
WITH user_info AS (SELECT id AS user_id FROM users WHERE username_ = $3),
role_info AS (
    SELECT role_id FROM users_roles ur
    JOIN user_info ui ON ur.user_id = ui.user_id
),
job_perms_info AS (
    SELECT job_id FROM jobs_perms jp
    WHERE jp.user_id = (SELECT user_id FROM user_info)
       OR jp.role_id IN (SELECT role_id FROM role_info)
),
no_job_perms AS (
    SELECT j.id as job_id FROM jobs j
    WHERE NOT EXISTS (SELECT 1 FROM jobs_perms jp WHERE j.id = jp.job_id)
)
SELECT count(*) FROM jobs j
WHERE ($1 = '' OR ts @@ plainto_tsquery('english', $1))
  AND (coalesce(array_length($2::text[], 1), 0) = 0 OR j.tags && $2)
  AND ($3 = ''
       OR EXISTS (SELECT 1 FROM no_job_perms njp WHERE njp.job_id = j.id)
       OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.job_id = j.id))
"#;

// ── Pure calculations ──────────────────────────────────────────────────────

fn classify_perm_error(e: sqlx::Error) -> DatastoreError {
    let msg = e.to_string();
    if msg.contains("_user_id_fkey") {
        DatastoreError::UserNotFound
    } else if msg.contains("_role_id_fkey") {
        DatastoreError::RoleNotFound
    } else {
        DatastoreError::Database(format!("assign role failed: {e}"))
    }
}

fn deserialize_tasks(raw: &[u8]) -> DatastoreResult<Vec<Task>> {
    serde_json::from_slice(raw)
        .map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))
}

fn parse_query_impl(q: &str) -> (String, Vec<String>) {
    let tags: Vec<String> = q
        .split_whitespace()
        .filter_map(|part| {
            part.strip_prefix("tag:")
                .map(|tag| vec![tag.to_string()])
                .or_else(|| {
                    part.strip_prefix("tags:")
                        .map(|s| s.split(',').map(|t| t.to_string()).collect())
                })
        })
        .flatten()
        .collect();

    let search: String = q
        .split_whitespace()
        .filter(|part| !part.starts_with("tag:") && !part.starts_with("tags:"))
        .collect::<Vec<_>>()
        .join(" ");

    (search, tags)
}

fn serialize_json_field<T: serde::Serialize>(
    field: &Option<T>,
    label: &str,
) -> DatastoreResult<Option<Vec<u8>>> {
    field
        .as_ref()
        .map(|v| {
            serde_json::to_vec(v)
                .map_err(|e| DatastoreError::Serialization(format!("job.{label}: {e}")))
        })
        .transpose()
}

// ── Executor dispatch helpers ──────────────────────────────────────────────

async fn insert_job_perms(
    conn: &mut sqlx::postgres::PgConnection,
    job_id: &str,
    permissions: &[Permission],
) -> DatastoreResult<()> {
    #[allow(unknown_lints, clippy::imperative_loops)]
    for perm in permissions {
        let perm_id = new_uuid();
        sqlx::query(SQL_INSERT_PERM)
            .bind(&perm_id)
            .bind(job_id)
            .bind(perm.user.as_ref().and_then(|u| u.username.as_ref()))
            .bind(perm.role.as_ref().and_then(|r| r.slug.as_ref()))
            .execute(&mut *conn)
            .await
            .map_err(classify_perm_error)?;
    }
    Ok(())
}

async fn fetch_job_record(
    executor: &Executor,
    sql: &str,
    id: &str,
) -> DatastoreResult<Option<JobRecord>> {
    match executor {
        Executor::Pool(p) => {
            sqlx::query_as::<Postgres, JobRecord>(sql)
                .bind(id)
                .fetch_optional(p)
                .await
        }
        Executor::Tx(tx) => {
            let mut tx = tx.lock().await;
            sqlx::query_as::<Postgres, JobRecord>(sql)
                .bind(id)
                .fetch_optional(&mut **tx)
                .await
        }
    }
    .map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))
}

async fn fetch_job_perms(executor: &Executor, job_id: &str) -> DatastoreResult<Vec<JobPermRecord>> {
    match executor {
        Executor::Pool(p) => {
            sqlx::query_as::<Postgres, JobPermRecord>(SQL_GET_JOB_PERMS)
                .bind(job_id)
                .fetch_all(p)
                .await
        }
        Executor::Tx(tx) => {
            let mut tx = tx.lock().await;
            sqlx::query_as::<Postgres, JobPermRecord>(SQL_GET_JOB_PERMS)
                .bind(job_id)
                .fetch_all(&mut **tx)
                .await
        }
    }
    .map_err(|e| DatastoreError::Database(format!("get perms failed: {e}")))
}

async fn fetch_jobs_search(
    executor: &Executor,
    search_term: &str,
    tags: &[String],
    current_user: &str,
    size: i64,
    offset: i64,
) -> DatastoreResult<Vec<JobRecord>> {
    match executor {
        Executor::Pool(p) => {
            sqlx::query_as::<Postgres, JobRecord>(SQL_SEARCH_JOBS)
                .bind(search_term)
                .bind(tags)
                .bind(current_user)
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await
        }
        Executor::Tx(tx) => {
            let mut tx = tx.lock().await;
            sqlx::query_as::<Postgres, JobRecord>(SQL_SEARCH_JOBS)
                .bind(search_term)
                .bind(tags)
                .bind(current_user)
                .bind(size)
                .bind(offset)
                .fetch_all(&mut **tx)
                .await
        }
    }
    .map_err(|e| DatastoreError::Database(format!("get jobs failed: {e}")))
}

async fn count_jobs_search(
    executor: &Executor,
    search_term: &str,
    tags: &[String],
    current_user: &str,
) -> DatastoreResult<i64> {
    match executor {
        Executor::Pool(p) => {
            sqlx::query_scalar(SQL_COUNT_JOBS)
                .bind(search_term)
                .bind(tags)
                .bind(current_user)
                .fetch_one(p)
                .await
        }
        Executor::Tx(tx) => {
            let mut tx = tx.lock().await;
            sqlx::query_scalar(SQL_COUNT_JOBS)
                .bind(search_term)
                .bind(tags)
                .bind(current_user)
                .fetch_one(&mut **tx)
                .await
        }
    }
    .map_err(|e| DatastoreError::Database(format!("count jobs failed: {e}")))
}

// ── Orchestration helpers ──────────────────────────────────────────────────

async fn resolve_permissions(
    ds: &PostgresDatastore,
    perm_records: Vec<JobPermRecord>,
) -> DatastoreResult<Vec<Permission>> {
    futures_util::future::try_join_all(perm_records.into_iter().map(|pr| async move {
        let user = match pr.user_id.as_ref() {
            Some(uid) => Some(ds.get_user_impl(uid).await?),
            None => None,
        };
        let role = match pr.role_id.as_ref() {
            Some(rid) => Some(ds.get_role_impl(rid).await?),
            None => None,
        };
        Ok(Permission { user, role })
    }))
    .await
}

#[allow(clippy::explicit_auto_deref)]
async fn apply_job_update(
    conn: &mut sqlx::postgres::PgConnection,
    ds: &PostgresDatastore,
    id: &str,
    modify: Box<dyn FnOnce(Job) -> DatastoreResult<Job> + Send>,
) -> DatastoreResult<()> {
    let record: JobRecord = sqlx::query_as::<Postgres, JobRecord>(SQL_GET_JOB_FOR_UPDATE)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))?
        .ok_or(DatastoreError::JobNotFound)?;

    let tasks = deserialize_tasks(&record.tasks)?;
    let user = ds.get_user_impl(&record.created_by).await?;
    let job = record.to_job(tasks, vec![], user, vec![], ds.encryption_key.as_deref())?;
    let job = modify(job)?;

    let context = serde_json::to_vec(&job.context)
        .map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;

    sqlx::query(SQL_UPDATE_JOB)
        .bind(&job.state)
        .bind(job.started_at)
        .bind(job.completed_at)
        .bind(job.failed_at)
        .bind(job.position)
        .bind(&context)
        .bind(&job.result)
        .bind(&job.error)
        .bind(job.delete_at)
        .bind(job.progress)
        .bind(&job.name)
        .bind(&job.description)
        .bind(job.tags.as_ref().map_or_else(Vec::new, Clone::clone))
        .bind(id)
        .execute(conn)
        .await
        .map_err(|e| DatastoreError::Database(format!("update job failed: {e}")))?;

    Ok(())
}

// ── PostgresDatastore methods ──────────────────────────────────────────────

impl PostgresDatastore {
    #[allow(clippy::explicit_auto_deref)]
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
        let defaults = serialize_json_field(&job.defaults, "defaults")?;
        let webhooks: Option<Vec<u8>> = job
            .webhooks
            .as_ref()
            .filter(|w| !w.is_empty())
            .map(|w| {
                serde_json::to_vec(w)
                    .map_err(|e| DatastoreError::Serialization(format!("job.webhooks: {e}")))
            })
            .transpose()?;
        let auto_delete = serialize_json_field(&job.auto_delete, "auto_delete")?;
        let secrets_bytes: Option<Vec<u8>> = job
            .secrets
            .as_ref()
            .map(|secrets| -> DatastoreResult<Vec<u8>> {
                let encrypted = encrypt::encrypt_secrets(secrets, encryption_key.as_deref())?;
                serde_json::to_vec(&encrypted)
                    .map_err(|e| DatastoreError::Serialization(format!("job.secrets: {e}")))
            })
            .transpose()?;
        let created_by = job.created_by.as_ref().and_then(|u| u.id.clone()).ok_or(
            DatastoreError::InvalidInput("job.created_by.id is required".to_string()),
        )?;

        let query = sqlx::query(SQL_INSERT_JOB)
            .bind(&**id)
            .bind(&job.name)
            .bind(&job.description)
            .bind(job.tags.as_ref().map_or_else(Vec::new, Clone::clone))
            .bind(&job.state)
            .bind(match job.created_at {
                Some(t) => t,
                None => OffsetDateTime::now_utc(),
            })
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
                    insert_job_perms(&mut *tx, id, permissions).await?;
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
                    insert_job_perms(&mut **tx, id, permissions).await?;
                }
            }
        }
        Ok(())
    }

    pub(super) async fn get_job_by_id_impl(&self, id: &str) -> DatastoreResult<Job> {
        let encryption_key = self.encryption_key.clone();
        let record = fetch_job_record(&self.executor, SQL_GET_JOB, id)
            .await?
            .ok_or(DatastoreError::JobNotFound)?;

        let tasks = deserialize_tasks(&record.tasks)?;
        let user = self.get_user_impl(&record.created_by).await?;
        let perms_records = fetch_job_perms(&self.executor, id).await?;
        let perms = resolve_permissions(self, perms_records).await?;

        record.to_job(tasks, vec![], user, perms, encryption_key.as_deref())
    }

    #[allow(clippy::explicit_auto_deref)]
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
                apply_job_update(&mut *tx, self, id, modify).await?;
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                apply_job_update(&mut **tx, self, id, modify).await?;
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

        let records = fetch_jobs_search(
            &self.executor,
            &search_term,
            &tags,
            current_user,
            size,
            offset,
        )
        .await?;

        let encryption_key = self.encryption_key.as_deref();
        let items: Vec<JobSummary> =
            futures_util::future::try_join_all(records.into_iter().map(|record| async move {
                let user = self.get_user_impl(&record.created_by).await?;
                let tasks = deserialize_tasks(&record.tasks)?;
                let job = record.to_job(tasks, vec![], user, vec![], encryption_key)?;
                Ok(new_job_summary(&job))
            }))
            .await?;

        let total = count_jobs_search(&self.executor, &search_term, &tags, current_user).await?;

        Ok(Page {
            items,
            number: page,
            size,
            total_pages: (total as f64 / size as f64).ceil() as i64,
            total_items: total,
        })
    }
}
