//! Shared test helpers for coordinator handler integration tests.
//!
//! Provides [`TestEnv`] which creates a real PostgreSQL datastore and
//! in-memory broker, with automatic table truncation for test isolation.

use std::sync::Arc;

use tork::Broker;

/// Wraps a PostgresDatastore + InMemoryBroker for integration tests.
pub struct TestEnv {
    pub ds: Arc<datastore::postgres::PostgresDatastore>,
    pub broker: Arc<dyn Broker>,
}

impl TestEnv {
    /// Creates a new test environment connected to the real PostgreSQL database.
    ///
    /// # Panics
    ///
    /// Panics if the database connection fails.
    pub async fn new() -> Self {
        let dsn = "host=localhost user=tork password=tork dbname=tork port=5432 sslmode=disable";
        let options = datastore::postgres::Options {
            disable_cleanup: true,
            ..datastore::postgres::Options::default()
        };
        let ds = datastore::postgres::PostgresDatastore::new(dsn, options)
            .await
            .expect("failed to connect to postgres for integration tests");

        let broker: Arc<dyn Broker> =
            Arc::new(tork_runtime::broker::inmemory::new_in_memory_broker());

        Self {
            ds: Arc::new(ds),
            broker,
        }
    }

    /// Truncates all core tables to prevent state leakage between tests.
    ///
    /// # Panics
    ///
    /// Panics if the SQL query fails.
    pub async fn cleanup(&self) {
        let pool = self.ds.pool();
        sqlx::query("TRUNCATE tasks_log_parts, tasks, jobs, nodes, scheduled_jobs CASCADE")
            .execute(pool)
            .await
            .expect("failed to truncate tables for integration test cleanup");
    }
}

/// Helper to generate a UUID string matching Go's uuid.NewUUID().
#[must_use]
pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}
