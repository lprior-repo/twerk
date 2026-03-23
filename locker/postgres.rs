//! `PostgreSQL` locker implementation using advisory locks.
//!
//! Uses `pg_try_advisory_xact_lock` which is transaction-scoped:
//! the advisory lock is held as long as the transaction remains open,
//! and is released when the transaction is rolled back or the connection
//! is closed (`PostgreSQL` auto-aborts open transactions on disconnect).
//!
//! # Executor HRTB workaround
//!
//! sqlx's [`sqlx::Executor`] trait has a known limitation: the compiler
//! cannot prove `for<'c> &'c mut PgConnection: Executor<'c>` in the
//! auto-trait inference context used by `Box<dyn Future + Send>` coercion
//! and `tokio::spawn`. This is why the sqlx codebase itself comments out
//! the `Executor` impl for `Transaction` ("fails to compile due to lack
//! of lazy normalization").
//!
//! We work around this by using the synchronous [`postgres`] crate for all
//! SQL operations, executed on tokio's blocking thread pool via
//! [`tokio::task::spawn_blocking`]. The synchronous `postgres::Client` is
//! `Send` but not `Sync`, so we wrap it in [`std::sync::Mutex`] to satisfy
//! the `Lock: Send + Sync` trait bound.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use std::pin::Pin;
use std::sync::Mutex;

use postgres::Client as PgClient;
use sha2::{Digest, Sha256};

use crate::error::{InitError, LockError};
use crate::{Lock, Locker};

/// PostgreSQL-backed distributed locker.
///
/// Uses `PostgreSQL` advisory locks (`pg_try_advisory_xact_lock`) for
/// distributed locking across multiple processes or machines.
pub struct PostgresLocker {
    dsn: String,
}

/// Inner lock type for `PostgreSQL` advisory locking.
///
/// Stores a synchronous [`PgClient`] (wrapped in [`Mutex`] for `Sync`)
/// with its open transaction. The lock is released when
/// [`Lock::release_lock`] rolls back and drops the client.
///
/// # Why Mutex?
///
/// `postgres::Client` is `Send` but not `Sync` (it contains
/// `Box<dyn Stream + Send>`). The `Lock` trait requires `Send + Sync`.
/// `Mutex<T>` is `Sync` when `T: Send`, so this satisfies the bound.
struct PostgresLock {
    #[allow(dead_code)]
    key: String,
    client: Mutex<PgClient>,
}

// ── Data (pure) ──────────────────────────────────────────────

/// Compute a 64-bit hash from a key string using SHA-256.
///
/// Takes the first 8 bytes of the SHA-256 digest and interprets them
/// as a big-endian i64, matching the Go implementation exactly.
#[must_use]
pub fn hash_key(key: &str) -> i64 {
    let result = Sha256::digest(key.as_bytes());
    i64::from_be_bytes([
        result[0], result[1], result[2], result[3],
        result[4], result[5], result[6], result[7],
    ])
}

// ── Options (data) ───────────────────────────────────────────

/// Options for configuring the [`PostgresLocker`].
#[derive(Debug, Clone, Default)]
pub struct PostgresLockerOptions {
    #[allow(dead_code)]
    max_open_conns: Option<u32>,
    #[allow(dead_code)]
    max_idle_conns: Option<u32>,
    #[allow(dead_code)]
    conn_max_lifetime: Option<std::time::Duration>,
    #[allow(dead_code)]
    conn_max_idle_time: Option<std::time::Duration>,
    connect_timeout: Option<std::time::Duration>,
}

impl PostgresLockerOptions {
    /// Set the maximum number of open connections.
    #[must_use]
    pub fn max_open_conns(self, n: u32) -> Self {
        Self {
            max_open_conns: Some(n),
            ..self
        }
    }

    /// Set the maximum number of idle connections.
    #[must_use]
    pub fn max_idle_conns(self, n: u32) -> Self {
        Self {
            max_idle_conns: Some(n),
            ..self
        }
    }

    /// Set the maximum lifetime for individual connections.
    #[must_use]
    pub fn conn_max_lifetime(self, d: std::time::Duration) -> Self {
        Self {
            conn_max_lifetime: Some(d),
            ..self
        }
    }

    /// Set the maximum idle time for individual connections.
    #[must_use]
    pub fn conn_max_idle_time(self, d: std::time::Duration) -> Self {
        Self {
            conn_max_idle_time: Some(d),
            ..self
        }
    }

    /// Set the connection timeout.
    #[must_use]
    pub fn connect_timeout(self, d: std::time::Duration) -> Self {
        Self {
            connect_timeout: Some(d),
            ..self
        }
    }
}

// ── Lock implementation (actions) ────────────────────────────

impl Lock for PostgresLock {
    fn release_lock(
        self: Pin<Box<Self>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), LockError>> + Send>> {
        let PostgresLock { client, .. } = *Pin::into_inner(self);

        // Rollback on a separate OS thread (see with_options comment),
        // then drop the client. Sending ROLLBACK ensures the advisory
        // lock is released immediately rather than waiting for TCP
        // keepalive timeout.
        let handle = std::thread::spawn(move || {
            if let Ok(mut c) = client.lock() {
                let _ = c.simple_query("ROLLBACK");
            }
        });

        Box::pin(async move {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = handle.join();
            })
            .await;
            Ok(())
        })
    }
}

// ── Locker implementation (actions) ──────────────────────────

impl PostgresLocker {
    /// Create a new [`PostgresLocker`] with default options.
    ///
    /// # Errors
    ///
    /// Returns [`InitError`] if the connection cannot be established.
    pub async fn new(dsn: &str) -> Result<Self, InitError> {
        Self::with_options(dsn, PostgresLockerOptions::default()).await
    }

    /// Create a new [`PostgresLocker`] with custom options.
    ///
    /// # Errors
    ///
    /// Returns [`InitError`] if the connection cannot be established.
    pub async fn with_options(
        dsn: &str,
        opts: PostgresLockerOptions,
    ) -> Result<Self, InitError> {
        let timeout = opts.connect_timeout.unwrap_or(std::time::Duration::from_secs(30));
        let dsn_owned = dsn.to_string();

        // Use std::thread::spawn to escape tokio's runtime context.
        // The postgres crate's Client::connect creates its own tokio
        // runtime internally, and Client::drop calls close_inner which
        // also creates one. Both must run on a thread with no context.
        // We connect, verify, and drop on this thread — the client is
        // not needed after construction (each lock opens its own conn).
        let handle = std::thread::spawn(move || {
            PgClient::connect(&dsn_owned, postgres::NoTls).map(|_client| ())
        });

        let connect_result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || handle.join()),
        )
        .await;

        match connect_result {
            Ok(Ok(Ok(Ok(())))) => Ok(Self {
                dsn: dsn.to_string(),
            }),
            Ok(Ok(Ok(Err(e)))) => Err(InitError::Connection(e.to_string())),
            Ok(Ok(Err(_panic))) => Err(InitError::Ping("connect thread panicked".to_string())),
            Ok(Err(_join_err)) => Err(InitError::Ping("spawn failed".to_string())),
            Err(_) => Err(InitError::Connection(format!(
                "connection timed out after {timeout:?}"
            ))),
        }
    }

    /// Create a new builder for [`PostgresLocker`].
    #[must_use]
    pub fn builder() -> PostgresLockerOptions {
        PostgresLockerOptions::default()
    }
}

impl Locker for PostgresLocker {
    fn acquire_lock(
        &self,
        key: &str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Pin<Box<dyn Lock>>, LockError>> + Send>>
    {
        let key = key.to_string();
        let dsn = self.dsn.clone();

        // Run all SQL on a completely separate OS thread — the synchronous
        // postgres crate creates its own tokio runtime internally, which
        // conflicts with tokio 1.x's context propagation on spawn_blocking
        // threads.
        let handle = std::thread::spawn(move || {
            let key_hash = hash_key(&key);

            // Open a dedicated connection — this is our "session" that holds the lock.
            let client = PgClient::connect(&dsn, postgres::NoTls)
                .map_err(|e| LockError::Connection(e.to_string()))?;

            acquire_advisory_lock(key, key_hash, client)
        });

        Box::pin(async move {
            match tokio::task::spawn_blocking(move || handle.join()).await {
                Ok(Ok(lock)) => lock,
                Ok(Err(panic_payload)) => {
                    let msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        (*s).to_string()
                    } else {
                        "thread panicked".to_string()
                    };
                    Err(LockError::Connection(msg))
                }
                Err(e) => Err(LockError::Connection(format!("spawn failed: {e}"))),
            }
        })
    }
}

/// Core advisory lock acquisition logic. Runs on a blocking thread.
///
/// Opens a transaction, attempts `pg_try_advisory_xact_lock`, and
/// returns a [`PostgresLock`] holding the connection on success.
/// On failure, rolls back the transaction and returns an error.
fn acquire_advisory_lock(
    key: String,
    key_hash: i64,
    mut client: PgClient,
) -> Result<Pin<Box<dyn Lock>>, LockError> {
    // Begin a transaction on this connection.
    client
        .simple_query("BEGIN")
        .map_err(|e| LockError::Transaction {
            key: key.clone(),
            source: Box::new(e),
        })?;

    // Attempt to acquire the advisory lock within the transaction.
    let row = client
        .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&key_hash])
        .map_err(|e| LockError::Transaction {
            key: key.clone(),
            source: Box::new(e),
        })?;

    let lock_acquired: bool = row.get(0);

    if !lock_acquired {
        // Lock not obtained — roll back the transaction.
        let _ = client.simple_query("ROLLBACK");
        return Err(LockError::AlreadyLocked { key });
    }

    // Lock acquired — store the connection (with its open transaction).
    // The advisory lock is held as long as the transaction remains open.
    // It will be released when release_lock() rolls back the transaction.
    let lock: Pin<Box<dyn Lock>> = Box::pin(PostgresLock {
        key,
        client: Mutex::new(client),
    });
    Ok(lock)
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_key() {
        let key = "2c7eb7e1951343468ce360c906003a22";
        let hash = hash_key(key);
        // Go reference: int64(-414568140838410356)
        let expected = i64::from_be_bytes([250, 63, 40, 120, 238, 33, 231, 140]);
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_hash_key_deterministic() {
        let key = "my-lock-key";
        let a = hash_key(key);
        let b = hash_key(key);
        assert_eq!(a, b);
    }

    #[test]
    fn test_hash_key_different_keys() {
        let a = hash_key("key-a");
        let b = hash_key("key-b");
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn test_postgres_locker_new() {
        let dsn = "postgres://tork:tork@localhost:5432/tork";
        let locker = PostgresLocker::new(dsn).await;
        assert!(locker.is_ok());
    }

    #[tokio::test]
    async fn test_postgres_locker_acquire_lock() {
        let dsn = "postgres://tork:tork@localhost:5432/tork";
        let locker = PostgresLocker::new(dsn).await.expect("locker should be created");
        let key = "test_key";

        // First acquisition should succeed
        let lock = locker
            .acquire_lock(key)
            .await
            .expect("first acquire should succeed");

        // Second acquisition for same key should fail (lock held by first)
        let result = locker.acquire_lock(key).await;
        assert!(result.is_err());

        // Release and reacquire should work
        lock.release_lock().await.expect("release should succeed");

        let lock2 = locker
            .acquire_lock(key)
            .await
            .expect("reacquire should succeed");
        lock2.release_lock().await.expect("release should succeed");
    }
}
