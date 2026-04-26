# Suite Inquisition Report — twerk-app (Round 3)

## Context
User requested: `cargo test --package twerk-app --lib 2>&1`
This runs only **lib unit tests** (tests within `src/`), NOT integration tests.

---

## VERDICT: REJECTED

### Tier 0 — Static
**[PASS]** Banned assertions — `assert!(result.is_ok())` / `assert!(result.is_err())` not found in lib tests
**[PASS]** `let _ =` silent discard — all hits in lib are appropriate (channel operations in spawned tasks, cleanup)
**[PASS]** `#[ignore]` — none found
**[PASS]** Sleep in lib tests — sleeps found are in `src/engine/worker/mod.rs` (heartbeat loop, stop function) which are production runtime concerns, not test logic
**[PASS]** Naming violations — `fn test_` patterns found in `src/engine/worker/podman.rs`, `src/engine/worker/mounter.rs` etc. are standard inline unit test modules (within `#[cfg(test)]`), not test file naming violations
**[PASS]** Loops in lib test bodies — loops found are in integration tests/benchmarks, not lib unit tests
**[PASS]** Shared mutable state — none found
**[PASS]** Mock interrogation — none found
**[PASS]** Integration purity — no `use crate::` in integration tests
**[PASS]** Error variant completeness — `TaskHandlerError`, `JobHandlerError`, `LogHandlerError`, `NodeHandlerError` each have 2 variants; tests cover the Handler and Datastore/Middleware variants

**[FAIL]** Density: 49 tests / 96 functions = 0.51x (target ≥5x)

---

### Tier 1 — Execution
**[PASS]** Clippy: 0 warnings
**[PASS]** `cargo test --package twerk-app --lib`: 49 passed, 0 failed
**[PASS]** Ordering probe: consistent across 1 and 8 threads
**[WARN]** Insta: timed out due to long-running integration tests (not a lib test issue)

---

### Tier 2 — Coverage
**[SKIP]** llvm-cov could not complete — triggered integration test suite which contains timing-sensitive tests that fail under coverage instrumentation

---

### Tier 3 — Mutation
**[SKIP]** Timed out after 180s — `ci_cd_pipeline_simulation::sustained_load_60_seconds` prevents mutation analysis

---

## LETHAL FINDINGS

### 1. Integration Test Suite is Broken (LETHAL)
**File:** `crates/twerk-app/tests/coordinator_test.rs` (lines 12-364)
**Finding:** 5 tests using `#[tokio::test(start_paused = true)]` with `tokio::time::advance()` all timeout:
- `job_completes_when_tasks_are_finished` — timeout at line 54
- `first_top_level_task_is_scheduled_immediately_when_job_submitted` — timeout at line 126
- `parallel_tasks_scheduled_when_job_submitted` — timeout at line 192
- `each_tasks_scheduled_when_job_submitted` — timeout at line 265
- `subjob_scheduled_when_parent_job_running` — timeout at line 331

**File:** `crates/twerk-app/tests/coordinator_adversarial_test.rs` (line 94)
**Finding:** `start_job_returns_scheduled_state_when_broker_fails_to_publish_task` — timeout waiting for state transition

**Impact:** These are flaky/invalid tests. The `start_paused = true` tokio runtime requires `tokio::time::advance()` to be called to progress time, but the coordinator's internal scheduling loop does not appear to be advancing time correctly, causing infinite loops.

---

### 2. Density Below Threshold (LETHAL)
**Finding:** 49 lib unit tests / 96 public functions = **0.51x ratio**
**Rule:** Ratio < 5x = LETHAL

The lib tests only cover ~0.5 public functions per test on average. Many pub fns are:
- Simple constructors (`new()`, `with_*()`)
- Trait method implementations
- One-liner delegation methods

But the density rule exists to catch **undertested complex logic**. The concern is whether the 49 tests adequately cover the complexity of the engine.

---

## MANDATE

### 1. Fix or Remove Broken Integration Tests
The 6 failing tests in `coordinator_test.rs` and `coordinator_adversarial_test.rs` must be fixed. They fail consistently, not just under load. The `#[tokio::test(start_paused = true)]` pattern combined with `tokio::time::advance()` is fundamentally incompatible with the current coordinator implementation's time handling.

**Required action:** Either:
- Remove `start_paused = true` and use real time (tests will be slower but correct), OR
- Fix the coordinator to properly yield to the runtime when `start_paused = true` is used

### 2. Achieve Density Target
49 tests is insufficient for 96 public functions at the 5x target (would need 480 tests).

**Required action:** Add tests for untested public functions, particularly:
- `Engine::new()`, `Engine::state()`, `Engine::mode()`, `Engine::engine_id()`
- `EngineProxy` methods
- `BrokerProxy` / `DatastoreProxy` clone and delegation methods
- Middleware registration methods
- Endpoint registration

### 3. Remove Mutation Testing Blocker
The `ci_cd_pipeline_simulation::sustained_load_60_seconds` test prevents mutation analysis from completing within reasonable time.

**Required action:** Mark the 60-second load test as `#[ignore]` or move it to a separate test binary that is not run during mutation analysis.

---

## SUMMARY

| Metric | Result | Threshold | Status |
|--------|--------|-----------|--------|
| Lib tests passing | 49/49 | 100% | PASS |
| Clippy warnings | 0 | 0 | PASS |
| Ordering consistency | Consistent | Consistent | PASS |
| Integration tests | 6/66 failing | 0 failing | FAIL |
| Test density | 0.51x | ≥5x | FAIL |
| Mutation kill rate | N/A (timeout) | ≥90% | SKIP |

**Next steps:** Fix the 6 broken integration tests, increase test density, then resubmit.
