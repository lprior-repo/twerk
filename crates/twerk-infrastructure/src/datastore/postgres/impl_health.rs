//! Health check for `PostgresDatastore`.

use crate::datastore::postgres::{DatastoreError, DatastoreResult, Executor, PostgresDatastore};

impl PostgresDatastore {
    pub(super) async fn health_check_impl(&self) -> DatastoreResult<()> {
        match &self.executor {
            Executor::Pool(p) => {
                sqlx::query("SELECT 1")
                    .execute(p)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("health check failed: {e}")))?;
            }
            Executor::Tx(tx) => {
                let mut tx = tx.lock().await;
                sqlx::query("SELECT 1")
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DatastoreError::Database(format!("health check failed: {e}")))?;
            }
        }
        Ok(())
    }
}
