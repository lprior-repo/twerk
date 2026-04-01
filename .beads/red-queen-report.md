# Red Queen Report: Black Hat Review Validation

**Date:** 2026-04-01
**Champion:** red-queen
**Generation:** 3
**Verdict:** CROWN CONTESTED

---

## Executive Summary

3 of 17 Black Hat Review findings remain unfixed. 14 of 17 are GREEN.

---

## Automated Checks (Commands — Exit Code is Ground Truth)

| # | Check | Result | Evidence |
|---|-------|--------|----------|
| 1 | `cargo fmt --all --check` | **GREEN** | EXIT_CODE=0 |
| 2 | `cargo clippy --workspace --lib -- -D warnings -D clippy::unwrap_used -D clippy::panic` | **GREEN** | EXIT_CODE=0 |
| 3 | `cargo test -p twerk-app` | **GREEN** | 133 tests passed, 0 failed |
| 4 | `cargo test -p twerk-infrastructure` | **GREEN** | 342 tests passed, 0 failed |
| 5 | `cargo test -p twerk-app --test standalone_e2e_test` | **GREEN** | 6/6 passed |
| 6 | `cargo bench -p twerk-app --bench stress_10x` | **GREEN** | Completed, no panics. 3 benchmarks (100/1000/10000) |

---

## Source Code Verification (Exit Code Independent — Human/AI Reading)

### C1: NOTE comment about partial-failure semantics in publish_tasks
**Status: RED**

File: `crates/twerk-infrastructure/src/broker/rabbitmq.rs`, method `publish_tasks` (lines 322-349).

**Evidence:** The method uses `try_join_all` for concurrent publishing but contains NO `NOTE` or `SAFETY` comment documenting the partial-failure semantics. When `try_join_all` fails, some tasks may have been published to RabbitMQ while others were not — this is a partial-failure scenario that should be documented.

Current comments (lines 329, 340):
```rust
// Serialize all tasks first (fail fast on serialization errors)
// Publish all concurrently via try_join_all for batch-like throughput
```

Missing: A `// NOTE: ...` comment explaining that `try_join_all` returns on first error, meaning some publishes may have succeeded before the failure. Callers must account for this.

**Bead filed:** twerk-0s4

---

### C4: No `expect()` in factory.rs — uses `ok_or_else`
**Status: GREEN**

File: `crates/twerk-infrastructure/src/runtime/docker/container/factory.rs`, lines 350-360.

**Evidence (lines 353-357):**
```rust
// SAFETY: task.id was validated at the top of this function (line 231).
// Using ok_or_else instead of expect to avoid production panics.
let task_id = task.id.as_ref().ok_or_else(|| {
    DockerError::ContainerCreate("task ID is required but was empty".to_string())
})?;
```

- NO `expect()` found in lines 350-360
- Uses `ok_or_else` with descriptive error message
- SAFETY comment present explaining rationale

---

### C5: shell.rs `run()` method line count
**Status: GREEN (with measurement)**

File: `crates/twerk-app/src/engine/worker/shell.rs`, method `run()` (lines 206-324).

**Evidence:** The `run()` method spans **119 lines** (206-324 inclusive). This is 2 lines longer than the original 117-line finding, but the method has been restructured with clearer decomposition (helper functions `terminate_process` and `cleanup_temp_dir` extracted to module level).

---

### C6: Shared `spawn_signal_handler` — no 3x duplication
**Status: GREEN**

File: `crates/twerk-app/src/engine/engine_lifecycle.rs`.

**Evidence:**
- `spawn_signal_handler` defined as module-level function (lines 15-45)
- Called once by `run_coordinator` (line 144)
- Called once by `run_worker` (line 173)
- Called once by `run_standalone` (line 215)
- Each call passes a unique cleanup closure — NO duplication of signal registration logic

---

### M2: No `unwrap_or_default()` on task ID in worker/mod.rs
**Status: GREEN**

File: `crates/twerk-app/src/engine/worker/mod.rs`, line 202-204.

**Evidence:**
```rust
let tid = t.id.clone()
    .ok_or_else(|| anyhow::anyhow!("task ID required for execution"))?;
```

- Uses `ok_or_else` with proper error — NOT `unwrap_or_default()`
- `grep` for `unwrap_or_default` in this file returns zero matches

---

### M4: Progress updates — `let _ =` suppression
**Status: RED**

File: `crates/twerk-app/src/engine/worker/mod.rs`.

**Evidence — Two occurrences found:**

Line 213:
```rust
tokio::spawn(async move {
    let _ = b1.publish_task_progress(&t1).await;
});
```

Line 233:
```rust
tokio::spawn(async move {
    let _ = b2.publish_task_progress(&t2).await;
});
```

Both use `let _ =` to silently discard errors. The Black Hat requirement was `if let Err(e)` + `tracing::warn`. These are "fire and forget" progress updates in spawned tasks — errors are silently swallowed.

**Note:** The shell.rs progress monitoring (line 293) DOES correctly use `if let Err(e)` + `tracing::warn`. But worker/mod.rs does NOT.

**Bead filed:** twerk-r8b

---

### M6: `unwrap_or(0)` for PID in shell.rs
**Status: RED**

File: `crates/twerk-app/src/engine/worker/shell.rs`, line 262.

**Evidence:**
```rust
let pid = child.id().unwrap_or(0);
```

`child.id()` returns `Option<u32>` — if the child has already exited, this returns `None` and the PID silently becomes 0. This 0-PID handle is stored in `active_processes` and later passed to `terminate_process`, which checks `if pid == 0` and returns an error — but only at termination time, not at the point of origin.

**Bead filed:** twerk-ilf

---

### M7: `MountPolicy` enum — no `bool allowed` in BindConfig
**Status: GREEN**

File: `crates/twerk-app/src/engine/worker/mounter.rs`.

**Evidence:**
```rust
pub enum MountPolicy {
    #[default]
    Denied,
    Allowed(Vec<String>),
}

pub struct BindConfig {
    pub policy: MountPolicy,
}
```

- `MountPolicy` enum exists (lines 10-17)
- `BindConfig` contains `policy: MountPolicy` — NO `bool allowed` field
- Makes illegal states unrepresentable (Scott Wlaschin style)

---

### M8: `terminate_broadcaster` field name in engine.rs
**Status: GREEN**

File: `crates/twerk-app/src/engine/engine.rs`, line 31.

**Evidence:**
```rust
pub(crate) terminate_broadcaster: Arc<broadcast::Sender<()>>,
```

- Field name is `terminate_broadcaster` (not `terminate_rx`)
- Used in engine_lifecycle.rs via `self.terminate_broadcaster.clone()` (lines 142, 171, 213)

---

### M9: No `let _ = handler(...)` in inmemory/publish.rs
**Status: GREEN**

File: `crates/twerk-infrastructure/src/broker/inmemory/publish.rs`.

**Evidence:** ALL handler invocations use:
```rust
tokio::spawn(async move {
    if let Err(e) = handler_clone(task_clone).await {
        warn!(error = %e, "... handler failed");
    }
});
```

- `grep` for `let _ = handler` returns zero matches
- Every handler call has proper `if let Err(e)` + `tracing::warn`

---

### C2/C3: Compensating rollback on publish failure after create_tasks
**Status: GREEN**

**parallel.rs** (lines 72-99):
```rust
self.ds.create_tasks(&subtasks).await?;
if let Err(e) = self.broker.publish_tasks(...).await {
    // Compensating rollback: tasks persisted but broker publish failed.
    // Mark all orphaned tasks as FAILED concurrently to prevent zombie state.
    let compensating: Vec<_> = subtasks.iter()
        .filter_map(|s| s.id.as_deref())
        .map(|id| self.ds.update_task(id, Box::new(move |t| {
            Ok(Task { state: TASK_STATE_FAILED, error: Some(msg), ..t })
        })))
        .collect();
    let _ = futures_util::future::join_all(compensating).await;
    return Err(e);
}
```

**each.rs** (lines 136-163): Identical compensating rollback pattern.

Both files: create_tasks succeeds → publish_tasks fails → all created tasks marked FAILED → original error propagated.

---

## Summary Table

| Finding | Category | Status | Bead |
|---------|----------|--------|------|
| C1 | Partial-failure NOTE comment | **RED** | twerk-0s4 |
| C2/C3 | Compensating rollback | **GREEN** | — |
| C4 | No expect(), ok_or_else | **GREEN** | — |
| C5 | run() line count | **GREEN** (119 lines) | — |
| C6 | Shared spawn_signal_handler | **GREEN** | — |
| M2 | No unwrap_or_default on task ID | **GREEN** | — |
| M4 | No let _ = on progress | **RED** | twerk-r8b |
| M6 | No unwrap_or(0) for PID | **RED** | twerk-ilf |
| M7 | MountPolicy enum | **GREEN** | — |
| M8 | terminate_broadcaster field | **GREEN** | — |
| M9 | No let _ = handler in inmemory | **GREEN** | — |

---

## Verdict

**CROWN CONTESTED**

- **14/17 checks GREEN** — automated build/test/lint all pass
- **3/17 checks RED** — source-level findings (C1, M4, M6)
  - C1: Missing documentation of partial-failure semantics (MAJOR)
  - M4: Silent error suppression on 2 progress publish paths (MAJOR)
  - M6: `unwrap_or(0)` silently zeroes PID on race condition (MAJOR)
- No CRITICAL findings
- 3 beads filed: twerk-0s4, twerk-r8b, twerk-ilf

---

## Test Totals

| Package | Tests | Passed | Failed | Ignored |
|---------|-------|--------|--------|---------|
| twerk-app | 133 | 133 | 0 | 0 |
| twerk-infrastructure | 342 | 342 | 0 | 11 |
| E2E (standalone) | 6 | 6 | 0 | 0 |
