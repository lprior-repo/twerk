//! Metrics operations for `PostgresDatastore`.

use crate::datastore::postgres::{DatastoreError, DatastoreResult, Executor, PostgresDatastore};

impl PostgresDatastore {
    pub(super) async fn get_metrics_impl(&self) -> DatastoreResult<twerk_core::stats::Metrics> {
        let jobs_running: i64 = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_scalar("SELECT count(*) FROM jobs WHERE state = 'RUNNING'")
                    .fetch_one(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar("SELECT count(*) FROM jobs WHERE state = 'RUNNING'")
                    .fetch_one(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("count running jobs failed: {e}")))?;

        let tasks_running: i64 = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_scalar("SELECT count(*) FROM tasks WHERE state = 'RUNNING'")
                    .fetch_one(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar("SELECT count(*) FROM tasks WHERE state = 'RUNNING'")
                    .fetch_one(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("count running tasks failed: {e}")))?;

        let nodes_running: i64 = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_scalar("SELECT count(*) FROM nodes WHERE status != 'DOWN'")
                    .fetch_one(p)
                    .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar("SELECT count(*) FROM nodes WHERE status != 'DOWN'")
                    .fetch_one(&mut **tx)
                    .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("count running nodes failed: {e}")))?;

        let cpu_percent: f64 = match &self.executor {
            Executor::Pool(p) => {
                sqlx::query_scalar(
                    "SELECT coalesce(avg(cpu_percent), 0.0) FROM nodes WHERE status != 'DOWN'",
                )
                .fetch_one(p)
                .await
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query_scalar(
                    "SELECT coalesce(avg(cpu_percent), 0.0) FROM nodes WHERE status != 'DOWN'",
                )
                .fetch_one(&mut **tx)
                .await
            }
        }
        .map_err(|e| DatastoreError::Database(format!("avg cpu percent failed: {e}")))?;

        Ok(twerk_core::stats::Metrics {
            jobs: twerk_core::stats::JobMetrics {
                running: jobs_running as i32,
            },
            tasks: twerk_core::stats::TaskMetrics {
                running: tasks_running as i32,
            },
            nodes: twerk_core::stats::NodeMetrics {
                running: nodes_running as i32,
                cpu_percent,
            },
        })
    }
}
