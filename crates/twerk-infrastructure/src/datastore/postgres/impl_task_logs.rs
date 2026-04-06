//! Task log operations for `PostgresDatastore`.

use sqlx::Postgres;
use twerk_core::task::TaskLogPart;

use crate::datastore::postgres::records::{TaskLogPartRecord, TaskLogPartRecordExt};
use crate::datastore::postgres::{
    DatastoreError, DatastoreResult, Executor, Page, PostgresDatastore,
};

impl PostgresDatastore {
    pub(super) async fn create_task_log_part_impl(
        &self,
        part: &TaskLogPart,
    ) -> DatastoreResult<()> {
        let id = part.id.as_ref().ok_or(DatastoreError::InvalidInput(
            "part ID is required".to_string(),
        ))?;
        let task_id = part.task_id.as_ref().ok_or(DatastoreError::InvalidInput(
            "task_id is required".to_string(),
        ))?;
        let contents = part.contents.as_ref().ok_or(DatastoreError::InvalidInput(
            "contents is required".to_string(),
        ))?;

        let q = r"INSERT INTO tasks_log_parts (id, number_, task_id, created_at, contents) VALUES ($1, $2, $3, $4, $5)";
        let query = sqlx::query(q)
            .bind(&**id)
            .bind(part.number)
            .bind(&**task_id)
            .bind(
                part.created_at
                    .unwrap_or_else(time::OffsetDateTime::now_utc),
            )
            .bind(contents);

        match &self.executor {
            Executor::Pool(p) => {
                query.execute(p).await.map_err(|e| {
                    DatastoreError::Database(format!("create task log part failed: {e}"))
                })?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                query.execute(&mut **tx).await.map_err(|e| {
                    DatastoreError::Database(format!("create task log part failed: {e}"))
                })?;
            }
        }
        Ok(())
    }

    pub(super) async fn get_task_log_parts_impl(
        &self,
        task_id: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<TaskLogPart>> {
        let offset = (page - 1) * size;
        let records: Vec<TaskLogPartRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, TaskLogPartRecord>(
                    "SELECT * FROM tasks_log_parts WHERE task_id = $1 ORDER BY number_ ASC LIMIT $2 OFFSET $3",
                )
                .bind(task_id)
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, TaskLogPartRecord>(
                    "SELECT * FROM tasks_log_parts WHERE task_id = $1 ORDER BY number_ ASC LIMIT $2 OFFSET $3",
                )
                .bind(task_id)
                .bind(size)
                .bind(offset)
                .fetch_all(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get task log parts failed: {e}")))?;

        let total: i64 = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_scalar("SELECT count(*) FROM tasks_log_parts WHERE task_id = $1")
                    .bind(task_id)
                    .fetch_one(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar("SELECT count(*) FROM tasks_log_parts WHERE task_id = $1")
                    .bind(task_id)
                    .fetch_one(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("count task log parts failed: {e}")))?;

        let items: Vec<TaskLogPart> = records
            .into_iter()
            .map(|r| r.to_task_log_part())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page {
            items,
            number: page,
            size,
            total_pages: (total as f64 / size as f64).ceil() as i64,
            total_items: total,
        })
    }

    pub(super) async fn get_job_log_parts_impl(
        &self,
        job_id: &str,
        _q: &str,
        page: i64,
        size: i64,
    ) -> DatastoreResult<Page<TaskLogPart>> {
        let offset = (page - 1) * size;
        let records: Vec<TaskLogPartRecord> = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_as::<Postgres, TaskLogPartRecord>(
                    r#"
                    SELECT lp.* FROM tasks_log_parts lp
                    INNER JOIN tasks t ON lp.task_id = t.id
                    WHERE t.job_id = $1
                    ORDER BY lp.number_ ASC
                    LIMIT $2 OFFSET $3
                    "#,
                )
                .bind(job_id)
                .bind(size)
                .bind(offset)
                .fetch_all(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_as::<Postgres, TaskLogPartRecord>(
                    r#"
                    SELECT lp.* FROM tasks_log_parts lp
                    INNER JOIN tasks t ON lp.task_id = t.id
                    WHERE t.job_id = $1
                    ORDER BY lp.number_ ASC
                    LIMIT $2 OFFSET $3
                    "#,
                )
                .bind(job_id)
                .bind(size)
                .bind(offset)
                .fetch_all(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("get job log parts failed: {e}")))?;

        let total: i64 = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_scalar(
                    r#"
                    SELECT count(lp.*) FROM tasks_log_parts lp
                    INNER JOIN tasks t ON lp.task_id = t.id
                    WHERE t.job_id = $1
                    "#,
                )
                .bind(job_id)
                .fetch_one(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar(
                    r#"
                    SELECT count(lp.*) FROM tasks_log_parts lp
                    INNER JOIN tasks t ON lp.task_id = t.id
                    WHERE t.job_id = $1
                    "#,
                )
                .bind(job_id)
                .fetch_one(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("count job log parts failed: {e}")))?;

        let items: Vec<TaskLogPart> = records
            .into_iter()
            .map(|r| r.to_task_log_part())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page {
            items,
            number: page,
            size,
            total_pages: (total as f64 / size as f64).ceil() as i64,
            total_items: total,
        })
    }
}
