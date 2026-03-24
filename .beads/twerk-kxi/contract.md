---
bead_id: twerk-kxi
bead_title: Fix locker gaps: connection pool return, spawn_blocking, eager validation
phase: 1
updated_at: "2026-03-24T00:00:00Z"
---

# Contract Specification

## Context

### Feature

Fixing five implementation gaps in the Rust `locker` crate's `PostgresLocker` to achieve parity with the Go `tork` implementation:

1. **GAP1**: Connection leak — `PooledClient.into_inner()` bypasses the `Drop` impl, so the connection is never returned to the pool on successful lock acquisition
2. **GAP2**: `release_lock` uses raw `std::thread::spawn` instead of `tokio::task::spawn_blocking`
3. **GAP3**: Connection validation is deferred to first lock acquisition instead of validated eagerly at construction time (Go calls `db.Ping()` immediately)
4. **GAP5**: `PostgresLock.key` field is stored but never accessed (marked `#[allow(dead_code)]`)

### Domain Terms

| Term | Definition |
|------|------------|
| `PostgresLocker` | PostgreSQL-backed distributed advisory lock provider |
| `PostgresLock` | Concrete `Lock` implementation wrapping a `PgClient` |
| `PooledClient` | RAII guard — on `Drop`, returns the `PgClient` to the pool via `PoolRef::put` |
| `PoolRef` | `Send + Sync` wrapper around `*const SyncPostgresPool` for safe cross-thread pool access |
| `SyncPostgresPool` | Synchronous connection pool with `get()` / `put()` semantics |
| `IdleConnection` | `(PgClient, created_at: Instant, last_used: Instant)` tuple stored in the pool |
| `PostgresLockerOptions` | Builder for pool configuration (max connections, timeouts, etc.) |
| Advisory lock | `pg_try_advisory_xact_lock(key_hash)` — transaction-scoped, auto-releases on rollback/disconnect |

### Assumptions

- The `locker` crate is used by the `engine` crate and must remain `no_std`-compatible where possible
- `tokio` runtime is available in the async context; `spawn_blocking` is the correct primitive
- `PooledClient` existing `Drop` impl is the intended mechanism for returning connections
- The `key` field in `PostgresLock` is only needed for debugging/logging and is not required for correctness

### Open Questions

- None — all five gaps are fully characterized in `patches/locker_gaps.patch`

---

## Preconditions

### For `PostgresLocker::with_options`

- `dsn` must be a valid PostgreSQL connection string
- `opts` must have non-negative pool size values if provided
- The PostgreSQL server must be reachable at `dsn` within `connect_timeout`

### For `PostgresLocker::acquire_lock`

- The `PostgresLocker` instance must have been successfully initialized (via `new` or `with_options`)
- The pool must not be fully exhausted (i.e., `open_count < max_open` OR idle connections are available)
- The underlying `PgClient` must be able to execute SQL (not disconnected)

### For `PostgresLock::release_lock`

- The `PostgresLock` instance must hold a `Some(client)` — not already released

---

## Postconditions

### GAP1 Fix — Connection Returned to Pool

**After** `acquire_lock` returns `Ok(lock)`:
- The connection must be **returned to the pool** when `lock` is eventually dropped
- The `PooledClient` RAII guard must not be bypassed — `into_inner()` must not be called on the pooled client
- `open_count` in the pool must be decremented when the connection is returned

**After** `release_lock` completes:
- The `PostgresLock` must not hold a `Some(PgClient)` — it must be `None`
- The connection must be in the pool's idle list or closed (if `max_idle` exceeded)
- `open_count` must remain unchanged (connection recycled, not newly allocated)

### GAP2 Fix — `spawn_blocking` for `release_lock`

**After** `release_lock` completes:
- The `ROLLBACK` query must have been executed via `tokio::task::spawn_blocking`, not raw `std::thread::spawn`
- The returned future must be `Ready(Ok(()))` on success

### GAP3 Fix — Eager Connection Validation

**After** `with_options` returns `Ok(locker)`:
- A connection must have been established and validated (e.g., via `SELECT 1` or `Ping`)
- If validation fails, `with_options` must return `Err(InitError::Ping(_))` or `Err(InitError::Connection(_))`

### GAP5 Fix — Unused Key Field

**After** any `PostgresLock` construction:
- The `key: String` field must be either:
  - Removed entirely, OR
  - Actively used for debugging/tracing (with `#[allow(dead_code)]` removed)

---

## Invariants

### Pool Invariants

1. `open_count == idle.len() + number_of_active_clients` at all times
2. `open_count <= max_open` at all times
3. If `max_idle` is `Some(n)`, then `idle.len() <= n` at all times (after `put`)
4. All `IdleConnection` entries have `last_used >= created_at` and `last_used <= Instant::now()`

### Lock Lifecycle Invariants

5. A `PgClient` held by a `PostgresLock` is always in a valid transaction state (`BEGIN` executed, lock acquired)
6. After `release_lock` completes, the `PgClient` is no longer held by `PostgresLock` — either returned to pool or closed
7. `PooledClient` always either holds a `Some(PgClient)` or `None` — never double-returned

### Type Safety Invariants

8. `PoolRef` pointer is always valid — derived from an `Arc` that outlives any `PooledClient`
9. `Send + Sync` requirements on `PoolRef` are upheld by construction from `Arc::as_ptr`

---

## Error Taxonomy

### `LockError` Variants (lock operations)

| Variant | Trigger | Semantic Meaning |
|---------|---------|------------------|
| `AlreadyLocked { key }` | `pg_try_advisory_xact_lock` returns `false` | Key is held by another transaction |
| `NotLocked { key }` | Attempt to release a lock not held | N/A — not expected in current impl |
| `LockInvalidated { key }` | Lock state corrupted | N/A — not expected in current impl |
| `Connection(String)` | Thread spawn failure, pool exhaustion | Cannot communicate with pool |
| `Transaction { key, source }` | `BEGIN`, `ROLLBACK`, or `SELECT` fails | Database-level error during lock ops |

### `InitError` Variants (construction)

| Variant | Trigger | Semantic Meaning |
|---------|---------|------------------|
| `Connection(String)` | `PgClient::connect` fails or times out | DSN invalid or server unreachable |
| `Ping(String)` | Eager validation query fails | Connection is dead on arrival |
| `PoolConfig(String)` | Invalid pool configuration values | Negative size, invalid timeout, etc. |

---

## Contract Signatures

All fallible operations use `Result<T, Error>` with semantic error types.

```rust
// ── Locker construction ────────────────────────────────────────

impl PostgresLocker {
    /// Creates a PostgresLocker with default options.
    /// 
    /// # Errors
    /// Returns `Err(InitError)` if:
    /// - Connection to the database fails
    /// - Eager validation (SELECT 1) fails
    pub async fn new(dsn: &str) -> Result<Self, InitError>;

    /// Creates a PostgresLocker with explicit pool options.
    /// 
    /// # Errors
    /// Returns `Err(InitError)` if:
    /// - `opts` contains invalid configuration
    /// - Connection to the database fails
    /// - Eager validation (SELECT 1) fails — **GAP3 fix**
    pub async fn with_options(dsn: &str, opts: PostgresLockerOptions) -> Result<Self, InitError>;
}

// ── Lock acquisition ────────────────────────────────────────────

impl Locker for PostgresLocker {
    /// Acquires an advisory lock for `key`.
    /// 
    /// # Preconditions
    /// - Locker must be initialized
    /// - Pool must have capacity (open_count < max_open OR idle connections exist)
    /// 
    /// # Postconditions
    /// - On `Ok`: `PostgresLock` holds a `PgClient` in a valid transaction
    /// - Connection is NOT leaked — **GAP1 fix**: PooledClient Drop returns to pool on release/drop
    /// 
    /// # Errors
    /// Returns `Err(LockError)` if:
    /// - `AlreadyLocked` — key is held
    /// - `Connection` — pool exhausted or spawn failed
    /// - `Transaction` — BEGIN or lock query failed
    fn acquire_lock(&self, key: &str) -> AcquireLockFuture;
}

// ── Lock release ───────────────────────────────────────────────

impl Lock for PostgresLock {
    /// Releases the advisory lock and returns the connection to the pool.
    /// 
    /// # Preconditions
    /// - `PostgresLock` holds a `Some(PgClient)`
    /// 
    /// # Postconditions
    /// - `ROLLBACK` executed via `spawn_blocking` — **GAP2 fix**
    /// - `PgClient` returned to pool (via `PoolRef::put`)
    /// - `PostgresLock.client` is `None` after completion
    /// 
    /// # Errors
    /// Returns `Err(LockError::Connection)` if spawn fails (unlikely)
    fn release_lock(self: Pin<Box<Self>>) -> Pin<Box<dyn Future<Output = Result<(), LockError>> + Send>>;
}

// ── Pool operations (internal) ────────────────────────────────

impl SyncPostgresPool {
    /// Acquires a connection from the pool.
    /// 
    /// # Postconditions
    /// Returns `Ok(PooledClient)` where PooledClient Drop will call `pool.put()`
    fn get(&self) -> Result<PooledClient, LockError>;

    /// Returns a connection to the pool.
    /// 
    /// # Postconditions
    /// - If `max_idle` not exceeded: connection is in `idle` list
    /// - If `max_idle` exceeded: connection is closed, `open_count` decremented
    fn put(&self, client: PgClient, created_at: Instant);
}

impl Drop for PooledClient {
    /// Returns the client to the pool if still held.
    /// 
    /// # Invariant
    /// Client can only be returned once — `self.client.take()` ensures `None` on repeated drops.
}
```

---

## Non-goals

- Modifying the `InMemoryLocker` implementation
- Changing the public API surface of `Locker` or `Lock` traits
- Adding new `LockError` or `InitError` variants beyond what is already defined
- GAP4 (API signature mismatch) — considered acceptable divergence
- GAP6 (default pool values) — documented but not prioritized
- GAP7 (signed/unsigned hash) — verified as already correct
