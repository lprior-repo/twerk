//! Datastore factory module
//!
//! Factory functions and configuration helpers for creating datastore implementations.

use std::env;

use anyhow::{anyhow, Result};
use twerk_infrastructure::datastore::{inmemory::InMemoryDatastore, Datastore};

use super::super::engine_helpers::ensure_config_loaded;

// ── Constants ──────────────────────────────────────────────────

const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable";

// ── Config helpers ─────────────────────────────────────────────

/// Retrieves an environment variable, returning an empty string if not set.
///
/// This is intentional for optional configuration values where missing env vars
/// should be treated as empty strings rather than errors.
fn env_string(key: &str) -> String {
    let env_key = format!("TWERK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| twerk_infrastructure::config::string(key))
}

fn env_string_default(key: &str, default: &str) -> String {
    let v = env_string(key);
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
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
                .map_err(|e| anyhow!("unable to connect to postgres: {}", e))?;
            Ok(Box::new(pg))
        }
        "inmemory" => Ok(Box::new(InMemoryDatastore::new())),
        other => Err(anyhow!("unknown datastore type: {}", other)),
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
