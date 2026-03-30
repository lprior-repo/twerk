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
pub use twerk_infrastructure::locker::InMemoryLocker;
pub use twerk_infrastructure::locker::Locker;

// Re-export the Lock trait
pub use twerk_infrastructure::locker::Lock;

// Re-export locker errors
pub use twerk_infrastructure::locker::LockError;

// Re-export locker type constants
pub use twerk_infrastructure::locker::{LOCKER_INMEMORY, LOCKER_POSTGRES};

/// Boxed future for locker operations
pub type BoxedFuture<T> = Pin<
    Box<
        dyn std::future::Future<Output = Result<T, twerk_infrastructure::locker::LockError>> + Send,
    >,
>;

/// Default DSN for PostgreSQL connections (matches Go default)
const DEFAULT_POSTGRES_DSN: &str =
    "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable";

/// Creates a locker based on configuration.
///
/// Matches Go `createLocker()`:
/// - `"inmemory"` → [`InMemoryLocker`]
/// - `"postgres"` → [`twerk_infrastructure::locker::PostgresLocker`] with full config
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

            let opts = twerk_infrastructure::locker::postgres::PostgresLockerOptions::default()
                .max_open_conns(max_open as u32)
                .max_idle_conns(max_idle as u32)
                .conn_max_lifetime(lifetime)
                .conn_max_idle_time(idle_time);

            let pg_locker =
                twerk_infrastructure::locker::postgres::PostgresLocker::with_options(&dsn, opts)
                    .await
                    .map_err(|e| anyhow::anyhow!("unable to connect to postgres locker: {}", e))?;

            Ok(Box::new(pg_locker))
        }
        other => Err(anyhow::anyhow!("unknown locker type: {}", other)),
    }
}

// ── Config helpers (pure) ──────────────────────────────────────

/// Get a string from environment variables (`TWERK_` prefix, dots → underscores).
fn env_string(key: &str) -> String {
    let env_key = format!("TWERK_{}", key.to_uppercase().replace('.', "_"));
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
mod tests {
    use std::time::Duration;

    use super::{
        create_locker, env_duration_default, env_int_default, env_string_default, LOCKER_INMEMORY,
    };

    // ── create_locker ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_locker_inmemory_returns_ok() {
        let locker = create_locker(LOCKER_INMEMORY).await;
        assert!(locker.is_ok(), "inmemory locker creation must succeed");
    }

    #[tokio::test]
    async fn create_locker_unknown_type_returns_err() {
        let result = create_locker("unsupported-locker-type").await;
        match result {
            Ok(_) => panic!("expected error for unknown locker type"),
            Err(e) => {
                let msg = format!("{e}");
                assert!(
                    msg.contains("unknown locker type"),
                    "error message should mention 'unknown locker type', got: {msg}"
                );
            }
        }
    }

    // ── env_string_default ────────────────────────────────────────────────────

    #[test]
    fn env_string_default_returns_default_when_unset() {
        let got = env_string_default("twerk.test.definitely_not_set_abc123", "fallback");
        assert_eq!(got, "fallback");
    }

    #[test]
    fn env_string_default_returns_env_value_when_set() {
        let env_key = "TWERK_TEST_LOCKER_STR";
        std::env::set_var(env_key, "from-env");
        let got = env_string_default("test.locker.str", "fallback");
        std::env::remove_var(env_key);
        assert_eq!(got, "from-env");
    }

    // ── env_int_default ───────────────────────────────────────────────────────

    #[test]
    fn env_int_default_returns_default_when_unset() {
        let got = env_int_default("twerk.test.definitely_not_set_int_abc123", 42);
        assert_eq!(got, 42);
    }

    #[test]
    fn env_int_default_parses_valid_integer_from_env() {
        let env_key = "TWERK_TEST_LOCKER_INT";
        std::env::set_var(env_key, "99");
        let got = env_int_default("test.locker.int", 0);
        std::env::remove_var(env_key);
        assert_eq!(got, 99);
    }

    #[test]
    fn env_int_default_falls_back_on_invalid_value() {
        let env_key = "TWERK_TEST_LOCKER_INT_BAD";
        std::env::set_var(env_key, "not-a-number");
        let got = env_int_default("test.locker.int.bad", 7);
        std::env::remove_var(env_key);
        assert_eq!(got, 7);
    }

    // ── env_duration_default ──────────────────────────────────────────────────

    #[test]
    fn env_duration_default_returns_default_when_unset() {
        let default = Duration::from_secs(60);
        let got = env_duration_default("twerk.test.definitely_not_set_dur_abc123", default);
        assert_eq!(got, default);
    }

    #[test]
    fn env_duration_default_parses_seconds_from_env() {
        let env_key = "TWERK_TEST_LOCKER_DUR";
        std::env::set_var(env_key, "120");
        let got = env_duration_default("test.locker.dur", Duration::from_secs(0));
        std::env::remove_var(env_key);
        assert_eq!(got, Duration::from_secs(120));
    }

    #[test]
    fn env_duration_default_falls_back_on_invalid_value() {
        let env_key = "TWERK_TEST_LOCKER_DUR_BAD";
        let default = Duration::from_secs(30);
        std::env::set_var(env_key, "not-a-duration");
        let got = env_duration_default("test.locker.dur.bad", default);
        std::env::remove_var(env_key);
        assert_eq!(got, default);
    }
}
