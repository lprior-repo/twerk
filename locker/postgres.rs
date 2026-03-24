//! `PostgreSQL` locker implementation using advisory locks.
//!
//! Uses `pg_try_advisory_xact_lock` which is transaction-scoped:
//! the advisory lock is held as long as the transaction remains open,
//! and is released when the transaction is rolled back or the connection
//! is closed (`PostgreSQL` auto-aborts open transactions on disconnect).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![allow(clippy::pedantic)]

use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use postgres::Client as PgClient;
use sha2::{Digest, Sha256};

use crate::error::{InitError, LockError};
use crate::{Lock, Locker};

/// PostgreSQL-backed distributed locker.
pub struct PostgresLocker {
    pool: Arc<SyncPostgresPool>,
}

/// Wrapper for raw pool pointer that safely implements Send + Sync.
#[derive(Clone)]
struct PoolRef {
    ptr: *const SyncPostgresPool,
}

// SAFETY: PoolRef is only created from Arc<SyncPostgresPool> and the pool
// is guaranteed to be alive (Arc keeps it alive). The pointer is valid
// for the lifetime of the Arc. Since SyncPostgresPool contains only
// Send + Sync fields (parking_lot::Mutex), it's safe to share across threads.
unsafe impl Send for PoolRef {}
unsafe impl Sync for PoolRef {}

impl PoolRef {
    fn new(pool: &Arc<SyncPostgresPool>) -> Self {
        Self {
            ptr: Arc::as_ptr(pool) as *const SyncPostgresPool,
        }
    }

    fn put(&self, client: PgClient, created_at: Instant) {
        // SAFETY: The pointer is valid for the lifetime of the pool,
        // which is guaranteed to outlive PooledClient since Arc keeps it alive.
        unsafe {
            let pool = &*self.ptr;
            pool.put(client, created_at);
        }
    }
}

/// Inner lock type for PostgreSQL advisory locking.
struct PostgresLock {
    #[allow(dead_code)]
    key: String,
    client: Mutex<Option<PgClient>>,
    pool: PoolRef,
    created_at: Instant,
}

// ── Data (pure) ──────────────────────────────────────────────

#[must_use]
pub fn hash_key(key: &str) -> i64 {
    let result = Sha256::digest(key.as_bytes());
    i64::from_be_bytes([
        result[0], result[1], result[2], result[3], result[4], result[5], result[6], result[7],
    ])
}

// ── Pool (data) ──────────────────────────────────────────────

struct SyncPostgresPool {
    dsn: String,
    max_open: u32,
    max_idle: Option<u32>,
    conn_max_lifetime: Option<std::time::Duration>,
    conn_max_idle_time: Option<std::time::Duration>,
    connect_timeout: std::time::Duration,
    idle: Mutex<Vec<IdleConnection>>,
    open_count: Mutex<u32>,
}

struct IdleConnection {
    client: PgClient,
    created_at: Instant,
    last_used: Instant,
}

impl SyncPostgresPool {
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

    fn get(&self) -> Result<PooledClient, LockError> {
        {
            let mut idle = self.idle.lock();

            while let Some(pooled) = idle.pop() {
                if let Some(max_idle_time) = self.conn_max_idle_time {
                    if pooled.last_used.elapsed() > max_idle_time {
                        let _ = pooled.client.close();
                        let mut count = self.open_count.lock();
                        *count = count.saturating_sub(1);
                        continue;
                    }
                }

                if let Some(max_lifetime) = self.conn_max_lifetime {
                    if pooled.created_at.elapsed() > max_lifetime {
                        let _ = pooled.client.close();
                        let mut count = self.open_count.lock();
                        *count = count.saturating_sub(1);
                        continue;
                    }
                }

                // SAFETY: self is valid for lifetime of PooledClient
                let pool_ref = unsafe {
                    PoolRef::new(&Arc::from_raw(self as *const SyncPostgresPool))
                };
                return Ok(PooledClient {
                    pool: pool_ref,
                    client: Some(pooled.client),
                    created_at: pooled.created_at,
                });
            }
        }

        {
            let mut count = self.open_count.lock();
            if *count >= self.max_open {
                return Err(LockError::Connection(format!(
                    "pool exhausted: max_open_conns={} reached",
                    self.max_open
                )));
            }
            *count += 1;
        }

        let client = self.connect_with_timeout()?;

        // SAFETY: self is valid for lifetime of PooledClient
        let pool_ref = unsafe {
            PoolRef::new(&Arc::from_raw(self as *const SyncPostgresPool))
        };

        Ok(PooledClient {
            pool: pool_ref,
            client: Some(client),
            created_at: Instant::now(),
        })
    }

    fn connect_with_timeout(&self) -> Result<PgClient, LockError> {
        let dsn = self.dsn.clone();
        let timeout = self.connect_timeout;

        let handle = std::thread::spawn(move || PgClient::connect(&dsn, postgres::NoTls));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| LockError::Connection(format!("failed to create runtime: {e}")))?;

        let result = rt.block_on(async {
            tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || handle.join())).await
        });

        match result {
            Ok(Ok(Ok(Ok(client)))) => Ok(client),
            Ok(Ok(Ok(Err(e)))) => Err(LockError::Connection(e.to_string())),
            Ok(Ok(Err(_panic))) => {
                Err(LockError::Connection("connect thread panicked".to_string()))
            }
            Ok(Err(_join_err)) => Err(LockError::Connection("spawn failed".to_string())),
            Err(_) => Err(LockError::Connection(format!(
                "connection timed out after {timeout:?}"
            ))),
        }
    }

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

struct PooledClient {
    pool: PoolRef,
    client: Option<PgClient>,
    created_at: Instant,
}

impl PooledClient {
    #[allow(clippy::unwrap_used)]
    fn take_client(mut self) -> PgClient {
        self.client.take().unwrap()
    }

    fn pool_ref(&self) -> PoolRef {
        self.pool.clone()
    }
}

impl Drop for PooledClient {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            self.pool.put(client, self.created_at);
        }
    }
}

// ── Options (data) ───────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PostgresLockerOptions {
    max_open_conns: Option<u32>,
    max_idle_conns: Option<u32>,
    conn_max_lifetime: Option<std::time::Duration>,
    conn_max_idle_time: Option<std::time::Duration>,
    connect_timeout: Option<std::time::Duration>,
}

impl PostgresLockerOptions {
    #[must_use]
    pub fn max_open_conns(self, n: u32) -> Self {
        Self {
            max_open_conns: Some(n),
            ..self
        }
    }

    #[must_use]
    pub fn max_idle_conns(self, n: u32) -> Self {
        Self {
            max_idle_conns: Some(n),
            ..self
        }
    }

    #[must_use]
    pub fn conn_max_lifetime(self, d: std::time::Duration) -> Self {
        Self {
            conn_max_lifetime: Some(d),
            ..self
        }
    }

    #[must_use]
    pub fn conn_max_idle_time(self, d: std::time::Duration) -> Self {
        Self {
            conn_max_idle_time: Some(d),
            ..self
        }
    }

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
        // ManuallyDrop prevents Drop from running on this
        let this = ManuallyDrop::new(*Pin::into_inner(self));
        
        // SAFETY: We're consuming self and putting it in ManuallyDrop, so
        // we take full ownership. We won't use this again after moving out.
        let client = unsafe { std::ptr::read(&this.client) };
        let pool = this.pool.clone();
        let created_at = this.created_at;

        // Spawn thread to do ROLLBACK and return the client
        let handle = std::thread::spawn(move || {
            let mut guard = client.lock();
            let mut client = guard.take();
            if let Some(ref mut c) = client {
                let _ = c.simple_query("ROLLBACK");
            }
            client
        });

        Box::pin(async move {
            let client = tokio::task::spawn_blocking(move || {
                handle.join().unwrap_or(None)
            })
            .await
            .unwrap_or(None);

            if let Some(c) = client {
                pool.put(c, created_at);
            }

            Ok(())
        })
    }
}

impl Drop for PostgresLock {
    fn drop(&mut self) {
        if let Some(client) = self.client.get_mut().take() {
            self.pool.put(client, self.created_at);
        }
    }
}

// ── Locker implementation (actions) ──────────────────────────

impl PostgresLocker {
    pub async fn new(dsn: &str) -> Result<Self, InitError> {
        Self::with_options(dsn, PostgresLockerOptions::default()).await
    }

    pub async fn with_options(dsn: &str, opts: PostgresLockerOptions) -> Result<Self, InitError> {
        let pool = Arc::new(SyncPostgresPool::new(dsn, &opts));

        let timeout = opts
            .connect_timeout
            .unwrap_or(std::time::Duration::from_secs(30));
        let dsn_owned = dsn.to_string();

        let handle = std::thread::spawn(move || {
            PgClient::connect(&dsn_owned, postgres::NoTls).map(|_client| ())
        });

        let connect_result =
            tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || handle.join())).await;

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

            // Get these before consuming pooled
            let pool_ref = pooled.pool_ref();
            let created_at = pooled.created_at;
            let client = pooled.take_client();

            acquire_advisory_lock(key, key_hash, client, pool_ref, created_at)
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

fn acquire_advisory_lock(
    key: String,
    key_hash: i64,
    mut client: PgClient,
    pool: PoolRef,
    created_at: Instant,
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
        client: Mutex::new(Some(client)),
        pool,
        created_at,
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
        let locker = PostgresLocker::new(dsn)
            .await
            .expect("locker should be created");
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
