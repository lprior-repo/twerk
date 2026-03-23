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
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use postgres::Client as PgClient;
use sha2::{Digest, Sha256};

use crate::error::{InitError, LockError};
use crate::{Lock, Locker};

/// PostgreSQL-backed distributed locker.
///
/// Uses `PostgreSQL` advisory locks (`pg_try_advisory_xact_lock`) for
/// distributed locking across multiple processes or machines.
pub struct PostgresLocker {
    pool: Arc<SyncPostgresPool>,
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

// ── Pool (data) ──────────────────────────────────────────────

/// A synchronous PostgreSQL connection pool that respects pool options.
///
/// Manages a collection of reusable connections according to the
/// configured pool options.
struct SyncPostgresPool {
    /// Connection string.
    dsn: String,
    /// Maximum total open connections.
    max_open: u32,
    /// Maximum idle connections in pool.
    max_idle: Option<u32>,
    /// Maximum lifetime for connections.
    conn_max_lifetime: Option<std::time::Duration>,
    /// Maximum idle time for connections.
    conn_max_idle_time: Option<std::time::Duration>,
    /// Connection timeout.
    connect_timeout: std::time::Duration,
    /// Pool of idle connections.
    idle: Mutex<Vec<IdleConnection>>,
    /// Number of currently open connections.
    open_count: Mutex<u32>,
}

/// An idle connection with metadata.
struct IdleConnection {
    /// The PostgreSQL client.
    client: PgClient,
    /// When the connection was created.
    created_at: Instant,
    /// When the connection was last returned to the pool.
    last_used: Instant,
}

impl SyncPostgresPool {
    /// Create a new pool with the given options.
    #[must_use]
    fn new(dsn: &str, opts: &PostgresLockerOptions) -> Self {
        Self {
            dsn: dsn.to_string(),
            max_open: opts.max_open_conns.unwrap_or(100),
            max_idle: opts.max_idle_conns,
            conn_max_lifetime: opts.conn_max_lifetime,
            conn_max_idle_time: opts.conn_max_idle_time,
            connect_timeout: opts
                .connect_timeout
                .unwrap_or(std::time::Duration::from_secs(30)),
            idle: Mutex::new(Vec::new()),
            open_count: Mutex::new(0),
        }
    }

    /// Get a connection from the pool, creating a new one if necessary.
    fn get(&self) -> Result<PooledClient, LockError> {
        // Try to get a valid idle connection
        {
            let mut idle = self.idle.lock();

            while let Some(pooled) = idle.pop() {
                // Check if connection has exceeded max idle time
                if let Some(max_idle_time) = self.conn_max_idle_time {
                    if pooled.last_used.elapsed() > max_idle_time {
                        let _ = pooled.client.close();
                        let mut count = self.open_count.lock();
                        *count = count.saturating_sub(1);
                        continue;
                    }
                }

                // Check if connection has exceeded max lifetime
                if let Some(max_lifetime) = self.conn_max_lifetime {
                    if pooled.created_at.elapsed() > max_lifetime {
                        let _ = pooled.client.close();
                        let mut count = self.open_count.lock();
                        *count = count.saturating_sub(1);
                        continue;
                    }
                }

                // Valid connection found
                return Ok(PooledClient {
                    pool: Arc::new(()),
                    client: Some(pooled.client),
                    created_at: pooled.created_at,
                    pool_ptr: self as *const SyncPostgresPool,
                });
            }
        }

        // No valid idle connection, check if we can open a new one
        {
            let mut count = self.open_count.lock();
            if *count >= self.max_open {
                return Err(LockError::Connection(
                    format!("pool exhausted: max_open_conns={} reached", self.max_open)
                ));
            }
            *count += 1;
        }

        // Create new connection with timeout
        let client = self.connect_with_timeout()?;

        Ok(PooledClient {
            pool: Arc::new(()),
            client: Some(client),
            created_at: Instant::now(),
            pool_ptr: self as *const SyncPostgresPool,
        })
    }

    /// Connect with timeout.
    fn connect_with_timeout(&self) -> Result<PgClient, LockError> {
        let dsn = self.dsn.clone();
        let timeout = self.connect_timeout;

        let handle = std::thread::spawn(move || {
            PgClient::connect(&dsn, postgres::NoTls)
        });

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| LockError::Connection(format!("failed to create runtime: {e}")))?;

        let result = rt.block_on(async {
            tokio::time::timeout(
                timeout,
                tokio::task::spawn_blocking(move || handle.join()),
            )
            .await
        });

        // result is Result<Result<Result<PgClient, Error>, JoinError>, Elapsed>
        match result {
            Ok(Ok(Ok(Ok(client)))) => Ok(client),
            Ok(Ok(Ok(Err(e)))) => Err(LockError::Connection(e.to_string())),
            Ok(Ok(Err(_panic))) => Err(LockError::Connection("connect thread panicked".to_string())),
            Ok(Err(_join_err)) => Err(LockError::Connection("spawn failed".to_string())),
            Err(_) => Err(LockError::Connection(format!(
                "connection timed out after {timeout:?}"
            ))),
        }
    }

    /// Return a connection to the pool.
    fn put(&self, client: PgClient, created_at: Instant) {
        let mut idle = self.idle.lock();

        if let Some(max_idle) = self.max_idle {
            if idle.len() >= max_idle as usize {
                let _ = client.close();
                let mut count = self.open_count.lock();
                *count = count.saturating_sub(1);
                return;
            }
        }

        idle.push(IdleConnection {
            client,
            created_at,
            last_used: Instant::now(),
        });
    }
}

/// A checked-out connection that is returned to the pool on drop.
struct PooledClient {
    #[allow(dead_code)]
    pool: Arc<()>,
    client: Option<PgClient>,
    created_at: Instant,
    pool_ptr: *const SyncPostgresPool,
}

impl PooledClient {
    #[allow(clippy::unwrap_used)]
    fn into_inner(mut self) -> PgClient {
        self.client.take().unwrap()
    }
}

impl Drop for PooledClient {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            unsafe {
                let pool = &*self.pool_ptr;
                pool.put(client, self.created_at);
            }
        }
    }
}

// ── Options (data) ───────────────────────────────────────────

/// Options for configuring the [`PostgresLocker`].
#[derive(Debug, Clone, Default)]
pub struct PostgresLockerOptions {
    max_open_conns: Option<u32>,
    max_idle_conns: Option<u32>,
    conn_max_lifetime: Option<std::time::Duration>,
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

        let handle = std::thread::spawn(move || {
            let mut guard = client.lock();
            let _ = guard.simple_query("ROLLBACK");
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
        let pool = Arc::new(SyncPostgresPool::new(dsn, &opts));

        let timeout = opts
            .connect_timeout
            .unwrap_or(std::time::Duration::from_secs(30));
        let dsn_owned = dsn.to_string();

        let handle = std::thread::spawn(move || {
            PgClient::connect(&dsn_owned, postgres::NoTls).map(|_client| ())
        });

        let connect_result = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || handle.join()),
        )
        .await;

        match connect_result {
            Ok(Ok(Ok(Ok(())))) => Ok(Self { pool }),
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
        let pool = Arc::clone(&self.pool);

        let handle = std::thread::spawn(move || {
            let key_hash = hash_key(&key);

            let pooled = pool.get()?;

            let client = pooled.into_inner();

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
    client
        .simple_query("BEGIN")
        .map_err(|e| LockError::Transaction {
            key: key.clone(),
            source: Box::new(e),
        })?;

    let row = client
        .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&key_hash])
        .map_err(|e| LockError::Transaction {
            key: key.clone(),
            source: Box::new(e),
        })?;

    let lock_acquired: bool = row.get(0);

    if !lock_acquired {
        let _ = client.simple_query("ROLLBACK");
        return Err(LockError::AlreadyLocked { key });
    }

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

        let lock = locker
            .acquire_lock(key)
            .await
            .expect("first acquire should succeed");

        let result = locker.acquire_lock(key).await;
        assert!(result.is_err());

        lock.release_lock().await.expect("release should succeed");

        let lock2 = locker
            .acquire_lock(key)
            .await
            .expect("reacquire should succeed");
        lock2.release_lock().await.expect("release should succeed");
    }
}
