//! Task operations for `PostgresDatastore`.

use serde::Serialize;
use sqlx::Postgres;
use time::OffsetDateTime;
use twerk_core::id::{JobId, TaskId};
use twerk_core::task::Task;

use crate::datastore::postgres::records::{TaskRecord, TaskRecordExt};
use crate::datastore::postgres::{DatastoreError, DatastoreResult, Executor, PostgresDatastore};

const SQL_UPDATE_TASK: &str = r"
UPDATE tasks SET
    state = $1, scheduled_at = $2, started_at = $3, completed_at = $4,
    failed_at = $5, error_ = $6, node_id = $7, retry = $8, result = $9,
    parallel = $10, each_ = $11, subjob = $12, progress = $13,
    priority = $14, queue = $15, limits = $16, timeout = $17
WHERE id = $18
";

const SQL_GET_TASK_FOR_UPDATE: &str = "SELECT * FROM tasks WHERE id = $1 FOR UPDATE";

const SQL_GET_ACTIVE_TASKS: &str = r"
SELECT * FROM tasks
WHERE job_id = $1 AND state = ANY($2)
ORDER BY position, created_at ASC
";

const SQL_GET_ALL_TASKS_FOR_JOB: &str = r"
SELECT * FROM tasks
WHERE job_id = $1
ORDER BY position, created_at ASC
";

// ── Pure calculations: JSON field serialization ──────────────────────────────

/// Serializes a single optional JSON field to bytes.
///
/// Pure calculation: converts `Option<T: Serialize>` into `Option<Vec<u8>>`,
/// mapping `serde_json` errors into `DatastoreError::Serialization`.
fn serialize_json_field<T: Serialize>(
    field: &Option<T>,
    field_name: &str,
) -> DatastoreResult<Option<Vec<u8>>> {
    field
        .as_ref()
        .map(|v| {
            serde_json::to_vec(v)
                .map_err(|e| DatastoreError::Serialization(format!("task.{field_name}: {e}")))
        })
        .transpose()
}

/// Holds all serialized optional JSON fields for a task INSERT.
struct SerializedTaskFields {
    registry: Option<Vec<u8>>,
    env: Option<Vec<u8>>,
    files: Option<Vec<u8>>,
    pre: Option<Vec<u8>>,
    post: Option<Vec<u8>>,
    sidecars: Option<Vec<u8>>,
    mounts: Option<Vec<u8>>,
    retry: Option<Vec<u8>>,
    limits: Option<Vec<u8>>,
    parallel: Option<Vec<u8>>,
    each: Option<Vec<u8>>,
    subjob: Option<Vec<u8>>,
}

/// Serializes all 12 optional JSON fields needed for task INSERT.
///
/// Pure calculation: delegates each field to `serialize_json_field`.
fn serialize_task_insert_fields(task: &Task) -> DatastoreResult<SerializedTaskFields> {
    Ok(SerializedTaskFields {
        registry: serialize_json_field(&task.registry, "registry")?,
        env: serialize_json_field(&task.env, "env")?,
        files: serialize_json_field(&task.files, "files")?,
        pre: serialize_json_field(&task.pre, "pre")?,
        post: serialize_json_field(&task.post, "post")?,
        sidecars: serialize_json_field(&task.sidecars, "sidecars")?,
        mounts: serialize_json_field(&task.mounts, "mounts")?,
        retry: serialize_json_field(&task.retry, "retry")?,
        limits: serialize_json_field(&task.limits, "limits")?,
        parallel: serialize_json_field(&task.parallel, "parallel")?,
        each: serialize_json_field(&task.each, "each")?,
        subjob: serialize_json_field(&task.subjob, "subjob")?,
    })
}

/// Serializes optional task fields for UPDATE.
#[allow(clippy::type_complexity)]
fn serialize_task_optionals_for_update(
    task: &Task,
) -> DatastoreResult<(
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
)> {
    Ok((
        serialize_json_field(&task.retry, "retry")?,
        serialize_json_field(&task.parallel, "parallel")?,
        serialize_json_field(&task.each, "each")?,
        serialize_json_field(&task.subjob, "subjob")?,
        serialize_json_field(&task.limits, "limits")?,
    ))
}

// ── Shell boundary: query execution ──────────────────────────────────────────

/// Builds and executes the 41-parameter task INSERT against the executor.
///
/// This function sits at the I/O shell boundary: it constructs the SQL query
/// and dispatches it to either a pool connection or a transaction.
async fn execute_task_insert(
    executor: &Executor,
    task: &Task,
    id: &TaskId,
    job_id: &JobId,
    f: &SerializedTaskFields,
) -> DatastoreResult<()> {
    let sql = r"
INSERT INTO tasks (
    id, job_id, position, name, state, created_at, scheduled_at,
    started_at, completed_at, failed_at, cmd, entrypoint, run_script,
    image, registry, env, files_, queue, error_, pre_tasks, post_tasks,
    sidecars, mounts, node_id, retry, limits, timeout, result, var,
    parallel, parent_id, each_, description, subjob, networks, gpus,
    if_, tags, priority, workdir, progress
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
    $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
    $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41
)";
    let query = sqlx::query(sql)
        .bind(&**id)
        .bind(&**job_id)
        .bind(task.position)
        .bind(&task.name)
        .bind(task.state.to_string())
        .bind(match task.created_at {
            Some(t) => t,
            None => OffsetDateTime::now_utc(),
        })
        .bind(task.scheduled_at)
        .bind(task.started_at)
        .bind(task.completed_at)
        .bind(task.failed_at)
        .bind(&task.cmd)
        .bind(&task.entrypoint)
        .bind(&task.run)
        .bind(&task.image)
        .bind(&f.registry)
        .bind(&f.env)
        .bind(&f.files)
        .bind(&task.queue)
        .bind(&task.error)
        .bind(&f.pre)
        .bind(&f.post)
        .bind(&f.sidecars)
        .bind(&f.mounts)
        .bind(task.node_id.as_ref().map(|n| n.as_str()))
        .bind(&f.retry)
        .bind(&f.limits)
        .bind(&task.timeout)
        .bind(&task.result)
        .bind(&task.var)
        .bind(&f.parallel)
        .bind(task.parent_id.as_ref().map(|p| p.as_str()))
        .bind(&f.each)
        .bind(&task.description)
        .bind(&f.subjob)
        .bind(&task.networks)
        .bind(&task.gpus)
        .bind(&task.r#if)
        .bind(&task.tags)
        .bind(task.priority)
        .bind(&task.workdir)
        .bind(task.progress);

    match executor {
        Executor::Pool(p) => {
            query
                .execute(p)
                .await
                .map_err(|e| DatastoreError::Database(format!("create task failed: {e}")))?;
        }
        Executor::Tx(tx) => {
            // SAFETY: &mut tx is required by sqlx's Transaction::execute API.
            let mut tx = tx.lock().await;
            query
                .execute(&mut **tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("create task failed: {e}")))?;
        }
    }
    Ok(())
}

// ── PostgresDatastore methods ─────────────────────────────────────────────────

impl PostgresDatastore {
    pub(super) async fn create_tasks_impl(&self, tasks: &[Task]) -> DatastoreResult<()> {
        // Clone into owned Vec to satisfy the 'static bound on with_tx_impl's closure
        let tasks: Vec<Task> = tasks.to_vec();
        self.with_tx_impl(Box::new(move |ds| {
            Box::pin(async move {
                for task in &tasks {
                    ds.create_task(task).await?;
                }
                Ok(())
            })
        }))
        .await
    }

    /// Orchestrates task creation: validate IDs → serialize fields → execute INSERT.
    pub(super) async fn create_task_impl(&self, task: &Task) -> DatastoreResult<()> {
        let id = task
            .id
            .as_ref()
            .ok_or_else(|| DatastoreError::InvalidInput("task ID is required".to_string()))?;
        let job_id = task
            .job_id
            .as_ref()
            .ok_or_else(|| DatastoreError::InvalidInput("job ID is required".to_string()))?;
        let fields = serialize_task_insert_fields(task)?;
        execute_task_insert(&self.executor, task, id, job_id, &fields).await
    }

    pub(super) async fn get_task_by_id_impl(&self, id: &str) -> DatastoreResult<Task> {
        let record: TaskRecord = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE id = $1")
                    .bind(id)
                    .fetch_optional(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, TaskRecord>("SELECT * FROM tasks WHERE id = $1")
                    .bind(id)
                    .fetch_optional(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))?
        .ok_or(DatastoreError::TaskNotFound)?;
        record.to_task()
    }

    pub(super) async fn update_task_impl(
        &self,
        id: &str,
        modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>,
    ) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                let mut tx = p
                    .begin()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;
                let record: TaskRecord =
                    sqlx::query_as::<Postgres, TaskRecord>(SQL_GET_TASK_FOR_UPDATE)
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))?
                        .ok_or(DatastoreError::TaskNotFound)?;
                let task = record.to_task()?;
                let task = modify(task)?;
                let (retry, parallel, each, subjob, limits) =
                    serialize_task_optionals_for_update(&task)?;
                sqlx::query(SQL_UPDATE_TASK)
                    .bind(task.state.to_string())
                    .bind(task.scheduled_at)
                    .bind(task.started_at)
                    .bind(task.completed_at)
                    .bind(task.failed_at)
                    .bind(&task.error)
                    .bind(task.node_id.as_ref().map(|n_id| n_id.as_str()))
                    .bind(&retry)
                    .bind(&task.result)
                    .bind(&parallel)
                    .bind(&each)
                    .bind(&subjob)
                    .bind(task.progress)
                    .bind(task.priority)
                    .bind(&task.queue)
                    .bind(&limits)
                    .bind(&task.timeout)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("update task failed: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: TaskRecord =
                    sqlx::query_as::<Postgres, TaskRecord>(SQL_GET_TASK_FOR_UPDATE)
                        .bind(id)
                        .fetch_optional(&mut **tx)
                        .await
                        .map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))?
                        .ok_or(DatastoreError::TaskNotFound)?;
                let task = record.to_task()?;
                let task = modify(task)?;
                let (retry, parallel, each, subjob, limits) =
                    serialize_task_optionals_for_update(&task)?;
                sqlx::query(SQL_UPDATE_TASK)
                    .bind(task.state.to_string())
                    .bind(task.scheduled_at)
                    .bind(task.started_at)
                    .bind(task.completed_at)
                    .bind(task.failed_at)
                    .bind(&task.error)
                    .bind(task.node_id.as_ref().map(|n_id| n_id.as_str()))
                    .bind(&retry)
                    .bind(&task.result)
                    .bind(&parallel)
                    .bind(&each)
                    .bind(&subjob)
                    .bind(task.progress)
                    .bind(task.priority)
                    .bind(&task.queue)
                    .bind(&limits)
                    .bind(&task.timeout)
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("update task failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn get_active_tasks_impl(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        let active_states = ["CREATED", "PENDING", "SCHEDULED", "RUNNING"];
        let records: Vec<TaskRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, TaskRecord>(SQL_GET_ACTIVE_TASKS)
                    .bind(job_id)
                    .bind(active_states)
                    .fetch_all(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, TaskRecord>(SQL_GET_ACTIVE_TASKS)
                    .bind(job_id)
                    .bind(active_states)
                    .fetch_all(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get active tasks failed: {e}")))?;
        records.into_iter().map(|r| r.to_task()).collect()
    }

    pub(super) async fn get_all_tasks_for_job_impl(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        let records: Vec<TaskRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, TaskRecord>(SQL_GET_ALL_TASKS_FOR_JOB)
                    .bind(job_id)
                    .fetch_all(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, TaskRecord>(SQL_GET_ALL_TASKS_FOR_JOB)
                    .bind(job_id)
                    .fetch_all(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get all tasks for job failed: {e}")))?;
        records.into_iter().map(|r| r.to_task()).collect()
    }

    pub(super) async fn get_next_task_impl(&self, parent_task_id: &str) -> DatastoreResult<Task> {
        let record: TaskRecord = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, TaskRecord>(
                    "SELECT * FROM tasks WHERE parent_id = $1 AND state = 'CREATED' LIMIT 1",
                )
                .bind(parent_task_id)
                .fetch_optional(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, TaskRecord>(
                    "SELECT * FROM tasks WHERE parent_id = $1 AND state = 'CREATED' LIMIT 1",
                )
                .bind(parent_task_id)
                .fetch_optional(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get next task failed: {e}")))?
        .ok_or(DatastoreError::TaskNotFound)?;
        record.to_task()
    }
}
