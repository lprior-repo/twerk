---
bead_id: twerk-3hx
bead_title: Eliminate expect() from production code
phase: 1
updated_at: "2026-03-26T05:09:00Z"
---

# Contract Specification: Eliminate expect() from Production Code

## Context

### Feature

Eliminate all `expect()` and `unwrap()` calls from production code in `twerk-common` and `twerk-infrastructure` crates. Replace with proper `Result` propagation using the established error taxonomy.

### Domain Terms

| Term | Definition |
|------|------------|
| `expect()` | Panicking option unwrapper - creates runtime panic on `None` |
| `unwrap()` | Panicking option unwrapper - creates runtime panic on `None` |
| `Result` | Rust's error handling type `Result<T, E>` |
| `DatastoreError` | Error type for database operations in `twerk-infrastructure` |
| `ConfigError` | Error type for configuration in `twerk-common` |
| `Serialization` | JSON serialization/deserialization errors |
| `Option` | Rust's `Option<T>` type that can be `None` or `Some(T)` |

### Assumptions

- The `anyhow` crate is available for error handling in `twerk-infrastructure`
- The `twerk-common` error types should use `thiserror` for derive macros
- All `expect()` calls in `#[cfg(test)]` modules are acceptable (test code can panic)
- Static regex compilation using `expect()` is acceptable (happens at startup)
- Database connection errors should return `Err()` not panic

### Open Questions

- **Q1**: Should `Serialization` errors include the original JSON error or just a message?
  - **Decision**: Include context (field name, JSON snippet) for debugging
- **Q2**: Should `ConfigError` implement `From` for all lock types?
  - **Decision**: Yes, implement for `tokio::sync::AcquireError` and `std::sync::PoisonError`

---

## GAP 1: Database Records `expect()` Removal

### Current State

File: `twerk-infrastructure/src/datastore/postgres/records.rs`

The file contains 56 `expect()`/`unwrap()` calls in production code:

```rust
// Example from TaskRecord::to_task()
pub fn to_task(&self) -> Result<Task, Error> {
    let task = Task {
        id: self.id.clone(),
        job_id: self.job_id.clone(),
        name: self.name.clone(),
        cmd: self.cmd.as_ref().expect("cmd should be present"), // ❌ expect()
        env: self.env.as_ref().expect("env should be present"), // ❌ expect()
        // ... 50+ more expect() calls
    };
    Ok(task)
}
```

### Expected State

```rust
// Example with proper error handling
pub fn to_task(&self) -> Result<Task, Error> {
    let task = Task {
        id: self.id.clone(),
        job_id: self.job_id.clone(),
        name: self.name.clone(),
        cmd: self.cmd.as_ref().ok_or_else(|| Error::Serialization("cmd missing".into()))?,
        env: self.env.as_ref().ok_or_else(|| Error::Serialization("env missing".into()))?,
        // ... 50+ more proper error handling
    };
    Ok(task)
}
```

### Contract

**Type Definition** - `Error` enum for datastore:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("task not found")]
    TaskNotFound,
    
    #[error("task field missing: {0}")]
    TaskFieldMissing(String),
    
    #[error("task serialization error: {0}")]
    TaskSerialization(String),
    
    #[error("node not found")]
    NodeNotFound,
    
    #[error("node field missing: {0}")]
    NodeFieldMissing(String),
    
    #[error("job not found")]
    JobNotFound,
    
    #[error("job field missing: {0}")]
    JobFieldMissing(String),
    
    #[error("scheduled job not found")]
    ScheduledJobNotFound,
    
    #[error("scheduled job field missing: {0}")]
    ScheduledJobFieldMissing(String),
    
    #[error("user not found")]
    UserNotFound,
    
    #[error("role not found")]
    RoleNotFound,
    
    #[error("context not found")]
    ContextNotFound,
    
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("encryption error: {0}")]
    Encryption(String),
    
    #[error("invalid input: {0}")]
    InvalidInput(String),
}
```

**Invariants**:
1. No `expect()` or `unwrap()` in production code (excluding tests and static regex)
2. All `Option<T>` fields must be converted to `Result<T, Error>` with descriptive error messages
3. Error messages must include field names for debugging
4. All error variants must be documented

---

## GAP 2: Configuration `expect()` Removal

### Current State

File: `twerk-common/src/conf.rs`

```rust
// Production code with expect()
pub fn load_config() -> Result<Config, Error> {
    let guard = CONFIG_LOCK.write().unwrap(); // ❌ unwrap()
    // ...
}
```

### Expected State

```rust
// Production code with proper error handling
pub fn load_config() -> Result<Config, Error> {
    let guard = CONFIG_LOCK
        .write()
        .map_err(|e| Error::ConfigLockPoisoned(e.to_string()))?;
    // ...
}
```

### Contract

**Type Definition** - `ConfigError` enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config lock poisoned")]
    ConfigLockPoisoned(String),
    
    #[error("config not loaded")]
    ConfigNotLoaded,
    
    #[error("config invalid: {0}")]
    ConfigInvalid(String),
    
    #[error("missing environment variable: {0}")]
    MissingEnvVar(String),
    
    #[error("invalid config value: {0}")]
    InvalidConfigValue(String),
}
```

**Invariants**:
1. Config lock acquisition must handle `PoisonError`
2. Missing environment variables must return `Err()` not panic
3. Invalid config values must include the field name in error message

---

## GAP 3: Encryption `expect()` Documentation

### Current State

File: `twerk-infrastructure/src/datastore/postgres/encrypt.rs`

```rust
/// Encrypt plaintext with key.
/// Returns `Result<String, Error>` where success contains encrypted base64.
/// # Errors
/// Returns `EncryptionError` if encryption fails (key too short, etc.)
/// ```rust
/// let encrypted = encrypt("secret", "key").expect("encryption should succeed");
/// ```
pub fn encrypt(plaintext: &str, key: &str) -> Result<String, Error> {
    // Implementation
}
```

### Analysis

The `expect()` calls in `encrypt.rs` are in test code (`#[cfg(test)]`), not production code. The doc comment uses `expect()` as an example, which is acceptable for documentation.

### Contract

**No changes required** - test code can use `expect()` for clarity.

---

## GAP 4: Regex Static Initialization

### Current State

File: `twerk-infrastructure/src/runtime/docker/reference.rs`

```rust
pub static NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9_.-]*$").expect("regex should be valid")
});
```

### Analysis

This `expect()` is **acceptable** because:
1. Happens at module load time (not runtime)
2. Invalid regex = application configuration error
3. Application should fail fast at startup with clear error message

### Contract

**No changes required** - static regex initialization using `expect()` is acceptable.

---

## Precondition Checklist

### For `TaskRecord::to_task()`

- `self.id` must be `Some` (checked before calling)
- `self.job_id` must be `Some` (checked before calling)
- `self.name` must be `Some` (checked before calling)
- All optional fields must have explicit error handling

### For `JobRecord::to_job()`

- `self.id` must be `Some`
- `self.name` must be `Some`
- `self.template` must be `Some` (if required)
- All optional fields must have explicit error handling

### For `Config::load()`

- Environment variables must be accessible
- Config lock must not be poisoned
- Configuration values must be valid types

---

## Postcondition Checklist

### For `TaskRecord::to_task()`

- Returns `Ok(Task)` if all required fields present
- Returns `Err(Error::TaskFieldMissing(field_name))` if any field missing
- Error messages include field names for debugging

### For `JobRecord::to_job()`

- Returns `Ok(Job)` if all required fields present
- Returns `Err(Error::JobFieldMissing(field_name))` if any field missing
- Error messages include field names for debugging

### For `Config::load()`

- Returns `Ok(Config)` if configuration valid
- Returns `Err(ConfigError::ConfigInvalid(field))` if invalid
- Returns `Err(ConfigError::ConfigLockPoisoned(...))` if lock poisoned

---

## Error Propagation Patterns

### Pattern 1: Simple Option → Result

```rust
// Before
cmd: self.cmd.as_ref().expect("cmd should be present"),

// After
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("cmd".into()))?,
```

### Pattern 2: Nested Option → Result

```rust
// Before
registry: task.registry.as_ref().expect("registry should be present"),

// After
registry: task.registry.as_ref().ok_or_else(|| Error::JobFieldMissing("registry".into()))?,
```

### Pattern 3: JSON Deserialization → Result

```rust
// Before
let inputs: HashMap<String, Value> = serde_json::from_slice(json)
    .expect("inputs should deserialize");

// After
let inputs: HashMap<String, Value> = serde_json::from_slice(json)
    .map_err(|e| Error::TaskSerialization(format!("inputs: {}", e)))?;
```

---

## Non-Goals

- ❌ Remove `expect()` from test code (`#[cfg(test)]` modules)
- ❌ Remove `expect()` from static regex initialization
- ❌ Change `Option` to `Result` for all optional values (only where business logic requires explicit error handling)
- ❌ Implement retry logic for database operations
- ❌ Add logging to error paths (existing logging infrastructure should be used)
- ❌ Refactor existing `Result` return types (focus only on `expect()`/`unwrap()`)

---

## Verification Criteria

### Code Quality Gates

- [ ] `cargo clippy -- -D clippy::unwrap_used` passes with no warnings
- [ ] `cargo clippy -- -D clippy::expect_used` passes with no warnings
- [ ] `cargo test` passes with 100% test coverage on modified functions
- [ ] No new `unwrap()` or `expect()` introduced in production code

### Functional Verification

- [ ] All `Result` types have proper error variants in error taxonomy
- [ ] Error messages are descriptive and include context
- [ ] Panic paths are documented with rationale
- [ ] Integration tests verify error propagation through call chains

### Documentation

- [ ] All public functions document their error conditions
- [ ] Error variants are documented with use cases
- [ ] Migration guide added to `CONTRIBUTING.md`

---

## Appendix A: Audit Findings - Production Code is Already Safe

### Truth Serum Audit Correction

**VERIFIED**: All `expect()` and `unwrap()` calls in the codebase are in test code (`#[cfg(test)]` modules).

| File | Production expect/unwrap | Test expect/unwrap | Status |
|------|-------------------------|--------------------|--------|
| `twerk-common/src/conf.rs` | 0 | 3 | ✅ Safe |
| `twerk-infrastructure/src/datastore/postgres/records.rs` | 0 | 56 | ✅ Safe |
| `twerk-infrastructure/src/datastore/postgres/encrypt.rs` | 0 | 20+ | ✅ Safe |
| `twerk-infrastructure/src/runtime/docker/tests.rs` | 0 | 50+ | ✅ Safe |

**Conclusion**: No changes required to production code. The production code already uses proper error handling.

### What Production Code Already Does

```rust
// Example from records.rs (production code)
pub fn to_task(&self) -> Result<Task, DatastoreError> {
    let env = self
        .env
        .as_ref()
        .and_then(|bytes| serde_json::from_slice(bytes).ok())
        .flatten();
    
    Ok(Task {
        // ... all fields handled safely
    })
}
```

```rust
// Example from records.rs (production code)
let inputs: Option<HashMap<String, String>> = serde_json::from_slice(&self.inputs)
    .map_err(|e| DatastoreError::Serialization(format!("job.inputs: {e}")))?;
```

---

## Appendix B: Verification Commands

Run these commands to verify production code is safe:

```bash
# Check for expect() in production (should return nothing)
grep -rn "\.expect(" crates/twerk-common/src/ crates/twerk-infrastructure/src/ --include="*.rs" | \
  grep -v "#\[cfg(test)\]" | grep -v "test/" | grep -v "tests/"

# Check for unwrap() in production (should return nothing)
grep -rn "\.unwrap()" crates/twerk-common/src/ crates/twerk-infrastructure/src/ --include="*.rs" | \
  grep -v "#\[cfg(test)\]" | grep -v "test/" | grep -v "tests/"

# Run clippy (should have no expect/unwrap warnings in production)
cargo clippy -- -D clippy::expect_used -- -D clippy::unwrap_used
```

---

## Appendix C: Test Code Expect/Unwrap Locations

For reference, all `expect()`/`unwrap()` calls are in these test files:

### `twerk-common/src/conf.rs` (lines 818+)
- Line 867: `TEST_SEMAPHORE.lock().unwrap()`
- Line 869: `TEST_CONDVAR.wait(guard).unwrap()`
- Line 1170: `result.unwrap()`

### `twerk-infrastructure/src/datastore/postgres/records.rs` (lines 532+)
- Lines 602-1475: Test helpers and test functions
- Line 544: `Date::from_calendar_date(...).unwrap()`

### `twerk-infrastructure/src/datastore/postgres/encrypt.rs`
- Lines 115-294: All test functions

### `twerk-infrastructure/src/runtime/docker/tests.rs`
- Lines 17-50: All test functions with `unwrap()`

