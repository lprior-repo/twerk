# Architectural Drift Review — twerk-0gr (Re-Review)

**STATUS: APPROVED**

**Reviewer:** Architectural Drift Agent
**Date:** 2026-04-22
**Scope:** Production handlers (7 files), banner.rs, test files (4 files)

---

## Summary

All 3 blocking issues from the previous review are **resolved**. The refactoring introduced a clean `handlers/common.rs` module that centralizes shared types and utilities. All handler files within the bead scope are under 300 lines. No new drift introduced.

---

## Blocking Issues — Resolution Verification

### B1. trigger.rs exceeds 300-line limit → FIXED ✅

**Before:** 330 lines
**After:** 298 lines

`TriggerView` and `TriggerErrorResponse` extracted to `common.rs`. File now contains only handler functions plus a re-export. Under the 300-line limit.

### B2. `encode_path_segment` duplicated 4× → FIXED ✅

**Before:** Identical private function in trigger.rs, queue.rs, task.rs, node.rs
**After:** Single canonical definition in `common.rs:9-11`

All consumers import via `use crate::handlers::common::encode_path_segment`:
- `trigger.rs:6`
- `queue.rs:8`
- `task.rs:8`
- `node.rs:8`

### B3. `TriggerErrorResponse` cross-context coupling → FIXED ✅

**Before:** Defined in `trigger.rs`, imported by queue/task/node/metrics/user (trigger → non-trigger coupling)
**After:** Defined in `common.rs:15-22`, imported by all 6 handler modules symmetrically

The type now lives in a neutral shared module. All handlers import from `common` — no handler-to-handler coupling remains. `trigger.rs` re-exports via `pub use` for backward compatibility.

---

## Production File Line Counts (Bead Scope: twerk-cli)

| File | Lines | Status |
|------|-------|--------|
| `handlers/common.rs` | 41 | ✅ NEW — shared types + utilities |
| `handlers/mod.rs` | 11 | ✅ Under 300 |
| `handlers/queue.rs` | 164 | ✅ Under 300 |
| `handlers/task.rs` | 201 | ✅ Under 300 |
| `handlers/node.rs` | 141 | ✅ Under 300 |
| `handlers/trigger.rs` | 298 | ✅ Under 300 |
| `handlers/metrics.rs` | 103 | ✅ Under 300 |
| `handlers/user.rs` | 96 | ✅ Under 300 |
| `banner.rs` | 134 | ✅ Under 300 |

**All files within bead scope are under 300 lines.**

---

## DDD & Cohesion Assessment

### `common.rs` — Single Responsibility ✅

The new module serves one purpose: shared infrastructure for CLI handler modules.

- `encode_path_segment()` — pure URL-encoding function
- `TriggerErrorResponse` — API error envelope DTO
- `TriggerView` — trigger resource DTO

Module is 41 lines, fully coherent, no business logic leakage.

### Handler Modules — Clean Boundaries ✅

Each handler file imports only what it needs from `common`:
- `trigger.rs`: `encode_path_segment` + re-exports `TriggerErrorResponse`, `TriggerView`
- `queue.rs`: `encode_path_segment`, `TriggerErrorResponse`
- `task.rs`: `encode_path_segment`, `TriggerErrorResponse`
- `node.rs`: `encode_path_segment`, `TriggerErrorResponse`
- `metrics.rs`: `TriggerErrorResponse`
- `user.rs`: `TriggerErrorResponse`

No circular dependencies. No handler-to-handler imports.

---

## Out-of-Scope Observation (Non-Blocking)

### O1. `task_handlers.rs` at 306 lines (twerk-app crate)

**File:** `crates/twerk-app/src/engine/coordinator/handlers/task_handlers.rs`
**Lines:** 306

This file exceeds the 300-line limit but is **outside this bead's scope** (`crates/twerk-cli` only). Recommend filing a follow-up bead for extraction of the redelivery/completion/failure handler group into a separate module.

---

## NON-BLOCKING Findings (Carried Forward)

### M1. Repetitive error-body parsing pattern across all handlers

Every handler function repeats the same ~8-line error-body parsing pattern. ~20 instances across 6 files. Pre-existing, not introduced by this bead. Recommend follow-up bead for a shared `fn parse_error_response(status, body) -> Result<(), CliError>`.

### M2. Test infrastructure duplication

`HttpTestServer` and `spawn_router()` identically implemented in:
- `tests/trigger_negative_test.rs:22-53`
- `tests/handler_error_body_test.rs:28-59`

Recommend extracting to `tests/common/mod.rs` in a follow-up bead. Test files are not subject to the 300-line production rule.

---

## Holzmann Compliance — PASS (Unchanged)

| Rule | Status | Evidence |
|------|--------|----------|
| No loops in test bodies | ✅ PASS | Zero `for` loops in any test function body |
| No `LazyLock`/shared mutable state | ✅ PASS | Replaced with `serial_test::serial` + `LoggingEnvGuard` |
| No bare `is_err()` | ✅ PASS | All error assertions use explicit `match` with variant binding |
| No `let _ =` value suppression | ✅ PASS | Only used for shutdown channel sends |
| Descriptive test names | ✅ PASS | All BDD tests use claim_/behavioral naming |

---

## Verdict

**APPROVED.** The 3 blocking issues are cleanly resolved. The `common.rs` extraction is well-targeted: single responsibility, correct granularity, no over-engineering. All bead-scoped production files are under 300 lines with clean module boundaries.
