//! `PostgreSQL` implementation of the datastore.
//!
//! This module provides a PostgreSQL-backed implementation of the `Datastore` trait.
//!
//! Structure:
//! - **mod.rs** (this file): Core struct, connection management, and cleanup
//! - **impl_datastore.rs**: Full `Datastore` trait implementation
//! - **records/**: Database record types and conversions
//! - **schema.rs**: Database schema migrations
//! - **encrypt.rs**: Encryption utilities

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

pub use super::{Datastore, Error as DatastoreError, Options, Page, Result as DatastoreResult};
pub mod encrypt;
pub mod impl_datastore;
pub mod impl_health;
pub mod impl_jobs;
pub mod impl_metrics;
pub mod impl_nodes;
pub mod impl_scheduled_jobs;
pub mod impl_task_logs;
pub mod impl_tasks;
pub mod impl_transaction;
pub mod impl_users_roles;
pub mod records;
pub mod schema;
#[cfg(test)]
pub mod schema_tests;

pub use schema::SCHEMA;

use twerk_core::job::{JOB_STATE_COMPLETED, JOB_STATE_FAILED};

/// Minimal cleanup interval (1 minute)
pub const MIN_CLEANUP_INTERVAL: Duration = Duration::minutes(1);
/// Maximum cleanup interval (1 hour)
pub const MAX_CLEANUP_INTERVAL: Duration = Duration::hours(1);

/// Default retention period for logs (1 week)
pub const DEFAULT_LOGS_RETENTION_DURATION: Duration = Duration::days(7);
/// Default retention period for jobs (1 year)
pub const DEFAULT_JOBS_RETENTION_DURATION: Duration = Duration::days(365);

/// Internal executor type for database operations.
#[derive(Clone, Debug)]
pub enum Executor {
    /// Direct pool connection
    Pool(PgPool),
    /// Transaction-scoped connection
    Tx(Arc<Mutex<Transaction<'static, Postgres>>>),
}

/// `PostgresDatastore` implements the `Datastore` trait using `PostgreSQL`.
#[derive(Clone, Debug)]
pub struct PostgresDatastore {
    pub(super) executor: Executor,
    pub(super) logs_retention_duration: Duration,
    pub(super) jobs_retention_duration: Duration,
    pub(super) cleanup_interval_ms: Arc<AtomicI64>,
    pub(super) disable_cleanup: bool,
    pub(super) encryption_key: Option<String>,
}

impl PostgresDatastore {
    /// Creates a new `PostgresDatastore` from a connection string.
    pub async fn new(dsn: &str, options: Options) -> DatastoreResult<Self> {
        let mut pool_options = PgPoolOptions::new();
        if let Some(max_conns) = options.max_open_conns {
            pool_options = pool_options.max_connections(max_conns as u32);
        }
        if let Some(max_idle) = options.max_idle_conns {
            pool_options = pool_options.min_connections(max_idle as u32);
        }
        if let Some(lifetime) = options.conn_max_lifetime {
            pool_options = pool_options.max_lifetime(StdDuration::from_secs(
                lifetime.whole_seconds().unsigned_abs(),
            ));
        }
        if let Some(idle_time) = options.conn_max_idle_time {
            pool_options = pool_options.idle_timeout(StdDuration::from_secs(
                idle_time.whole_seconds().unsigned_abs(),
            ));
        }
        let pool = pool_options
            .connect(dsn)
            .await
            .map_err(|e| DatastoreError::Database(format!("connection failed: {e}")))?;
        let cleanup_interval = if options.cleanup_interval < MIN_CLEANUP_INTERVAL {
            MIN_CLEANUP_INTERVAL
        } else {
            options.cleanup_interval
        };
        let logs_retention_duration = if options.logs_retention_duration < MIN_CLEANUP_INTERVAL {
            MIN_CLEANUP_INTERVAL
        } else {
            options.logs_retention_duration
        };
        let jobs_retention_duration = if options.jobs_retention_duration < MIN_CLEANUP_INTERVAL {
            MIN_CLEANUP_INTERVAL
        } else {
            options.jobs_retention_duration
        };
        #[allow(clippy::cast_possible_truncation)]
        let cleanup_interval_ms = cleanup_interval.whole_milliseconds() as i64;
        Ok(Self {
            executor: Executor::Pool(pool),
            logs_retention_duration,
            jobs_retention_duration,
            cleanup_interval_ms: Arc::new(AtomicI64::new(cleanup_interval_ms)),
            disable_cleanup: options.disable_cleanup,
            encryption_key: options.encryption_key,
        })
    }

    /// Returns the underlying database pool.
    ///
    /// Returns an error if called on a transaction-scoped datastore.
    pub fn pool(&self) -> DatastoreResult<&PgPool> {
        match &self.executor {
            Executor::Pool(p) => Ok(p),
            Executor::Tx(_) => Err(DatastoreError::Database("cannot get pool from transaction".to_string())),
        }
    }

    /// Closes the database connection pool.
    pub async fn close(&self) -> DatastoreResult<()> {
        if let Executor::Pool(p) = &self.executor {
            p.close().await;
        }
        Ok(())
    }

    /// Spawns the background cleanup process.
    pub fn spawn_cleanup(self) {
        if self.disable_cleanup {
            return;
        }
        let ds = self.clone();
        tokio::spawn(async move {
            ds.cleanup_process().await;
        });
    }

    async fn cleanup_process(&self) {
        loop {
            let interval_ms = self.cleanup_interval_ms.load(Ordering::Relaxed);
            tokio::time::sleep(StdDuration::from_millis(interval_ms as u64)).await;
            match self.cleanup().await {
                Ok(count) => {
                    if count > 0 {
                        let mut new_val = interval_ms / 2;
                        if new_val < MIN_CLEANUP_INTERVAL.whole_milliseconds() as i64 {
                            new_val = MIN_CLEANUP_INTERVAL.whole_milliseconds() as i64;
                        }
                        self.cleanup_interval_ms.store(new_val, Ordering::Relaxed);
                    } else {
                        let mut new_val = interval_ms * 2;
                        if new_val > MAX_CLEANUP_INTERVAL.whole_milliseconds() as i64 {
                            new_val = MAX_CLEANUP_INTERVAL.whole_milliseconds() as i64;
                        }
                        self.cleanup_interval_ms.store(new_val, Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    tracing::error!("cleanup error: {e}");
                }
            }
        }
    }

    /// Runs cleanup for logs and jobs.
    pub async fn cleanup(&self) -> DatastoreResult<i64> {
        let mut total = 0;
        total += self.cleanup_logs().await?;
        total += self.cleanup_jobs().await?;
        Ok(total)
    }

    async fn cleanup_logs(&self) -> DatastoreResult<i64> {
        let cutoff = OffsetDateTime::now_utc() - self.logs_retention_duration;
        let pool = self.pool()?;
        let result = sqlx::query("DELETE FROM tasks_log_parts WHERE created_at < $1")
            .bind(cutoff)
            .execute(pool)
            .await
            .map_err(|e| DatastoreError::Database(format!("cleanup logs failed: {e}")))?;
        Ok(result.rows_affected() as i64)
    }

    async fn cleanup_jobs(&self) -> DatastoreResult<i64> {
        let cutoff = OffsetDateTime::now_utc() - self.jobs_retention_duration;
        let pool = self.pool()?;
        let result = sqlx::query("DELETE FROM jobs WHERE state IN ($1, $2) AND (delete_at IS NOT NULL AND delete_at < $3 OR (delete_at IS NULL AND (completed_at < $4 OR failed_at < $4)))").bind(JOB_STATE_COMPLETED).bind(JOB_STATE_FAILED).bind(OffsetDateTime::now_utc()).bind(cutoff).execute(pool).await.map_err(|e| DatastoreError::Database(format!("cleanup jobs failed: {e}")))?;
        Ok(result.rows_affected() as i64)
    }

    /// Executes a SQL script (multiple statements separated by semicolons).
    pub async fn exec_script(&self, script: &str) -> DatastoreResult<()> {
        let pool = self.pool()?;
        for stmt in script.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            sqlx::query(trimmed)
                .execute(pool)
                .await
                .map_err(|e| DatastoreError::Database(format!("exec script failed: {e}")))?;
        }
        Ok(())
    }
}
