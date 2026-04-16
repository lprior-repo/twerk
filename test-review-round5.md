# Suite Inquisition Report — Round 5

## VERDICT: REJECTED

### Tier 0 — Static Analysis

**[FAIL] Banned patterns**: 0 found in project tests/src  
**[FAIL] Holzmann rules**: `for .* in` loops found in coordinator_test.rs:44-51, 68-75, etc.  
**[PASS] Mock interrogation**: No mockall/mocks found  
**[PASS] Integration purity**: No `use crate::` in integration tests  
**[FAIL] Error variant completeness**: NOT VERIFIED (tests failing before analysis)  
**[FAIL] Density**: 41 integration tests in twerk-app / 96 pub fn in twerk-app = 0.43x (target ≥5x)

### Tier 1 — Compilation + Execution

**[FAIL] Clippy**: 4 errors in `crates/twerk-core/tests/red_queen_trigger_error.rs`:
- Line 72, 136, 713: `assert!(true)` — assertion is always true
- Line 679: unnecessary `map_err(|e| e)` 
- Line 687: `io::Error::new(io::ErrorKind::Other, ...)` should be `io::Error::other(...)`

**[FAIL] Tests**: 12 FAILED / 146 PASSED / 10 SKIPPED in twerk-app

Failed tests (all timing-based):
1. `coordinator_test::job_completes_when_tasks_are_finished` — timeout
2. `coordinator_test::first_top_level_task_is_scheduled_immediately_when_job_submitted` — timeout
3. `coordinator_test::parallel_tasks_scheduled_when_job_submitted` — timeout
4. `coordinator_test::each_tasks_scheduled_when_job_submitted` — timeout
5. `coordinator_test::subjob_scheduled_when_parent_job_running` — timeout
6. `coordinator_adversarial_test::start_job_returns_scheduled_state_when_broker_fails_to_publish_task` — timeout
7. `standalone_e2e_test::standalone_engine_marks_job_as_failed_when_task_fails` — timeout
8. `standalone_e2e_test::standalone_engine_retries_failed_task` — timeout
9. `standalone_e2e_test::standalone_engine_marks_parallel_job_as_failed_when_subtask_fails` — timeout
10. `standalone_e2e_test::standalone_engine_completes_job_naturally` — timeout
11. `standalone_e2e_test::standalone_engine_completes_each_job_naturally` — timeout
12. `standalone_e2e_test::standalone_engine_completes_parallel_job_naturally` — timeout

**[FAIL] Ordering probe**: Tests fail identically across thread counts — not an ordering issue  
**[N/A] Insta**: Not used in this workspace

### Tier 2 — Coverage

Not run due to Tier 1 failure.

### Tier 3 — Mutation

Not run due to Tier 1 failure.

---

## LETHAL FINDINGS

### 1. `coordinator_test.rs` — Virtual Time Testing Anti-pattern (LETHAL)

**Location**: `crates/twerk-app/tests/coordinator_test.rs:12, 86, 140, 211, 284`

**Issue**: Tests use `#[tokio::test(start_paused = true)]` with `tokio::time::advance()` but the coordinator's background task processing doesn't synchronize with the virtual time clock.

**Example** (lines 43-54):
```rust
let tasks = tokio::time::timeout(std::time::Duration::from_secs(5), async {
    loop {
        let tasks = datastore.get_active_tasks("test-job-2").await?;
        if !tasks.is_empty() {
            return Ok::<_, anyhow::Error>(tasks);
        }
        tokio::time::advance(std::time::Duration::from_millis(100)).await;
        tokio::task::yield_now().await;
    }
})
.await
.expect("timeout waiting for tasks")?;
```

**Root Cause**: `start_paused = true` creates a paused time clock. `tokio::time::advance()` only advances the clock in the current task. The coordinator spawns background tasks via `broker.subscribe_for_*` calls. These tasks use the paused clock and don't process messages because:
1. The coordinator's `handle_job_event` is invoked asynchronously
2. The handler uses `tokio::spawn` internally which doesn't inherit the virtual time context
3. The `start_paused` feature requires multi-threaded runtime to properly share time state

**Fix Required**: Either:
- Remove `start_paused = true` and use real timeouts (e.g., `tokio::time::sleep(Duration::from_secs(10))`)
- Or implement a test harness that properly drives the coordinator's event loop

### 2. `standalone_e2e_test.rs` — Same Virtual Time Issue (LETHAL)

**Location**: `crates/twerk-app/tests/standalone_e2e_test.rs:30-86, 88-153, 154-209, etc.`

**Issue**: Tests use `tokio::time::sleep(Duration::from_millis(100))` in polling loops without `start_paused`, but the engine's background processing doesn't complete within the 10-second timeout.

**Root Cause**: The engine's task processing happens in background tasks. When the test polls for `JobState::Failed`, the coordinator may not have processed the job event yet because:
- The broker's `publish_job` sends to handlers asynchronously
- Handlers update the datastore, but the test's poll interval may miss state transitions
- The 10-second timeout isn't sufficient for the full job lifecycle

### 3. Clippy Errors in Test Code (LETHAL)

**Location**: `crates/twerk-core/tests/red_queen_trigger_error.rs:72, 136, 679, 687, 713`

```rust
// Line 72, 136, 713 — assertion is always true
assert!(true, "All 11 TriggerError variants constructed successfully");

// Line 679 — unnecessary map_err
let result: Result<(), TriggerError> = err.map_err(|e| e);

// Line 687 — should use io::Error::other()
let io_err = io::Error::new(io::ErrorKind::Other, "specific error message");
```

**Fix**: Remove tautological assertions, fix `map_err(|e| e)` to just `err`, use `io::Error::other()`.

### 4. Test Density Violation (LETHAL)

**Metric**: 41 integration tests / 96 pub fn = 0.43x ratio

**Requirement**: ≥5x coverage means for 96 public functions, need ≥480 integration tests.

**Current Tests by Category**:
- `coordinator_test.rs`: 5 tests (all failing)
- `standalone_e2e_test.rs`: 6 tests (all failing)
- `engine_lifecycle_test.rs`: 41 tests (passing)
- `coordinator_adversarial_test.rs`: 3 tests (2 passing, 1 failing)
- `middleware_test.rs`: 20 tests (assumed passing)
- `benchmark_test.rs`: 4 tests (passing)
- `ci_cd_pipeline_simulation.rs`: 6 tests (passing)

**Gap Analysis**:
- `handlers.rs` (job event handling): No integration test coverage for `handle_cancel`, `handle_scheduled_job`
- `scheduler/` module: Only unit tests in `scheduler/tests.rs`
- `worker/` module: No integration tests for docker/podman runtime adapters
- `webhook.rs`: Only unit tests
- `limits.rs`: No integration tests for rate limiting under load

### 5. Holzmann Rule 2 Violation — Loops in Test Bodies (LETHAL)

**Location**: `crates/twerk-app/tests/coordinator_test.rs:44-51, 68-75, etc.`

**Issue**: Polling loops inside test bodies violate Holzmann Rule 2: "Loops are not test code."

**Example**:
```rust
loop {
    let tasks = datastore.get_active_tasks("test-job-2").await?;
    if !tasks.is_empty() {
        return Ok::<_, anyhow::Error>(tasks);
    }
    tokio::time::advance(std::time::Duration::from_millis(100)).await;
    tokio::task::yield_now().await;
}
```

**Fix**: Extract polling logic to a helper function or use `tokio::time::timeout` with a future that awaits specific state transitions via channels.

---

## MAJOR FINDINGS

### 1. Missing Error Variant Testing

**Location**: `coordinator_adversarial_test.rs` only tests 2 failure modes:
- `fail_create_job`
- `fail_publish_job`

**Not tested**:
- `fail_create_task` — no scenario
- `fail_update_job` — no scenario  
- `fail_publish_task` — partial (1 failing test but it times out)

### 2. No End-to-End Workflow Tests for Parallel/Each/Subjob

**Gap**: While unit tests exist for `schedule_parallel_task`, `schedule_each_task`, `schedule_subjob_task`, the full end-to-end integration (submit job → coordinator processes → tasks execute → job completes) is broken due to timing issues.

### 3. Test File Organization

The test suite is scattered across:
- `crates/twerk-app/tests/` — 15 files
- `crates/twerk-core/tests/` — 17 files
- `crates/twerk-web/tests/` — 12 files
- `crates/twerk-infrastructure/tests/` — 5 files

No clear hierarchy or naming convention distinguishing unit/integration/e2e tests.

---

## MINOR FINDINGS

1. `ci_cd_pipeline_simulation::sustained_load_60_seconds` takes >60s to run (marked as SLOW but passes)
2. Test file names: `twerk_yaml_workflow_benchmark.rs`, `realistic_profiling.rs`, `pokemon_api_benchmark.rs` are misnamed (no `#[test]` in them, just benchmark harnesses)
3. `mock.rs` in `scheduler/` directory contains test utilities, not mocks
4. No `#[ignore]` annotations on known-flaky tests

---

## MANDATE

The following MUST be fixed before re-submission:

### P0 — Critical (Any one is REJECTED)

1. **[FIX] coordinator_test.rs**: Remove `start_paused = true` and rewrite tests using real `tokio::time::sleep` with proper timeouts. The coordinator event processing must be driven synchronously in tests OR the tests must wait for actual state transitions via channels.

2. **[FIX] standalone_e2e_test.rs**: Rewrite polling loops to use proper async notification mechanisms (e.g., oneshot channel that the handler fires when state changes).

3. **[FIX] red_queen_trigger_error.rs**: Remove tautological `assert!(true)` assertions, fix `map_err(|e| e)` to just `err`, replace `io::Error::new(io::ErrorKind::Other, ...)` with `io::Error::other(...)`.

4. **[FIX] Holzmann loops**: Extract all `loop { ... tokio::time::advance ... }` polling constructs into a reusable `wait_for_state` helper that takes a future and timeout.

5. **[ADD] Coverage to 5x**: Need 480 integration tests for 96 public functions. Currently have ~41 in twerk-app alone. Either:
   - Write 439 more integration tests, OR
   - Refactor pub fn count down by extracting internal modules

### P1 — High Priority

6. **[ADD] Missing error variant tests**: Write scenarios for `fail_create_task`, `fail_update_job` in coordinator_adversarial_test.rs.

7. **[ADD] End-to-end parallel/each/subjob tests**: After fixing timing issues, add tests that verify full workflow: submit → coordinator schedules → subtasks execute → job completes.

8. **[REFACTOR] Test organization**: Establish clear naming: `*_unit_test.rs`, `*_integration_test.rs`, `*_e2e_test.rs`.

---

## REQUIRED TEST NAMES (For Mutation Survivors)

If mutation testing were run, survivors would indicate missing behavior. Based on code review:

| Missing Behavior | Required Test Name |
|------------------|-------------------|
| `handle_cancel` not exercised | `coordinator_handles_job_cancellation` |
| `handle_scheduled_job` not tested | `coordinator_processes_scheduled_job` |
| Rate limit middleware not tested under load | `rate_limiter_rejects_over_limit_requests` |
| Parallel task partial failure not tested | `parallel_job_fails_when_any_subtask_fails` |
| Each task expansion edge case | `each_task_handles_empty_list` |
| Webhook delivery retry not tested | `webhook_retries_on_delivery_failure` |

---

## Summary

**Test Suite Status**: BROKEN — 12/158 tests fail, all due to improper async/time testing patterns.

**Root Cause**: The `start_paused = true` pattern is incorrectly applied to tests where the code under test spawns background tasks that don't inherit the virtual time context.

**Recommendation**: Rewrite the timing-sensitive integration tests to use channel-based state notification instead of polling loops with `tokio::time::advance`.
