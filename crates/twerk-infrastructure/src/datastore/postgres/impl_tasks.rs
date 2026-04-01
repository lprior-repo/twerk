//! Task operations for `PostgresDatastore`.

use sqlx::Postgres;
use twerk_core::task::Task;

use crate::datastore::postgres::records::{TaskRecord, TaskRecordExt};
use crate::datastore::postgres::{DatastoreError, DatastoreResult, Executor, PostgresDatastore};

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

    pub(super) async fn create_task_impl(&self, task: &Task) -> DatastoreResult<()> {
        let id = task.id.as_ref().ok_or(DatastoreError::InvalidInput(
            "task ID is required".to_string(),
        ))?;
        let job_id = task.job_id.as_ref().ok_or(DatastoreError::InvalidInput(
            "job ID is required".to_string(),
        ))?;
        let registry: Option<Vec<u8>> = task
            .registry
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.registry: {e}")))
            })
            .transpose()?;
        let env: Option<Vec<u8>> = task
            .env
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.env: {e}")))
            })
            .transpose()?;
        let files: Option<Vec<u8>> = task
            .files
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.files: {e}")))
            })
            .transpose()?;
        let pre: Option<Vec<u8>> = task
            .pre
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.pre: {e}")))
            })
            .transpose()?;
        let post: Option<Vec<u8>> = task
            .post
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.post: {e}")))
            })
            .transpose()?;
        let sidecars: Option<Vec<u8>> = task
            .sidecars
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.sidecars: {e}")))
            })
            .transpose()?;
        let mounts: Option<Vec<u8>> = task
            .mounts
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.mounts: {e}")))
            })
            .transpose()?;
        let retry: Option<Vec<u8>> = task
            .retry
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.retry: {e}")))
            })
            .transpose()?;
        let limits: Option<Vec<u8>> = task
            .limits
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.limits: {e}")))
            })
            .transpose()?;
        let parallel: Option<Vec<u8>> = task
            .parallel
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.parallel: {e}")))
            })
            .transpose()?;
        let each: Option<Vec<u8>> = task
            .each
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.each: {e}")))
            })
            .transpose()?;
        let subjob: Option<Vec<u8>> = task
            .subjob
            .as_ref()
            .map(|v| {
                serde_json::to_vec(v)
                    .map_err(|e| DatastoreError::Serialization(format!("task.subjob: {e}")))
            })
            .transpose()?;

        let q = r"INSERT INTO tasks (id, job_id, position, name, state, created_at, scheduled_at, started_at, completed_at, failed_at, cmd, entrypoint, run_script, image, registry, env, files_, queue, error_, pre_tasks, post_tasks, sidecars, mounts, node_id, retry, limits, timeout, result, var, parallel, parent_id, each_, description, subjob, networks, gpus, if_, tags, priority, workdir, progress) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41)";
        let query = sqlx::query(q)
            .bind(&**id)
            .bind(&**job_id)
            .bind(task.position)
            .bind(&task.name)
            .bind(task.state.as_str())
            .bind(task.created_at)
            .bind(task.scheduled_at)
            .bind(task.started_at)
            .bind(task.completed_at)
            .bind(task.failed_at)
            .bind(&task.cmd)
            .bind(&task.entrypoint)
            .bind(&task.run)
            .bind(&task.image)
            .bind(&registry)
            .bind(&env)
            .bind(&files)
            .bind(&task.queue)
            .bind(&task.error)
            .bind(&pre)
            .bind(&post)
            .bind(&sidecars)
            .bind(&mounts)
            .bind(task.node_id.as_ref().map(|n_id| n_id.as_str()))
            .bind(&retry)
            .bind(&limits)
            .bind(&task.timeout)
            .bind(&task.result)
            .bind(&task.var)
            .bind(&parallel)
            .bind(task.parent_id.as_ref().map(|p_id| p_id.as_str()))
            .bind(&each)
            .bind(&task.description)
            .bind(&subjob)
            .bind(&task.networks)
            .bind(&task.gpus)
            .bind(&task.r#if)
            .bind(&task.tags)
            .bind(task.priority)
            .bind(&task.workdir)
            .bind(task.progress);

        match &self.executor {
            Executor::Pool(p) => {
                query
                    .execute(p)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create task failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("create task failed: {e}")))?;
            }
        }
        Ok(())
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
                let record: TaskRecord = sqlx::query_as::<Postgres, TaskRecord>(
                    "SELECT * FROM tasks WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))?
                .ok_or(DatastoreError::TaskNotFound)?;
                let task = record.to_task()?;
                let task = modify(task)?;
                let (retry, parallel, each, subjob) = serialize_task_optionals_impl(&task)?;
                sqlx::query(r"UPDATE tasks SET state = $1, scheduled_at = $2, started_at = $3, completed_at = $4, failed_at = $5, error_ = $6, node_id = $7, retry = $8, result = $9, parallel = $10, each_ = $11, subjob = $12, progress = $13, priority = $14 WHERE id = $15").bind(task.state.as_str()).bind(task.scheduled_at).bind(task.started_at).bind(task.completed_at).bind(task.failed_at).bind(&task.error).bind(task.node_id.as_ref().map(|n_id| n_id.as_str())).bind(&retry).bind(&task.result).bind(&parallel).bind(&each).bind(&subjob).bind(task.progress).bind(task.priority).bind(id).execute(&mut *tx).await.map_err(|e| DatastoreError::Database(format!("update task failed: {e}")))?;
                tx.commit()
                    .await
                    .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                let record: TaskRecord = sqlx::query_as::<Postgres, TaskRecord>(
                    "SELECT * FROM tasks WHERE id = $1 FOR UPDATE",
                )
                .bind(id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| DatastoreError::Database(format!("get task failed: {e}")))?
                .ok_or(DatastoreError::TaskNotFound)?;
                let task = record.to_task()?;
                let task = modify(task)?;
                let (retry, parallel, each, subjob) = serialize_task_optionals_impl(&task)?;
                sqlx::query(r"UPDATE tasks SET state = $1, scheduled_at = $2, started_at = $3, completed_at = $4, failed_at = $5, error_ = $6, node_id = $7, retry = $8, result = $9, parallel = $10, each_ = $11, subjob = $12, progress = $13, priority = $14 WHERE id = $15").bind(task.state.as_str()).bind(task.scheduled_at).bind(task.started_at).bind(task.completed_at).bind(task.failed_at).bind(&task.error).bind(task.node_id.as_ref().map(|n_id| n_id.as_str())).bind(&retry).bind(&task.result).bind(&parallel).bind(&each).bind(&subjob).bind(task.progress).bind(task.priority).bind(id).execute(&mut **tx).await.map_err(|e| DatastoreError::Database(format!("update task failed: {e}")))?;
            }
        }
        Ok(())
    }

    pub(super) async fn get_active_tasks_impl(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        let active_states = ["CREATED", "PENDING", "SCHEDULED", "RUNNING"];
        let records: Vec<TaskRecord> = match &self.executor {
            Executor::Pool(p) => sqlx::query_as::<Postgres, TaskRecord>(r"SELECT * FROM tasks WHERE job_id = $1 AND state = ANY($2) ORDER BY position, created_at ASC").bind(job_id).bind(active_states).fetch_all(p).await,
            Executor::Tx(tx) => { let mut tx = tx.lock().await; sqlx::query_as::<Postgres, TaskRecord>(r"SELECT * FROM tasks WHERE job_id = $1 AND state = ANY($2) ORDER BY position, created_at ASC").bind(job_id).bind(active_states).fetch_all(&mut **tx).await }
        }.map_err(|e| DatastoreError::Database(format!("get active tasks failed: {e}")))?;
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

/// Serializes optional task fields for update.
#[allow(clippy::type_complexity)]
fn serialize_task_optionals_impl(
    task: &Task,
) -> DatastoreResult<(
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
)> {
    let retry = task
        .retry
        .as_ref()
        .map(|v| {
            serde_json::to_vec(v)
                .map_err(|e| DatastoreError::Serialization(format!("task.retry: {e}")))
        })
        .transpose()?;
    let parallel = task
        .parallel
        .as_ref()
        .map(|v| {
            serde_json::to_vec(v)
                .map_err(|e| DatastoreError::Serialization(format!("task.parallel: {e}")))
        })
        .transpose()?;
    let each = task
        .each
        .as_ref()
        .map(|v| {
            serde_json::to_vec(v)
                .map_err(|e| DatastoreError::Serialization(format!("task.each: {e}")))
        })
        .transpose()?;
    let subjob = task
        .subjob
        .as_ref()
        .map(|v| {
            serde_json::to_vec(v)
                .map_err(|e| DatastoreError::Serialization(format!("task.subjob: {e}")))
        })
        .transpose()?;
    Ok((retry, parallel, each, subjob))
}
