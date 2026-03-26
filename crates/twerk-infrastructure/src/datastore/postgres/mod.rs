//! `PostgreSQL` implementation of the datastore.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

pub use super::{Datastore, Error as DatastoreError, Options, Page, Result as DatastoreResult};
pub mod encrypt;
pub mod records;
pub mod schema;

pub use schema::SCHEMA;

use records::{
    JobPermRecord, JobRecord, NodeRecord, RoleRecord, ScheduledJobRecord, ScheduledPermRecord,
    TaskLogPartRecord, TaskRecord, UserRecord,
};
use records::{
    JobRecordExt, NodeRecordExt, RoleRecordExt, ScheduledJobRecordExt, TaskLogPartRecordExt,
    TaskRecordExt, UserRecordExt,
};
use twerk_core::job::{
    Job, JobSummary, ScheduledJob, ScheduledJobSummary, JOB_STATE_COMPLETED, JOB_STATE_FAILED,
};
use twerk_core::node::{Node, LAST_HEARTBEAT_TIMEOUT};
use twerk_core::role::Role;
use twerk_core::task::{Permission, Task, TaskLogPart};
use twerk_core::user::User;

/// Minimal cleanup interval (1 minute)
pub const MIN_CLEANUP_INTERVAL: Duration = Duration::minutes(1);
/// Maximum cleanup interval (1 hour)
pub const MAX_CLEANUP_INTERVAL: Duration = Duration::hours(1);

/// Default retention period for logs (1 week)
pub const DEFAULT_LOGS_RETENTION_DURATION: Duration = Duration::days(7);
/// Default retention period for jobs (1 year)
pub const DEFAULT_JOBS_RETENTION_DURATION: Duration = Duration::days(365);

#[derive(Clone, Debug)]
enum Executor {
    Pool(PgPool),
    Tx(Arc<Mutex<Transaction<'static, Postgres>>>),
}

/// `PostgresDatastore` implements the `Datastore` trait using `PostgreSQL`.
#[derive(Clone, Debug)]
pub struct PostgresDatastore {
    executor: Executor,
    logs_retention_duration: Duration,
    jobs_retention_duration: Duration,
    cleanup_interval_ms: Arc<AtomicI64>,
    disable_cleanup: bool,
    encryption_key: Option<String>,
}

impl PostgresDatastore {
    /// Creates a new `PostgresDatastore` from a connection string.
    pub async fn new(dsn: &str, options: Options) -> DatastoreResult<Self> {
        let mut pool_options = PgPoolOptions::new();
        if let Some(max_conns) = options.max_open_conns { pool_options = pool_options.max_connections(max_conns as u32); }
        if let Some(max_idle) = options.max_idle_conns { pool_options = pool_options.min_connections(max_idle as u32); }
        if let Some(lifetime) = options.conn_max_lifetime { pool_options = pool_options.max_lifetime(StdDuration::from_secs(lifetime.whole_seconds().unsigned_abs())); }
        if let Some(idle_time) = options.conn_max_idle_time { pool_options = pool_options.idle_timeout(StdDuration::from_secs(idle_time.whole_seconds().unsigned_abs())); }
        let pool = pool_options.connect(dsn).await.map_err(|e| DatastoreError::Database(format!("connection failed: {e}")))?;
        let cleanup_interval = if options.cleanup_interval < MIN_CLEANUP_INTERVAL { MIN_CLEANUP_INTERVAL } else { options.cleanup_interval };
        let logs_retention_duration = if options.logs_retention_duration < MIN_CLEANUP_INTERVAL { MIN_CLEANUP_INTERVAL } else { options.logs_retention_duration };
        let jobs_retention_duration = if options.jobs_retention_duration < MIN_CLEANUP_INTERVAL { MIN_CLEANUP_INTERVAL } else { options.jobs_retention_duration };
        #[allow(clippy::cast_possible_truncation)]
        let cleanup_interval_ms = cleanup_interval.whole_milliseconds() as i64;
        Ok(Self { executor: Executor::Pool(pool), logs_retention_duration, jobs_retention_duration, cleanup_interval_ms: Arc::new(AtomicI64::new(cleanup_interval_ms)), disable_cleanup: options.disable_cleanup, encryption_key: options.encryption_key })
    }

    /// Returns the underlying database pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        match &self.executor {
            Executor::Pool(p) => p,
            Executor::Tx(_) => panic!("cannot get pool from transaction"),
        }
    }

    pub async fn close(&self) -> DatastoreResult<()> {
        if let Executor::Pool(p) = &self.executor {
            p.close().await;
        }
        Ok(())
    }

    pub fn spawn_cleanup(self) {
        if self.disable_cleanup { return; }
        let ds = self.clone();
        tokio::spawn(async move { ds.cleanup_process().await; });
    }

    async fn cleanup_process(&self) {
        loop {
            let interval_ms = self.cleanup_interval_ms.load(Ordering::Relaxed);
            tokio::time::sleep(StdDuration::from_millis(interval_ms as u64)).await;
            match self.cleanup().await {
                Ok(count) => {
                    if count > 0 {
                        let mut new_val = interval_ms / 2;
                        if new_val < MIN_CLEANUP_INTERVAL.whole_milliseconds() as i64 { new_val = MIN_CLEANUP_INTERVAL.whole_milliseconds() as i64; }
                        self.cleanup_interval_ms.store(new_val, Ordering::Relaxed);
                    } else {
                        let mut new_val = interval_ms * 2;
                        if new_val > MAX_CLEANUP_INTERVAL.whole_milliseconds() as i64 { new_val = MAX_CLEANUP_INTERVAL.whole_milliseconds() as i64; }
                        self.cleanup_interval_ms.store(new_val, Ordering::Relaxed);
                    }
                }
                Err(e) => { tracing::error!("cleanup error: {e}"); }
            }
        }
    }

    pub async fn cleanup(&self) -> DatastoreResult<i64> {
        let mut total = 0; total += self.cleanup_logs().await?; total += self.cleanup_jobs().await?; Ok(total)
    }

    async fn cleanup_logs(&self) -> DatastoreResult<i64> {
        let cutoff = OffsetDateTime::now_utc() - self.logs_retention_duration;
        let pool = self.pool();
        let result = sqlx::query("DELETE FROM tasks_log_parts WHERE created_at < $1").bind(cutoff).execute(pool).await.map_err(|e| DatastoreError::Database(format!("cleanup logs failed: {e}")))?;
        Ok(result.rows_affected() as i64)
    }

    async fn cleanup_jobs(&self) -> DatastoreResult<i64> {
        let cutoff = OffsetDateTime::now_utc() - self.jobs_retention_duration;
        let pool = self.pool();
        let result = sqlx::query("DELETE FROM jobs WHERE state IN ($1, $2) AND (delete_at IS NOT NULL AND delete_at < $3 OR (delete_at IS NULL AND (completed_at < $4 OR failed_at < $4)))").bind(JOB_STATE_COMPLETED).bind(JOB_STATE_FAILED).bind(OffsetDateTime::now_utc()).bind(cutoff).execute(pool).await.map_err(|e| DatastoreError::Database(format!("cleanup jobs failed: {e}")))?;
        Ok(result.rows_affected() as i64)
    }

    pub async fn exec_script(&self, script: &str) -> DatastoreResult<()> {
        let pool = self.pool();
        for stmt in script.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() { continue; }
            sqlx::query(trimmed).execute(pool).await.map_err(|e| DatastoreError::Database(format!("exec script failed: {e}")))?;
        }
        Ok(())
    }
}

#[async_trait]
impl Datastore for PostgresDatastore {
    async fn create_job(&self, job: &Job) -> DatastoreResult<()> {
        let id = job.id.as_ref().ok_or(DatastoreError::InvalidInput("job ID is required".to_string()))?;
        let encryption_key = self.encryption_key.clone();
        let tasks = serde_json::to_vec(&job.tasks).map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
        let inputs = serde_json::to_vec(&job.inputs).map_err(|e| DatastoreError::Serialization(format!("job.inputs: {e}")))?;
        let context = serde_json::to_vec(&job.context).map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;
        let defaults: Option<Vec<u8>> = job.defaults.as_ref().map(|d| serde_json::to_vec(d).map_err(|e| DatastoreError::Serialization(format!("job.defaults: {e}")))).transpose()?;
        let webhooks: Option<Vec<u8>> = job.webhooks.as_ref().filter(|w| !w.is_empty()).map(|w| serde_json::to_vec(w).map_err(|e| DatastoreError::Serialization(format!("job.webhooks: {e}")))).transpose()?;
        let auto_delete: Option<Vec<u8>> = job.auto_delete.as_ref().map(|d| serde_json::to_vec(d).map_err(|e| DatastoreError::Serialization(format!("job.auto_delete: {e}")))).transpose()?;
        let mut secrets_bytes = None;
        if let Some(secrets) = &job.secrets {
            let encrypted = encrypt::encrypt_secrets(secrets, encryption_key.as_deref())?;
            secrets_bytes = Some(serde_json::to_vec(&encrypted).map_err(|e| DatastoreError::Serialization(format!("job.secrets: {e}")))?,);
        }
        let created_by = job.created_by.as_ref().and_then(|u| u.id.clone()).ok_or(DatastoreError::InvalidInput("job.created_by.id is required".to_string(),))?;
        
        let q = r"INSERT INTO jobs (id, name, description, tags, state, created_at, created_by, tasks, position, inputs, context, task_count, output_, defaults, webhooks, auto_delete, secrets, progress, scheduled_job_id, started_at, completed_at, failed_at, delete_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)";
        let query = sqlx::query(q).bind(&**id).bind(&job.name).bind(&job.description).bind(&job.tags).bind(&job.state).bind(job.created_at).bind(&*created_by).bind(&tasks).bind(job.position).bind(&inputs).bind(&context).bind(job.task_count).bind(&job.output).bind(&defaults).bind(&webhooks).bind(&auto_delete).bind(&secrets_bytes).bind(job.progress).bind(job.schedule.as_ref().and_then(|s| s.id.as_ref().map(|id| id.to_string()))).bind(job.started_at).bind(job.completed_at).bind(job.failed_at).bind(job.delete_at);

        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                query.execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("create job failed: {e}")))?;
                if let Some(permissions) = &job.permissions {
                    for perm in permissions {
                        let perm_id = uuid::Uuid::new_v4().to_string().replace('-', "");
                        sqlx::query("INSERT INTO jobs_perms (id, job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut *tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("create job failed: {e}")))?;
                if let Some(permissions) = &job.permissions {
                    for perm in permissions {
                        let perm_id = uuid::Uuid::new_v4().to_string().replace('-', "");
                        sqlx::query("INSERT INTO jobs_perms (id, job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut **tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn get_job_by_id(&self, id: &str) -> DatastoreResult<Job> {
        let encryption_key = self.encryption_key.clone();
        let record: JobRecord = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, JobRecord>("SELECT * FROM jobs WHERE id = $1").bind(id).fetch_optional(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, JobRecord>("SELECT * FROM jobs WHERE id = $1").bind(id).fetch_optional(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))? .ok_or(DatastoreError::JobNotFound)?;
        
        let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
        let user = self.get_user(&record.created_by).await?;
        let perms_records: Vec<JobPermRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, JobPermRecord>("SELECT * FROM jobs_perms WHERE job_id = $1").bind(id).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, JobPermRecord>("SELECT * FROM jobs_perms WHERE job_id = $1").bind(id).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get perms failed: {e}")))?;
        
        let mut perms = Vec::new();
        for pr in perms_records {
            let mut p = Permission { user: None, role: None };
            if let Some(uid) = &pr.user_id { let u = self.get_user(uid).await?; p.user = Some(u); }
            if let Some(rid) = &pr.role_id { let r = self.get_role(rid).await?; p.role = Some(r); }
            perms.push(p);
        }
        record.to_job(tasks, vec![], user, perms, encryption_key.as_deref())
    }

    async fn update_job(&self, id: &str, modify: Box<dyn FnOnce(Job) -> DatastoreResult<Job> + Send>) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: JobRecord = sqlx::query_as::<Postgres, JobRecord>("SELECT * FROM jobs WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))? .ok_or(DatastoreError::JobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
                let user = self.get_user(&record.created_by).await?;
                let job = record.to_job(tasks, vec![], user, vec![], self.encryption_key.as_deref())?;
                let job = modify(job)?;
                let context = serde_json::to_vec(&job.context).map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;
                sqlx::query(r"UPDATE jobs SET state = $1, started_at = $2, completed_at = $3, failed_at = $4, position = $5, context = $6, result = $7, error_ = $8, delete_at = $9, progress = $10, name = $11, description = $12, tags = $13 WHERE id = $14").bind(&job.state).bind(job.started_at).bind(job.completed_at).bind(job.failed_at).bind(job.position).bind(&context).bind(&job.result).bind(&job.error).bind(job.delete_at).bind(job.progress).bind(&job.name).bind(&job.description).bind(&job.tags).bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("update job failed: {e}")))?;
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: JobRecord = sqlx::query_as::<Postgres, JobRecord>("SELECT * FROM jobs WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("get job failed: {e}")))? .ok_or(DatastoreError::JobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| DatastoreError::Serialization(format!("job.tasks: {e}")))?;
                let user = self.get_user(&record.created_by).await?;
                let job = record.to_job(tasks, vec![], user, vec![], self.encryption_key.as_deref())?;
                let job = modify(job)?;
                let context = serde_json::to_vec(&job.context).map_err(|e| DatastoreError::Serialization(format!("job.context: {e}")))?;
                sqlx::query(r"UPDATE jobs SET state = $1, started_at = $2, completed_at = $3, failed_at = $4, position = $5, context = $6, result = $7, error_ = $8, delete_at = $9, progress = $10, name = $11, description = $12, tags = $13 WHERE id = $14").bind(&job.state).bind(job.started_at).bind(job.completed_at).bind(job.failed_at).bind(job.position).bind(&context).bind(&job.result).bind(&job.error).bind(job.delete_at).bind(job.progress).bind(&job.name).bind(&job.description).bind(&job.tags).bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("update job failed: {e}")))?;
            }
        }
        Ok(())
    }

    async fn create_task(&self, task: &Task) -> DatastoreResult<()> {
        let id = task.id.as_ref().ok_or(DatastoreError::InvalidInput("task ID is required".to_string()))?;
        let job_id = task.job_id.as_ref().ok_or(DatastoreError::InvalidInput("job ID is required".to_string()))?;
        // Serialize optional fields: None → SQL NULL (not JSON null bytes)
        let registry: Option<Vec<u8>> = task.registry.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.registry: {e}")))).transpose()?;
        let env: Option<Vec<u8>> = task.env.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.env: {e}")))).transpose()?;
        let files: Option<Vec<u8>> = task.files.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.files: {e}")))).transpose()?;
        let pre: Option<Vec<u8>> = task.pre.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.pre: {e}")))).transpose()?;
        let post: Option<Vec<u8>> = task.post.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.post: {e}")))).transpose()?;
        let sidecars: Option<Vec<u8>> = task.sidecars.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.sidecars: {e}")))).transpose()?;
        let mounts: Option<Vec<u8>> = task.mounts.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.mounts: {e}")))).transpose()?;
        let retry: Option<Vec<u8>> = task.retry.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.retry: {e}")))).transpose()?;
        let limits: Option<Vec<u8>> = task.limits.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.limits: {e}")))).transpose()?;
        let parallel: Option<Vec<u8>> = task.parallel.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.parallel: {e}")))).transpose()?;
        let each: Option<Vec<u8>> = task.each.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.each: {e}")))).transpose()?;
        let subjob: Option<Vec<u8>> = task.subjob.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.subjob: {e}")))).transpose()?;
        
        let q = r"INSERT INTO tasks (id, job_id, position, name, state, created_at, scheduled_at, started_at, completed_at, failed_at, cmd, entrypoint, run_script, image, registry, env, files_, queue, error_, pre_tasks, post_tasks, sidecars, mounts, node_id, retry, limits, timeout, result, var, parallel, parent_id, each_, description, subjob, networks, gpus, if_, tags, priority, workdir, progress) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41)";
        let query = sqlx::query(q).bind(&**id).bind(&**job_id).bind(task.position).bind(&task.name).bind(task.state.as_str()).bind(task.created_at).bind(task.scheduled_at).bind(task.started_at).bind(task.completed_at).bind(task.failed_at).bind(&task.cmd).bind(&task.entrypoint).bind(&task.run).bind(&task.image).bind(&registry).bind(&env).bind(&files).bind(&task.queue).bind(&task.error).bind(&pre).bind(&post).bind(&sidecars).bind(&mounts).bind(task.node_id.as_ref().map(|id| id.as_str())).bind(&retry).bind(&limits).bind(&task.timeout).bind(&task.result).bind(&task.var).bind(&parallel).bind(task.parent_id.as_ref().map(|id| id.as_str())).bind(&each).bind(&task.description).bind(&subjob).bind(&task.networks).bind(&task.gpus).bind(&task.r#if).bind(&task.tags).bind(task.priority).bind(&task.workdir).bind(task.progress);

        match &self.executor {
            Executor::Pool(p) => { query.execute(p).await.map_err(|e| DatastoreError::Database(format!("create task failed: {e}")))?; }
            Executor::Tx(tx) => { let mut tx = tx.lock().await; query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("create task failed: {e}")))?; }
        }
        Ok(())
    }

    async fn get_task_by_id(&self, id: &str) -> DatastoreResult<Task> {
        let record: TaskRecord = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE id = $1").bind(id).fetch_optional(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE id = $1").bind(id).fetch_optional(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))? .ok_or(DatastoreError::TaskNotFound)?;
        record.to_task()
    }

    async fn update_task(&self, id: &str, modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: TaskRecord = sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))? .ok_or(DatastoreError::TaskNotFound)?;
                let task = record.to_task()?;
                let task = modify(task)?;
                let retry: Option<Vec<u8>> = task.retry.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.retry: {e}")))).transpose()?;
                let parallel: Option<Vec<u8>> = task.parallel.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.parallel: {e}")))).transpose()?;
                let each: Option<Vec<u8>> = task.each.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.each: {e}")))).transpose()?;
                let subjob: Option<Vec<u8>> = task.subjob.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.subjob: {e}")))).transpose()?;
                sqlx::query(r"UPDATE tasks SET state = $1, scheduled_at = $2, started_at = $3, completed_at = $4, failed_at = $5, error_ = $6, node_id = $7, retry = $8, result = $9, parallel = $10, each_ = $11, subjob = $12, progress = $13, priority = $14 WHERE id = $15").bind(task.state.as_str()).bind(task.scheduled_at).bind(task.started_at).bind(task.completed_at).bind(task.failed_at).bind(&task.error).bind(task.node_id.as_ref().map(|id| id.as_str())).bind(&retry).bind(&task.result).bind(&parallel).bind(&each).bind(&subjob).bind(task.progress).bind(task.priority).bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("update task failed: {e}")))?;
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: TaskRecord = sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))? .ok_or(DatastoreError::TaskNotFound)?;
                let task = record.to_task()?;
                let task = modify(task)?;
                let retry: Option<Vec<u8>> = task.retry.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.retry: {e}")))).transpose()?;
                let parallel: Option<Vec<u8>> = task.parallel.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.parallel: {e}")))).transpose()?;
                let each: Option<Vec<u8>> = task.each.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.each: {e}")))).transpose()?;
                let subjob: Option<Vec<u8>> = task.subjob.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("task.subjob: {e}")))).transpose()?;
                sqlx::query(r"UPDATE tasks SET state = $1, scheduled_at = $2, started_at = $3, completed_at = $4, failed_at = $5, error_ = $6, node_id = $7, retry = $8, result = $9, parallel = $10, each_ = $11, subjob = $12, progress = $13, priority = $14 WHERE id = $15").bind(task.state.as_str()).bind(task.scheduled_at).bind(task.started_at).bind(task.completed_at).bind(task.failed_at).bind(&task.error).bind(task.node_id.as_ref().map(|id| id.as_str())).bind(&retry).bind(&task.result).bind(&parallel).bind(&each).bind(&subjob).bind(task.progress).bind(task.priority).bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("update task failed: {e}")))?;
            }
        }
        Ok(())
    }

    async fn create_node(&self, node: &Node) -> DatastoreResult<()> {
        let id = node.id.as_ref().ok_or(DatastoreError::InvalidInput("node ID is required".to_string()))?;
        let name = node.name.as_ref().ok_or(DatastoreError::InvalidInput("node name is required".to_string()))?;
        let queue = node.queue.as_ref().ok_or(DatastoreError::InvalidInput("node queue is required".to_string()))?;
        let hostname = node.hostname.as_ref().ok_or(DatastoreError::InvalidInput("node hostname is required".to_string()))?;
        let status = node.status.as_ref().ok_or(DatastoreError::InvalidInput("node status is required".to_string()))?;
        let q = r"INSERT INTO nodes (id, name, queue, started_at, last_heartbeat_at, cpu_percent, status, hostname, port, task_count, version_) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)";
        let query = sqlx::query(q).bind(&**id).bind(name).bind(queue).bind(node.started_at).bind(node.last_heartbeat_at).bind(node.cpu_percent.map_or(0.0, |p| p)).bind(status.as_ref()).bind(hostname).bind(node.port.map_or(0, |p| p)).bind(node.task_count.map_or(0, |p| p)).bind(&node.version);
        match &self.executor {
            Executor::Pool(p) => { query.execute(p).await.map_err(|e| DatastoreError::Database(format!("create node failed: {e}")))?; }
            Executor::Tx(tx) => { let mut tx = tx.lock().await; query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("create node failed: {e}")))?; }
        }
        Ok(())
    }

    async fn update_node(&self, id: &str, modify: Box<dyn FnOnce(Node) -> DatastoreResult<Node> + Send>) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: NodeRecord = sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("get node failed: {e}")))? .ok_or(DatastoreError::NodeNotFound)?;
                let node = record.to_node();
                let node = modify(node)?;
                sqlx::query(r"UPDATE nodes SET last_heartbeat_at = $1, cpu_percent = $2, status = $3, task_count = $4 WHERE id = $5").bind(node.last_heartbeat_at).bind(node.cpu_percent.map_or(0.0, |p| p)).bind(node.status.as_ref().map(|s| s.as_ref())).bind(node.task_count.map_or(0, |p| p)).bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("update node failed: {e}")))?;
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: NodeRecord = sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("get node failed: {e}")))? .ok_or(DatastoreError::NodeNotFound)?;
                let node = record.to_node();
                let node = modify(node)?;
                sqlx::query(r"UPDATE nodes SET last_heartbeat_at = $1, cpu_percent = $2, status = $3, task_count = $4 WHERE id = $5").bind(node.last_heartbeat_at).bind(node.cpu_percent.map_or(0.0, |p| p)).bind(node.status.as_ref().map(|s| s.as_ref())).bind(node.task_count.map_or(0, |p| p)).bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("update node failed: {e}")))?;
            }
        }
        Ok(())
    }

    async fn get_node_by_id(&self, id: &str) -> DatastoreResult<Node> {
        let record: NodeRecord = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE id = $1").bind(id).fetch_optional(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE id = $1").bind(id).fetch_optional(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get node failed: {e}")))? .ok_or(DatastoreError::NodeNotFound)?;
        Ok(record.to_node())
    }

    async fn get_active_nodes(&self) -> DatastoreResult<Vec<Node>> {
        let timeout = time::OffsetDateTime::now_utc() - LAST_HEARTBEAT_TIMEOUT;
        let records: Vec<NodeRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE status = 'UP' AND last_heartbeat_at > $1").bind(timeout).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, NodeRecord>("SELECT * FROM nodes WHERE status = 'UP' AND last_heartbeat_at > $1").bind(timeout).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get active nodes failed: {e}")))?;
        Ok(records.into_iter().map(|r| r.to_node()).collect())
    }

    async fn create_task_log_part(&self, part: &TaskLogPart) -> DatastoreResult<()> {
        let id = part.id.as_ref().ok_or(DatastoreError::InvalidInput("log part ID is required".to_string()))?;
        let task_id = part.task_id.as_ref().ok_or(DatastoreError::InvalidInput("task ID is required".to_string()))?;
        let number = part.number;
        if number < 1 { return Err(DatastoreError::InvalidInput("log part number must be >= 1".to_string())); }
        let q = r"INSERT INTO tasks_log_parts (id, number_, task_id, created_at, contents) VALUES ($1, $2, $3, $4, $5)";
        let query = sqlx::query(q).bind(id).bind(number).bind(&**task_id).bind(part.created_at).bind(&part.contents);
        match &self.executor {
            Executor::Pool(p) => { query.execute(p).await.map_err(|e| DatastoreError::Database(format!("create log part failed: {e}")))?; }
            Executor::Tx(tx) => { let mut tx = tx.lock().await; query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("create log part failed: {e}")))?; }
        }
        Ok(())
    }

    async fn get_task_log_parts(&self, task_id: &str, _q: &str, page: i64, size: i64) -> DatastoreResult<Page<TaskLogPart>> {
        if page < 1 { return Err(DatastoreError::InvalidInput("page must be >= 1".to_string())); }
        if size < 1 { return Err(DatastoreError::InvalidInput("size must be >= 1".to_string())); }
        let offset = (page - 1) * size;
        let records: Vec<TaskLogPartRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, TaskLogPartRecord>("SELECT * FROM tasks_log_parts WHERE task_id = $1 ORDER BY number_ ASC LIMIT $2 OFFSET $3").bind(task_id).bind(size).bind(offset).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, TaskLogPartRecord>("SELECT * FROM tasks_log_parts WHERE task_id = $1 ORDER BY number_ ASC LIMIT $2 OFFSET $3").bind(task_id).bind(size).bind(offset).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get log parts failed: {e}")))?;
        let total: i64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar("SELECT COUNT(*) FROM tasks_log_parts WHERE task_id = $1").bind(task_id).fetch_one(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_scalar("SELECT COUNT(*) FROM tasks_log_parts WHERE task_id = $1").bind(task_id).fetch_one(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get log parts count failed: {e}")))?;
        let items: Vec<TaskLogPart> = records.into_iter().map(|r| r.to_task_log_part()).collect();
        Ok(Page { items, number: page, size, total_pages: (total as f64 / size as f64).ceil() as i64, total_items: total })
    }

    async fn create_scheduled_job(&self, sj: &ScheduledJob) -> DatastoreResult<()> {
        let id = sj.id.as_ref().ok_or(DatastoreError::InvalidInput("scheduled job ID is required".to_string()))?;
        let encryption_key = self.encryption_key.clone();
        let tasks = serde_json::to_vec(&sj.tasks).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
        let inputs = serde_json::to_vec(&sj.inputs).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.inputs: {e}")))?;
        let defaults: Option<Vec<u8>> = sj.defaults.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.defaults: {e}")))).transpose()?;
        let webhooks: Option<Vec<u8>> = sj.webhooks.as_ref().filter(|w| !w.is_empty()).map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.webhooks: {e}")))).transpose()?;
        let auto_delete: Option<Vec<u8>> = sj.auto_delete.as_ref().map(|v| serde_json::to_vec(v).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.auto_delete: {e}")))).transpose()?;
        let mut secrets_bytes = None;
        if let Some(secrets) = &sj.secrets {
            let encrypted = encrypt::encrypt_secrets(secrets, encryption_key.as_deref())?;
            secrets_bytes = Some(serde_json::to_vec(&encrypted).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.secrets: {e}")))?,);
        }
        let created_by = sj.created_by.as_ref().and_then(|u| u.id.clone()).ok_or(DatastoreError::InvalidInput("scheduled_job.created_by.id is required".to_string(),))?;
        
        let q = r"INSERT INTO scheduled_jobs (id, name, description, created_at, tasks, inputs, output_, defaults, webhooks, created_by, tags, auto_delete, secrets, cron_expr, state) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)";
        let query = sqlx::query(q).bind(&**id).bind(&sj.name).bind(&sj.description).bind(sj.created_at).bind(&tasks).bind(&inputs).bind(&sj.output).bind(&defaults).bind(&webhooks).bind(&*created_by).bind(&sj.tags).bind(&auto_delete).bind(&secrets_bytes).bind(&sj.cron).bind(&sj.state);

        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                query.execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("create scheduled job failed: {e}")))?;
                if let Some(permissions) = &sj.permissions {
                    for perm in permissions {
                        let perm_id = uuid::Uuid::new_v4().to_string().replace('-', "");
                        sqlx::query("INSERT INTO scheduled_jobs_perms (id, scheduled_job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut *tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("create scheduled job failed: {e}")))?;
                if let Some(permissions) = &sj.permissions {
                    for perm in permissions {
                        let perm_id = uuid::Uuid::new_v4().to_string().replace('-', "");
                        sqlx::query("INSERT INTO scheduled_jobs_perms (id, scheduled_job_id, user_id, role_id) VALUES ($1, $2, CASE WHEN $3::varchar IS NOT NULL THEN coalesce((select id from users where username_ = $3), $3) ELSE NULL END, CASE WHEN $4::varchar IS NOT NULL THEN coalesce((select id from roles where slug = $4), $4) ELSE NULL END)").bind(&perm_id).bind(&**id).bind(perm.user.as_ref().and_then(|u| u.username.as_ref())).bind(perm.role.as_ref().and_then(|r| r.slug.as_ref())).execute(&mut **tx).await.map_err(|e| { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { DatastoreError::UserNotFound } else if err_msg.contains("_role_id_fkey") { DatastoreError::RoleNotFound } else { DatastoreError::Database(format!("assign role failed: {e}")) } })?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<ScheduledJob>> {
        let records: Vec<ScheduledJobRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs WHERE state = 'ACTIVE'").fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs WHERE state = 'ACTIVE'").fetch_all(&mut **tx).await }
        }.map_err(|e| { DatastoreError::Database(format!("get active scheduled jobs failed: {e}")) })?;
        let mut result = Vec::new();
        for record in records {
            let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
            let user = self.get_user(&record.created_by).await?;
            let sj = record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
            result.push(sj);
        }
        Ok(result)
    }

    async fn get_scheduled_jobs(&self, _current_user: &str, page: i64, size: i64) -> DatastoreResult<Page<ScheduledJobSummary>> {
        if page < 1 { return Err(DatastoreError::InvalidInput("page must be >= 1".to_string())); }
        if size < 1 { return Err(DatastoreError::InvalidInput("size must be >= 1".to_string())); }
        let offset = (page - 1) * size;
        let records: Vec<ScheduledJobRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs ORDER BY created_at DESC LIMIT $1 OFFSET $2").bind(size).bind(offset).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs ORDER BY created_at DESC LIMIT $1 OFFSET $2").bind(size).bind(offset).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get scheduled jobs failed: {e}")))?;
        let mut items = Vec::new();
        for r in records {
            let user = self.get_user(&r.created_by).await?;
            let tasks: Vec<Task> = serde_json::from_slice(&r.tasks).map_or_else(|_| Vec::new(), |v| v);
            let sj = r.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
            items.push(twerk_core::job::new_scheduled_job_summary(&sj));
        }
        let total: i64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar("SELECT COUNT(*) FROM scheduled_jobs").fetch_one(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_scalar("SELECT COUNT(*) FROM scheduled_jobs").fetch_one(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get scheduled jobs count failed: {e}")))?;
        Ok(Page { items, number: page, size, total_pages: (total as f64 / size as f64).ceil() as i64, total_items: total })
    }

    async fn get_scheduled_job_by_id(&self, id: &str) -> DatastoreResult<ScheduledJob> {
        let encryption_key = self.encryption_key.clone();
        let record: ScheduledJobRecord = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs WHERE id = $1").bind(id).fetch_optional(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs WHERE id = $1").bind(id).fetch_optional(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get scheduled job failed: {e}")))? .ok_or(DatastoreError::ScheduledJobNotFound)?;
        let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
        let user = self.get_user(&record.created_by).await?;
        let perms_records: Vec<ScheduledPermRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, ScheduledPermRecord>("SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id = $1").bind(id).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, ScheduledPermRecord>("SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id = $1").bind(id).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get perms failed: {e}")))?;
        let mut perms = Vec::new();
        for pr in perms_records {
            let mut p = Permission { user: None, role: None };
            if let Some(uid) = &pr.user_id { let u = self.get_user(uid).await?; p.user = Some(u); }
            if let Some(rid) = &pr.role_id { let r = self.get_role(rid).await?; p.role = Some(r); }
            perms.push(p);
        }
        record.to_scheduled_job(tasks, user, perms, encryption_key.as_deref())
    }

    async fn update_scheduled_job(&self, id: &str, modify: Box<dyn FnOnce(ScheduledJob) -> DatastoreResult<ScheduledJob> + Send>) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: ScheduledJobRecord = sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("get scheduled job failed: {e}")))? .ok_or(DatastoreError::ScheduledJobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
                let user = self.get_user(&record.created_by).await?;
                let sj = record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
                let sj = modify(sj)?;
                sqlx::query(r"UPDATE scheduled_jobs SET state = $1 WHERE id = $2").bind(&sj.state).bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("update scheduled job failed: {e}")))?;
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: ScheduledJobRecord = sqlx::query_as::<Postgres, ScheduledJobRecord>("SELECT * FROM scheduled_jobs WHERE id = $1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("get scheduled job failed: {e}")))? .ok_or(DatastoreError::ScheduledJobNotFound)?;
                let tasks: Vec<Task> = serde_json::from_slice(&record.tasks).map_err(|e| DatastoreError::Serialization(format!("scheduled_job.tasks: {e}")))?;
                let user = self.get_user(&record.created_by).await?;
                let sj = record.to_scheduled_job(tasks, user, vec![], self.encryption_key.as_deref())?;
                let sj = modify(sj)?;
                sqlx::query(r"UPDATE scheduled_jobs SET state = $1 WHERE id = $2").bind(&sj.state).bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("update scheduled job failed: {e}")))?;
            }
        }
        Ok(())
    }

    async fn delete_scheduled_job(&self, id: &str) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                sqlx::query("DELETE FROM scheduled_jobs_perms WHERE scheduled_job_id = $1").bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("delete scheduled job perms failed: {e}")))?;
                sqlx::query("DELETE FROM scheduled_jobs WHERE id = $1").bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("delete scheduled job failed: {e}")))?;
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query("DELETE FROM scheduled_jobs_perms WHERE scheduled_job_id = $1").bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("delete scheduled job perms failed: {e}")))?;
                sqlx::query("DELETE FROM scheduled_jobs WHERE id = $1").bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("delete scheduled job failed: {e}")))?;
            }
        }
        Ok(())
    }

    async fn create_user(&self, user: &User) -> DatastoreResult<()> {
        let id = user.id.as_ref().ok_or(DatastoreError::InvalidInput("user ID is required".to_string()))?;
        let name = user.name.as_ref().ok_or(DatastoreError::InvalidInput("user name is required".to_string()))?;
        let username = user.username.as_ref().ok_or(DatastoreError::InvalidInput("username is required".to_string()))?;
        let password_hash = user.password_hash.as_ref().ok_or(DatastoreError::InvalidInput("password hash is required".to_string()))?;
        let created_at = user.created_at.ok_or(DatastoreError::InvalidInput("created_at is required".to_string(),))?;
        let q = r"INSERT INTO users (id, name, username_, password_, created_at, is_disabled) VALUES ($1, $2, $3, $4, $5, $6)";
        let query = sqlx::query(q).bind(&**id).bind(name).bind(username).bind(password_hash).bind(created_at).bind(user.disabled);
        match &self.executor {
            Executor::Pool(p) => { query.execute(p).await.map_err(|e| DatastoreError::Database(format!("create user failed: {e}")))?; }
            Executor::Tx(tx) => { let mut tx = tx.lock().await; query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("create user failed: {e}")))?; }
        }
        Ok(())
    }

    async fn get_user(&self, uid: &str) -> DatastoreResult<User> {
        let record: UserRecord = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, UserRecord>("SELECT * FROM users WHERE username_ = $1 OR id = $1").bind(uid).fetch_optional(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, UserRecord>("SELECT * FROM users WHERE username_ = $1 OR id = $1").bind(uid).fetch_optional(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get user failed: {e}")))? .ok_or(DatastoreError::UserNotFound)?;
        Ok(record.to_user())
    }

    async fn create_role(&self, role: &Role) -> DatastoreResult<()> {
        let id = role.id.as_ref().ok_or(DatastoreError::InvalidInput("role ID is required".to_string()))?;
        let slug = role.slug.as_ref().ok_or(DatastoreError::InvalidInput("role slug is required".to_string()))?;
        let name = role.name.as_ref().ok_or(DatastoreError::InvalidInput("role name is required".to_string()))?;
        let created_at = role.created_at.ok_or(DatastoreError::InvalidInput("created_at is required".to_string(),))?;
        let q = r"INSERT INTO roles (id, slug, name, created_at) VALUES ($1, $2, $3, $4)";
        let query = sqlx::query(q).bind(&**id).bind(slug).bind(name).bind(created_at);
        match &self.executor {
            Executor::Pool(p) => { query.execute(p).await.map_err(|e| DatastoreError::Database(format!("create role failed: {e}")))?; }
            Executor::Tx(tx) => { let mut tx = tx.lock().await; query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("create role failed: {e}")))?; }
        }
        Ok(())
    }

    async fn get_role(&self, id: &str) -> DatastoreResult<Role> {
        let record: RoleRecord = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles WHERE id = $1 OR slug = $1").bind(id).fetch_optional(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles WHERE id = $1 OR slug = $1").bind(id).fetch_optional(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get role failed: {e}")))? .ok_or(DatastoreError::RoleNotFound)?;
        Ok(record.to_role())
    }

    async fn get_roles(&self) -> DatastoreResult<Vec<Role>> {
        let records: Vec<RoleRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles ORDER BY name").fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, RoleRecord>("SELECT * FROM roles ORDER BY name").fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get roles failed: {e}")))?;
        Ok(records.into_iter().map(|r| r.to_role()).collect())
    }

    async fn get_user_roles(&self, user_id: &str) -> DatastoreResult<Vec<Role>> {
        let records: Vec<RoleRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, RoleRecord>(r"SELECT r.* FROM roles r INNER JOIN users_roles ur ON ur.role_id = r.id WHERE ur.user_id = $1").bind(user_id).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, RoleRecord>(r"SELECT r.* FROM roles r INNER JOIN users_roles ur ON ur.role_id = r.id WHERE ur.user_id = $1").bind(user_id).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get user roles failed: {e}")))?;
        Ok(records.into_iter().map(|r| r.to_role()).collect())
    }

    async fn assign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        let id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let q = r"insert into users_roles (id, user_id, role_id, created_at) values ($1, $2, $3, $4)";
        let query = sqlx::query(q).bind(&id).bind(user_id).bind(role_id).bind(time::OffsetDateTime::now_utc());
        let result = match &self.executor {
            Executor::Pool(p) => query.execute(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; query.execute(&mut **tx).await }
        };
        match result { Ok(_) => Ok(()), Err(e) => { let err_msg = e.to_string(); if err_msg.contains("_user_id_fkey") { Err(DatastoreError::UserNotFound) } else if err_msg.contains("_role_id_fkey") { Err(DatastoreError::RoleNotFound) } else { Err(DatastoreError::Database(format!("assign role failed: {e}"))) } } }
    }

    async fn unassign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        let q = "delete from users_roles where user_id = $1 and role_id = $2";
        let query = sqlx::query(q).bind(user_id).bind(role_id);
        match &self.executor {
            Executor::Pool(p) => { query.execute(p).await.map_err(|e| DatastoreError::Database(format!("unassign role failed: {e}")))?; }
            Executor::Tx(tx) => { let mut tx = tx.lock().await; query.execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("unassign role failed: {e}")))?; }
        }
        Ok(())
    }

    async fn get_metrics(&self) -> DatastoreResult<twerk_core::stats::Metrics> {
        use twerk_core::stats::{JobMetrics, Metrics, NodeMetrics, TaskMetrics};
        let jobs_running: i64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE state = 'RUNNING'").fetch_one(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE state = 'RUNNING'").fetch_one(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get jobs metrics failed: {e}")))?;
        let tasks_running: i64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar("SELECT COUNT(*) FROM tasks t JOIN jobs j ON t.job_id = j.id WHERE t.state = 'RUNNING' AND j.state = 'RUNNING'").fetch_one(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_scalar("SELECT COUNT(*) FROM tasks t JOIN jobs j ON t.job_id = j.id WHERE t.state = 'RUNNING' AND j.state = 'RUNNING'").fetch_one(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get tasks metrics failed: {e}")))?;
        let nodes_running: i64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar("SELECT COUNT(*) FROM nodes WHERE last_heartbeat_at > current_timestamp - interval '5 minutes'").fetch_one(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_scalar("SELECT COUNT(*) FROM nodes WHERE last_heartbeat_at > current_timestamp - interval '5 minutes'").fetch_one(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get nodes metrics failed: {e}")))?;
        let avg_cpu: f64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar("SELECT coalesce(avg(cpu_percent),0) FROM nodes WHERE last_heartbeat_at > current_timestamp - interval '5 minutes'").fetch_one(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_scalar("SELECT coalesce(avg(cpu_percent),0) FROM nodes WHERE last_heartbeat_at > current_timestamp - interval '5 minutes'").fetch_one(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get nodes cpu failed: {e}")))?;
        Ok(Metrics { jobs: JobMetrics { running: jobs_running as i32 }, tasks: TaskMetrics { running: tasks_running as i32 }, nodes: NodeMetrics { running: nodes_running as i32, cpu_percent: avg_cpu } })
    }

    async fn with_tx(
        &self,
        f: Box<dyn for<'a> FnOnce(&'a dyn Datastore) -> futures_util::future::BoxFuture<'a, DatastoreResult<()>> + Send>,
    ) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let tx = p.begin().await.map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let tx = Arc::new(Mutex::new(tx));
                let ds_tx = PostgresDatastore {
                    executor: Executor::Tx(tx.clone()),
                    logs_retention_duration: self.logs_retention_duration,
                    jobs_retention_duration: self.jobs_retention_duration,
                    cleanup_interval_ms: self.cleanup_interval_ms.clone(),
                    disable_cleanup: self.disable_cleanup,
                    encryption_key: self.encryption_key.clone(),
                };
                if let Err(e) = f(&ds_tx).await {
                    return Err(e);
                }
                let tx = Arc::try_unwrap(tx).map_err(|_| DatastoreError::Transaction("failed to unwrap tx".to_string()))?.into_inner();
                tx.commit().await.map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
                Ok(())
            }
            Executor::Tx(_) => {
                f(self).await
            }
        }
    }


    async fn health_check(&self) -> DatastoreResult<()> {
        let pool = self.pool();
        sqlx::query("SELECT 1").execute(pool).await.map_err(|e| DatastoreError::Database(format!("health check failed: {e}")))?;
        Ok(())
    }

    async fn get_next_task(&self, parent_task_id: &str) -> DatastoreResult<Task> {
        let record: TaskRecord = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE parent_id = $1 AND state = 'CREATED' LIMIT 1").bind(parent_task_id).fetch_optional(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE parent_id = $1 AND state = 'CREATED' LIMIT 1").bind(parent_task_id).fetch_optional(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get next task failed: {e}")))? .ok_or(DatastoreError::TaskNotFound)?;
        record.to_task()
    }

    async fn get_job_log_parts(&self, job_id: &str, _q: &str, page: i64, size: i64) -> DatastoreResult<Page<TaskLogPart>> {
        if page < 1 { return Err(DatastoreError::InvalidInput("page must be >= 1".to_string())); }
        if size < 1 { return Err(DatastoreError::InvalidInput("size must be >= 1".to_string())); }
        let offset = (page - 1) * size;
        let records: Vec<TaskLogPartRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, TaskLogPartRecord>(r"SELECT tlp.* FROM tasks_log_parts tlp JOIN tasks t ON t.id = tlp.task_id WHERE t.job_id = $1 ORDER BY tlp.number_ ASC LIMIT $2 OFFSET $3").bind(job_id).bind(size).bind(offset).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, TaskLogPartRecord>(r"SELECT tlp.* FROM tasks_log_parts tlp JOIN tasks t ON t.id = tlp.task_id WHERE t.job_id = $1 ORDER BY tlp.number_ ASC LIMIT $2 OFFSET $3").bind(job_id).bind(size).bind(offset).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get job log parts failed: {e}")))?;
        let count: i64 = match &self.executor {
            Executor::Pool(p) => sqlx::query_scalar("SELECT count(*) FROM tasks_log_parts tlp JOIN tasks t ON t.id = tlp.task_id WHERE t.job_id = $1").bind(job_id).fetch_one(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_scalar("SELECT count(*) FROM tasks_log_parts tlp JOIN tasks t ON t.id = tlp.task_id WHERE t.job_id = $1").bind(job_id).fetch_one(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("count job log parts failed: {e}")))?;
        let items: Vec<TaskLogPart> = records.into_iter().map(|r| r.to_task_log_part()).collect();
        Ok(Page { items, number: page, size, total_pages: (count as f64 / size as f64).ceil() as i64, total_items: count })
    }

    async fn get_jobs(&self, current_user: &str, q: &str, page: i64, size: i64) -> DatastoreResult<Page<JobSummary>> {
        let (search_term, tags) = parse_query(q);
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
            items.push(twerk_core::job::new_job_summary(&job));
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

        Ok(Page { items, number: page, size, total_pages: (total as f64 / size as f64).ceil() as i64, total_items: total })
    }

    async fn get_active_tasks(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        let active_states = ["CREATED", "PENDING", "SCHEDULED", "RUNNING"];
        let records: Vec<TaskRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, TaskRecord>(r"SELECT * FROM tasks WHERE job_id = $1 AND state = ANY($2) ORDER BY position, created_at ASC").bind(job_id).bind(active_states).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, TaskRecord>(r"SELECT * FROM tasks WHERE job_id = $1 AND state = ANY($2) ORDER BY position, created_at ASC").bind(job_id).bind(active_states).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get active tasks failed: {e}")))?;
        let mut tasks = Vec::new();
        for r in records { tasks.push(r.to_task()?); }
        Ok(tasks)
    }
}

fn parse_query(q: &str) -> (String, Vec<String>) {
    let mut terms = Vec::new();
    let mut tags = Vec::new();
    for part in q.split_whitespace() {
        if let Some(tag) = part.strip_prefix("tag:") {
            tags.push(tag.to_string());
        } else if let Some(tags_str) = part.strip_prefix("tags:") {
            for t in tags_str.split(',') {
                tags.push(t.to_string());
            }
        } else {
            terms.push(part.to_string());
        }
    }
    (terms.join(" "), tags)
}
