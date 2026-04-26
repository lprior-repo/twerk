# Suite Inquisition Report: twerk-infrastructure

**Package:** `twerk-infrastructure`
**Review Mode:** Mode 2 — Suite Inquisition (implementation exists, tests written)
**Date:** 2026-04-16

---

## VERDICT: REJECTED

### Tier 0 — Static

**[FAIL] Banned patterns** — Sleep in tests found:
- `broker/inmemory/tests.rs:58` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:94` — `tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;`
- `broker/inmemory/tests.rs:125` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:165` — `tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;`
- `broker/inmemory/tests.rs:191` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:219` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:255` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:364` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:416` — `tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;`
- `broker/inmemory/tests.rs:470` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:514` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- `broker/inmemory/tests.rs:567` — `tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;`
- **13 total sleep calls** in broker/inmemory/tests.rs

**[FAIL] Holzmann rules** — Loops in test bodies:
- `broker/inmemory/tests.rs:282` — `for _ in 0..10`
- `broker/inmemory/tests.rs:405` — `for i in 0..10`
- `broker/inmemory/tests.rs:527` — `for i in 0..5`

**[PASS] Mock interrogation** — No mockall/Mock patterns found

**[PASS] Integration purity** — No `use crate::` in /tests/ integration tests

**[PASS] Error variant completeness** — No broker Error enum (uses anyhow::Result)

**[FAIL] Density: 483 tests / 140 functions = 3.45x (target ≥5x)** — LETHAL

---

### Tier 1 — Execution

**[PASS] Clippy** — 0 warnings with `-D warnings`

**[FAIL] nextest** — 1 failed (test_postgres_all), 379 passed, 11 skipped
- `test_postgres_all` fails consistently at `postgres_test.rs:437` with `InvalidId("invalid UUID format: expected RFC 4122 compliant UUID")`
- Note: This is NOT in the broker/inmemory test suite

**[PASS] Ordering probe** — consistent between --test-threads=1 and --test-threads=8

**[PASS] Insta** — not present in project

---

### Tier 2 — Coverage

**[SKIP]** llvm-cov not available

---

### Tier 3 — Mutation

**[SKIP]** cargo-mutants not available

---

## LETHAL FINDINGS

### 1. Sleep in test bodies (Holzmann Rule 2)
**File:** `crates/twerk-infrastructure/src/broker/inmemory/tests.rs`
**Lines:** 58, 94, 125, 165, 191, 219, 255, 364, 416, 470, 514, 567

Tests use `tokio::time::sleep()` to wait for async events. This is an anti-pattern:
- Non-deterministic: timing-dependent tests fail under load
- Hides race conditions: events that should be immediate may have real timing issues
- Slows test suite: unnecessary waiting

**Required fix:** Replace sleep-based polling with proper event notification mechanisms (e.g., `broadcast::Receiver`, `watch` channel, or `futures::AsyncReadExt` with timeout).

### 2. Loops in test bodies (Holzmann Rule 2)
**File:** `crates/twerk-infrastructure/src/broker/inmemory/tests.rs`
**Lines:** 282, 405, 527

```rust
// Line 282
for _ in 0..10 {
    let handler: super::super::TaskHandler = Arc::new(|_| Box::pin(async { Ok(()) }));
    ...
}

// Line 405
for i in 0..10 {
    let job = twerk_core::job::Job { ... };
    ...
}

// Line 527
for i in 0..5 {
    let task = Task { ... };
    ...
}
```

Loops in tests violate Holzmann Rule 2. Tests should be deterministic and independent.

**Required fix:** Unroll loops into explicit test cases, or use parameterized testing (rstest).

### 3. Test density below threshold
**Calculated:** 483 tests / 140 pub fn = 3.45x
**Required:** ≥5x

**Required fix:** Write 217 more tests to reach 5x density, OR refactor to reduce public function count.

---

## MANDATE

Before resubmission, the following must be completed:

1. **Remove all sleep calls** from `broker/inmemory/tests.rs` — Replace with proper async event notification
2. **Unroll all loops** in `broker/inmemory/tests.rs` — Convert `for` loops to explicit test cases
3. **Increase test density** — Add 217+ tests OR reduce public API surface
4. **Fix `test_postgres_all`** — Resolve UUID format error at `postgres_test.rs:437` (separate issue but blocking CI)

**Survivors requiring named tests:**
- `InMemoryBroker::publish_heartbeat` — needs test for duplicate heartbeats (same node ID overwrites)
- `InMemoryBroker::publish_task_log_part` — needs test for max storage bounds
- `InMemoryBroker::subscribe_for_events` — needs test for wildcard pattern like `job.*` NOT matching `jobcompleted`

---

## SPECIFIC BROKER INMEMORY TEST ISSUES

| Test Name | Issue | Line |
|-----------|-------|------|
| `test_publish_heartbeat_stores_and_notifies` | sleep(50) | 58 |
| `test_subscribe_for_heartbeats_sends_existing` | sleep(200) | 94 |
| `test_publish_task_log_part_stores_and_notifies` | sleep(50) | 125 |
| `test_subscribe_for_task_log_part_sends_existing` | sleep(200) | 165 |
| `test_heartbeat_without_id_does_not_store` | sleep(50) | 191 |
| `test_task_log_part_without_task_id_does_not_store` | sleep(50) | 219 |
| `test_publish_and_subscribe_for_task` | sleep(50) | 255 |
| `test_get_queues` | for loop | 282 |
| `test_publish_and_subscribe_for_job` | sleep(50) | 364 |
| `test_multiple_subscribers_for_job` | for loop + sleep(100) | 405, 416 |
| `test_subscribe_for_events` | sleep(50) | 470 |
| `test_publish_and_subscribe_for_task_progress` | sleep(50) | 514 |
| `test_queue_info` | for loop | 527 |
| `broker_publish_heartbeat_receives_handler` | sleep(50) | 567 |

---

**STATUS: REJECTED** — Three LETHAL findings in Tier 0 require immediate correction.
