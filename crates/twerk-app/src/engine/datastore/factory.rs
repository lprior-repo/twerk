//! Datastore factory module
//!
//! Factory functions and configuration helpers for creating datastore implementations.

use anyhow::Result;
use twerk_infrastructure::datastore::{inmemory::InMemoryDatastore, Datastore};

use super::super::engine_helpers::{ensure_config_loaded, env_string, env_string_default};
use twerk_common::constants::DEFAULT_POSTGRES_DSN;

// ── Typed errors for datastore factory ─────────────────────────────

#[derive(Debug, thiserror::Error)]
enum DatastoreFactoryError {
    #[error("unable to connect to postgres: {0}")]
    PostgresConnection(String),
    #[error("unknown datastore type: {0}")]
    UnknownDatastoreType(String),
}

// ── Factory functions ──────────────────────────────────────────

/// Creates a datastore based on configuration.
///
/// # Errors
///
/// Returns an error if:
/// - The datastore type is unknown
/// - The Postgres connection cannot be established
pub async fn create_datastore() -> Result<Box<dyn Datastore + Send + Sync>> {
    ensure_config_loaded();
    let dstype = env_string_default("datastore.type", "postgres");

    match dstype.as_str() {
        "postgres" => {
            let dsn = env_string_default("datastore.postgres.dsn", DEFAULT_POSTGRES_DSN);
            let opts = twerk_infrastructure::datastore::Options {
                encryption_key: Some(env_string("datastore.encryption.key"))
                    .filter(|s| !s.is_empty()),
                ..Default::default()
            };
            let pg = twerk_infrastructure::datastore::postgres::PostgresDatastore::new(&dsn, opts)
                .await
                .map_err(|e| DatastoreFactoryError::PostgresConnection(e.to_string()))?;
            Ok(Box::new(pg))
        }
        "inmemory" => Ok(Box::new(InMemoryDatastore::new())),
        other => Err(DatastoreFactoryError::UnknownDatastoreType(other.to_string()).into()),
    }
}

/// Creates a new in-memory datastore.
#[must_use]
pub fn new_inmemory_datastore() -> Box<dyn Datastore + Send + Sync> {
    Box::new(InMemoryDatastore::new())
}

/// Creates a new in-memory datastore wrapped in an `Arc`.
#[must_use]
pub fn new_inmemory_datastore_arc() -> std::sync::Arc<dyn Datastore> {
    std::sync::Arc::new(InMemoryDatastore::new())
}
