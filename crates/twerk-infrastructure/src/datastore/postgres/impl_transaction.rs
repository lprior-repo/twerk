//! Transaction wrapper for `PostgresDatastore`.

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::datastore::postgres::{DatastoreError, DatastoreResult, Executor, PostgresDatastore};
use crate::datastore::Datastore;

impl PostgresDatastore {
    #[allow(clippy::type_complexity)]
    pub(super) async fn with_tx_impl(
        &self,
        f: Box<
            dyn for<'a> FnOnce(
                    &'a dyn Datastore,
                )
                    -> futures_util::future::BoxFuture<'a, DatastoreResult<()>>
                + Send,
        >,
    ) -> DatastoreResult<()> {
        let pool = match &self.executor {
            Executor::Pool(p) => p.clone(),
            Executor::Tx(_) => {
                return Err(DatastoreError::Transaction(
                    "cannot start transaction within existing transaction".to_string(),
                ));
            }
        };

        let tx = pool
            .begin()
            .await
            .map_err(|e| DatastoreError::Transaction(format!("begin tx failed: {e}")))?;

        let tx = Arc::new(Mutex::new(tx));
        let tx_datastore = PostgresDatastore {
            executor: Executor::Tx(tx.clone()),
            logs_retention_duration: self.logs_retention_duration,
            jobs_retention_duration: self.jobs_retention_duration,
            cleanup_interval_ms: self.cleanup_interval_ms.clone(),
            disable_cleanup: true,
            encryption_key: self.encryption_key.clone(),
        };

        f(&tx_datastore).await?;

        // Extract the transaction from the Arc<Mutex>
        // We need to avoid the deadlock that Arc::try_unwrap + Mutex::into_inner would cause
        // Instead, we get a lock and leak it to extract the tx
        let tx = Arc::try_unwrap(tx)
            .map_err(|_| DatastoreError::Transaction("failed to unwrap tx".to_string()))?;
        let tx = tx.into_inner();
        // Note: Mutex::into_inner() returns the inner value without locking since
        // we know the lock is not held (f has completed and released any guards)

        tx.commit()
            .await
            .map_err(|e| DatastoreError::Transaction(format!("commit tx failed: {e}")))?;

        Ok(())
    }
}
