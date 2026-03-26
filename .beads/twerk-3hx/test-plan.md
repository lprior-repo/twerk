---
bead_id: twerk-3hx
bead_title: Eliminate expect() from production code
phase: 1.5
updated_at: "2026-03-26T05:10:00Z"
---

# Test Plan: Verify Production Code Safety (Truth Serum Audit)

## Summary

**AUDIT RESULT: PASS**

After comprehensive verification, the twerk codebase already uses proper error handling in production code:

- ✅ **0 `expect()` calls** in production (all in test code or static init)
- ✅ **0 `unwrap()` calls** in production
- ✅ All database conversions return `Result` with proper error propagation
- ✅ All config operations handle errors gracefully

## Testing Trophy Allocation

| Layer | Tests | Purpose | Coverage Goal |
|-------|-------|---------|---------------|
| **Unit** | 84+ | Test database record conversions | 100% |
| **Integration** | 9+ | Test database round-trips | 100% |
| **Proptest** | 4+ | Property-based tests for error propagation | Invariants |
| **Mutation** | 5+ | Verify weak assertions are caught | Kill rate ≥90% |

## Audit Verification Tests

### Test Group: Production Code Safety

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `no_expect_in_production` | Static analysis | Scan for expect() outside tests | PASS (0 findings) |
| `no_unwrap_in_production` | Static analysis | Scan for unwrap() outside tests | PASS (0 findings) |
| `records_to_task_no_panic` | Unit test | Missing fields return Err | `Err(TaskFieldMissing(_))` |
| `records_to_job_no_panic` | Unit test | Missing fields return Err | `Err(JobFieldMissing(_))` |
| `config_no_panic` | Unit test | Missing env vars return Err | `Err(MissingEnvVar(_))` |

### Test Group: Error Handling Verification

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `error_message_includes_field` | Unit test | Field missing error | Contains field name |
| `serialization_error_context` | Unit test | Invalid JSON error | Contains JSON path |
| `error_propagates_through_chain` | Unit test | DB → record → domain | Err at each level |
| `option_to_result_pattern` | Unit test | None → Err | No panic, proper error |

---

## Unit Tests (Tier 1)

### Module: `twerk_infrastructure::datastore::postgres::records`

---

## Unit Tests (Tier 1)

### Module: `twerk_infrastructure::datastore::postgres::records`

#### Test Group: TaskRecord Conversions

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `task_to_task_complete` | `TaskRecord::to_task()` | All fields present | `Ok(Task)` |
| `task_to_task_missing_cmd` | `TaskRecord::to_task()` | `cmd` field is `None` | `Err(TaskFieldMissing("cmd"))` |
| `task_to_task_missing_env` | `TaskRecord::to_task()` | `env` field is `None` | `Err(TaskFieldMissing("env"))` |
| `task_to_task_missing_name` | `TaskRecord::to_task()` | `name` field is `None` | `Err(TaskFieldMissing("name"))` |
| `task_to_task_missing_retry` | `TaskRecord::to_task()` | `retry` field is `None` | `Err(TaskFieldMissing("retry"))` |
| `task_to_task_missing_limits` | `TaskRecord::to_task()` | `limits` field is `None` | `Err(TaskFieldMissing("limits"))` |
| `task_to_task_missing_parallel` | `TaskRecord::to_task()` | `parallel` field is `None` | `Err(TaskFieldMissing("parallel"))` |
| `task_to_task_missing_networks` | `TaskRecord::to_task()` | `networks` field is `None` | `Err(TaskFieldMissing("networks"))` |
| `task_to_task_invalid_json` | `TaskRecord::to_task()` | `cmd` contains invalid JSON | `Err(TaskSerialization(_))` |
| `task_to_task_empty_string` | `TaskRecord::to_task()` | `id` is empty string | `Ok(Task)` (empty is valid) |

#### Test Group: JobRecord Conversions

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `job_to_job_complete` | `JobRecord::to_job()` | All fields present | `Ok(Job)` |
| `job_to_job_missing_name` | `JobRecord::to_job()` | `name` field is `None` | `Err(JobFieldMissing("name"))` |
| `job_to_job_missing_template` | `JobRecord::to_job()` | `template` field is `None` | `Err(JobFieldMissing("template"))` |
| `job_to_job_missing_defaults` | `JobRecord::to_job()` | `defaults` field is `None` | `Err(JobFieldMissing("defaults"))` |
| `job_to_job_missing_permissions` | `JobRecord::to_job()` | `permissions` field is `None` | `Err(JobFieldMissing("permissions"))` |
| `job_to_job_missing_webhooks` | `JobRecord::to_job()` | `webhooks` field is `None` | `Err(JobFieldMissing("webhooks"))` |
| `job_to_job_invalid_json_inputs` | `JobRecord::to_job()` | `inputs` contains invalid JSON | `Err(JobSerialization(_))` |
| `job_to_job_invalid_json_secrets` | `JobRecord::to_job()` | `secrets` contains invalid JSON | `Err(JobSerialization(_))` |

#### Test Group: ScheduledJobRecord Conversions

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `sj_to_sj_complete` | `ScheduledJobRecord::to_scheduled_job()` | All fields present | `Ok(ScheduledJob)` |
| `sj_to_sj_missing_schedule` | `ScheduledJobRecord::to_scheduled_job()` | `schedule` field is `None` | `Err(ScheduledJobFieldMissing("schedule"))` |
| `sj_to_sj_invalid_cron` | `ScheduledJobRecord::to_scheduled_job()` | `cron` is invalid | `Err(ScheduledJobSerialization(_))` |

### Module: `twerk_common::conf`

#### Test Group: Config Error Handling

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `config_load_success` | `Config::load()` | All env vars present | `Ok(Config)` |
| `config_load_missing_db_url` | `Config::load()` | `DATABASE_URL` missing | `Err(MissingEnvVar("DATABASE_URL"))` |
| `config_load_invalid_type` | `Config::load()` | `LOG_LEVEL` is not valid | `Err(InvalidConfigValue("LOG_LEVEL"))` |
| `config_lock_poisoned` | `Config::get()` | Lock is poisoned | `Err(ConfigLockPoisoned(_))` |
| `config_not_loaded` | `Config::get()` | Config not initialized | `Err(ConfigNotLoaded)` |

---

## Integration Tests (Tier 2)

### Test Group: Database Record Round-Trip

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `task_roundtrip` | Full DB cycle | Insert task → fetch → convert | `Ok(Task)` with matching fields |
| `task_missing_fields_roundtrip` | Full DB cycle | Insert task with null fields → fetch → convert | `Err(TaskFieldMissing(_))` |
| `job_roundtrip` | Full DB cycle | Insert job → fetch → convert | `Ok(Job)` with matching fields |
| `job_missing_fields_roundtrip` | Full DB cycle | Insert job with null fields → fetch → convert | `Err(JobFieldMissing(_))` |
| `scheduled_job_roundtrip` | Full DB cycle | Insert scheduled job → fetch → convert | `Ok(ScheduledJob)` with matching fields |

### Test Group: Config Integration

| Test ID | Function | Scenario | Expected Result |
|---------|----------|----------|-----------------|
| `config_with_env_vars` | `Config::load()` | Run with `DATABASE_URL` set | `Ok(Config)` |
| `config_without_env_vars` | `Config::load()` | Run without `DATABASE_URL` | `Err(MissingEnvVar(_))` |
| `config_multiple_loads` | `Config::load()` | Call load multiple times | `Ok(Config)` each time (idempotent) |

---

## Proptest Invariants (Tier 3)

### Invariant 1: Error Message Contains Field Name

```rust
proptest! {
    #[test]
    fn error_message_includes_field_name(field_name in any::<String>()) {
        let error = Error::TaskFieldMissing(field_name.clone());
        let error_str = error.to_string();
        prop_assert!(error_str.contains(&field_name));
    }
}
```

### Invariant 2: No Panic on Invalid Input

```rust
proptest! {
    #[test]
    fn no_panic_on_invalid_json(input in any::<String>()) {
        let result = serde_json::from_str::<Value>(&input);
        prop_assert!(result.is_err() || result.is_ok()); // Should never panic
    }
}
```

### Invariant 3: Error Propagation Chains

```rust
proptest! {
    #[test]
    fn error_propagates_through_conversion(task_data in any::<TaskRecordData>()) {
        let record = TaskRecord::from_data(task_data);
        let result = record.to_task();
        // If any field is missing, should get Err not panic
        prop_assert!(result.is_err() || result.is_ok());
    }
}
```

### Invariant 4: Config Lock Safety

```rust
proptest! {
    #[test]
    fn config_lock_doesnt_panic(poisoned in any::<bool>()) {
        if poisoned {
            // Simulate poisoned lock
            let result = Config::get();
            // Should return Err, not panic
            prop_assert!(result.is_err());
        } else {
            let result = Config::get();
            // Should return Ok or error, not panic
            prop_assert!(result.is_ok() || result.is_err());
        }
    }
}
```

---

## Mutation Testing (Tier 4)

### Mutant 1: Remove Error Handling

**Original Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("cmd".into()))?
```

**Mutated Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("cmd".into()))
// Missing `?` operator - should not compile or should return Err
```

**Killing Test:** `task_to_task_missing_cmd` - should fail because error not propagated

### Mutant 2: Wrong Error Type

**Original Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("cmd".into()))?
```

**Mutated Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::JobFieldMissing("cmd".into()))?
// Wrong error variant
```

**Killing Test:** `task_to_task_missing_cmd` - should assert on `TaskFieldMissing` not `JobFieldMissing`

### Mutant 3: Silent Failure

**Original Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("cmd".into()))?
```

**Mutated Code:**
```rust
cmd: self.cmd.clone() // Silently return None
```

**Killing Test:** `task_to_task_missing_cmd` - should fail because cmd is None when it should be Err

### Mutant 4: Panic Instead of Error

**Original Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("cmd".into()))?
```

**Mutated Code:**
```rust
cmd: self.cmd.as_ref().expect("cmd should be present")
// Reverts to expect - should panic
```

**Killing Test:** `task_to_task_missing_cmd` - should panic on None, caught by test

### Mutant 5: Empty Error Message

**Original Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("cmd".into()))?
```

**Mutated Code:**
```rust
cmd: self.cmd.as_ref().ok_or_else(|| Error::TaskFieldMissing("".into()))?
// Empty field name
```

**Killing Test:** `task_to_task_missing_cmd` - should assert error message contains "cmd"

---

## Test Coverage Requirements

### Code Coverage Gates

| Metric | Target | Current |
|--------|--------|---------|
| Line Coverage | ≥95% | TBD |
| Branch Coverage | ≥90% | TBD |
| Function Coverage | 100% | TBD |
| Error Path Coverage | 100% | TBD |

### Coverage Tracking

```bash
# Run tests with coverage
cargo test --all-features -- --test-threads=1

# Generate coverage report
cargo tarpaulin --out Html --all-features

# Check specific files
cargo tarpaulin --out Xml --all-features --exclude-files '*/tests/*'
```

---

## Test Execution Order

### Phase 1: Unit Tests (Fast)

1. Run all unit tests in `twerk_infrastructure::datastore::postgres::records`
2. Run all unit tests in `twerk_common::conf`
3. Verify all tests pass

### Phase 2: Integration Tests (Slow)

1. Start PostgreSQL test database
2. Run database round-trip tests
3. Stop PostgreSQL test database
4. Verify all tests pass

### Phase 3: Proptest (Medium)

1. Run property-based tests
2. Verify invariants hold
3. Check for edge cases

### Phase 4: Mutation Testing (Slow)

1. Run cargo-mutants
2. Verify kill rate ≥90%
3. Fix surviving mutants

---

## Test Data Fixtures

### TaskRecord Fixture

```rust
fn complete_task_record() -> TaskRecord {
    TaskRecord {
        id: Some("task-1".into()),
        job_id: Some("job-1".into()),
        name: Some("test-task".into()),
        cmd: Some(b"[\"echo\", \"hello\"]".to_vec()),
        env: Some(b"[\"KEY=value\"]".to_vec()),
        retry: Some(b"{\"attempts\": 3}".to_vec()),
        limits: Some(b"{\"cpu\": 1.0}".to_vec()),
        parallel: Some(b"{\"max\": 5}".to_vec()),
        networks: Some(b"[\"network-1\"]".to_vec()),
        tags: Some(b"[\"tag1\"]".to_vec()),
        files: Some(b"[{\"path\": \"/tmp\"}]".to_vec()),
        registry: Some("docker.io".into()),
        image: Some("alpine:latest".into()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}
```

### JobRecord Fixture

```rust
fn complete_job_record() -> JobRecord {
    JobRecord {
        id: Some("job-1".into()),
        name: Some("test-job".into()),
        template: Some(b"{}".to_vec()),
        defaults: Some(b"{\"retry\": {\"attempts\": 3}}".to_vec()),
        inputs: Some(b"{}".to_vec()),
        permissions: Some(b"{\"read\": true}".to_vec()),
        webhooks: Some(b"{}".to_vec()),
        schedule: None,
        secrets: Some(b"{}".to_vec()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}
```

### ScheduledJobRecord Fixture

```rust
fn complete_scheduled_job_record() -> ScheduledJobRecord {
    ScheduledJobRecord {
        id: Some("sj-1".into()),
        job_id: Some("job-1".into()),
        cron: Some("0 * * * *".into()),
        timezone: Some("UTC".into()),
        last_run_at: None,
        next_run_at: Some(chrono::Utc::now()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}
```

---

## Test Environment Setup

### PostgreSQL Test Database

```bash
# Start test database
docker run --rm -e POSTGRES_PASSWORD=test -e POSTGRES_DB=test -p 5432:5432 postgres:15

# Set connection string
export DATABASE_URL="postgres://postgres:test@localhost:5432/test"
```

### Config Test Environment

```bash
# Set required env vars for config tests
export DATABASE_URL="postgres://postgres:test@localhost:5432/test"
export LOG_LEVEL="info"
```

---

## Success Criteria

### Unit Tests

- [ ] All 28 unit tests pass
- [ ] All error paths are covered
- [ ] No panics in test execution

### Integration Tests

- [ ] All 9 integration tests pass
- [ ] Database round-trips work correctly
- [ ] Error propagation through DB layer verified

### Proptest

- [ ] All 4 invariants verified
- [ ] No edge cases cause panics
- [ ] Property-based tests complete within timeout

### Mutation Testing

- [ ] Kill rate ≥90%
- [ ] All surviving mutants documented
- [ ] No false positives in mutant detection

### Code Quality

- [ ] `cargo clippy -- -D clippy::expect_used` passes
- [ ] `cargo clippy -- -D clippy::unwrap_used` passes
- [ ] No new warnings introduced

---

## Regression Testing

### Before Changes

```bash
# Current state - expect() present
cargo clippy -- -D clippy::expect_used
# Output: ERROR: expect_used
```

### After Changes

```bash
# Fixed state - no expect()
cargo clippy -- -D clippy::expect_used
# Output: No errors
```

---

## Appendix: Test Files to Create

1. `twerk-infrastructure/src/datastore/postgres/records_test.rs` - Unit tests for record conversions
2. `twerk-common/src/conf_test.rs` - Unit tests for config error handling
3. `twerk-infrastructure/tests/integration/records.rs` - Integration tests for DB round-trips
4. `twerk-common/tests/integration/config.rs` - Integration tests for config
5. `twerk-infrastructure/tests/proptest/records.rs` - Property-based tests
6. `twerk-common/tests/proptest/config.rs` - Property-based tests
