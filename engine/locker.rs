//! Locker initialization module
//!
//! This module handles locker creation based on configuration.
//!
//! # Go Parity
//!
//! Matches `engine/initLocker()` and `engine/createLocker()` from Go:
//! - Defaults to `locker.type` or `datastore.type`, falling back to "inmemory"
//! - Supports "inmemory" and "postgres" locker types
//! - Reads all PostgreSQL options from environment variables
//! - Falls back to datastore DSN when locker DSN is not set

use std::env;
use std::pin::Pin;
use std::time::Duration;

// Re-export Locker trait and implementations from the external locker crate
pub use locker::InMemoryLocker;
pub use locker::Locker;

// Re-export the Lock trait
pub use locker::Lock;

// Re-export locker errors
pub use locker::LockError;

// Re-export locker type constants
pub use locker::{LOCKER_INMEMORY, LOCKER_POSTGRES};

/// Boxed future for locker operations
pub type BoxedFuture<T> =
    Pin<Box<dyn std::future::Future<Output = Result<T, locker::LockError>> + Send>>;

/// Default DSN for PostgreSQL connections (matches Go default)
const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=tork password=tork dbname=tork port=5432 sslmode=disable";

/// Creates a locker based on configuration.
///
/// Matches Go `createLocker()`:
/// - `"inmemory"` → [`InMemoryLocker`]
/// - `"postgres"` → [`locker::PostgresLocker`] with full config
///
/// # Errors
///
/// Returns an error if:
/// - An unknown locker type is specified
/// - The postgres connection cannot be established
pub async fn create_locker(
    locker_type: &str,
) -> Result<Box<dyn Locker + Send + Sync>, anyhow::Error> {
    match locker_type {
        LOCKER_INMEMORY => Ok(Box::new(InMemoryLocker::new())),
        LOCKER_POSTGRES => {
            let dsn = env_string_default(
                "locker.postgres.dsn",
                &env_string_default("datastore.postgres.dsn", DEFAULT_POSTGRES_DSN),
            );

            let max_open = env_int_default(
                "locker.postgres.max_open_conns",
                env_int_default("datastore.postgres.max_open_conns", 25),
            );
            let max_idle = env_int_default(
                "locker.postgres.max_idle_conns",
                env_int_default("datastore.postgres.max_idle_conns", 25),
            );
            let lifetime = env_duration_default(
                "locker.postgres.conn_max_lifetime",
                env_duration_default(
                    "datastore.postgres.conn_max_lifetime",
                    Duration::from_secs(3600),
                ),
            );
            let idle_time = env_duration_default(
                "locker.postgres.conn_max_idle_time",
                env_duration_default(
                    "datastore.postgres.conn_max_idle_time",
                    Duration::from_secs(300),
                ),
            );

            let opts = locker::postgres::PostgresLockerOptions::default()
                .max_open_conns(max_open as u32)
                .max_idle_conns(max_idle as u32)
                .conn_max_lifetime(lifetime)
                .conn_max_idle_time(idle_time);

            let pg_locker = locker::PostgresLocker::with_options(&dsn, opts)
                .await
                .map_err(|e| anyhow::anyhow!("unable to connect to postgres locker: {}", e))?;

            Ok(Box::new(pg_locker))
        }
        other => Err(anyhow::anyhow!("unknown locker type: {}", other)),
    }
}

// ── Config helpers (pure) ──────────────────────────────────────

/// Get a string from environment variables (`TORK_` prefix, dots → underscores).
fn env_string(key: &str) -> String {
    let env_key = format!("TORK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key).unwrap_or_default()
}

/// Get a string with default from environment variables.
fn env_string_default(key: &str, default: &str) -> String {
    let value = env_string(key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

/// Get an integer from environment variables with default.
fn env_int_default(key: &str, default: i32) -> i32 {
    let value = env_string(key);
    value.parse::<i32>().unwrap_or(default)
}

/// Get a [`Duration`] from environment variables with default (parsed as seconds).
fn env_duration_default(key: &str, default: Duration) -> Duration {
    let value = env_string(key);
    if value.is_empty() {
        default
    } else {
        value
            .parse::<u64>()
            .map(Duration::from_secs)
            .unwrap_or(default)
    }
}

#[cfg(test)]
mod locker_test;
