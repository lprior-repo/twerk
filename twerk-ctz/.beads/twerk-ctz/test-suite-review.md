# Test Suite Review: twerk-ctz (TriggerError Enum)

## VERDICT: REJECTED

---

## Executive Summary

The test suite **cannot compile** due to broken kani proofs and **contains 9 failing tests** that use the old error API (`InvalidConfiguration`, `TriggerNotActive`) that was replaced per the contract specification. The tests do NOT properly validate the contract — they validate a previous version of the API.

**Critical Blocker**: The `#[cfg(test)] mod kani_proofs` block at trigger.rs:2076-2134 references the `kani` crate which is not a dev-dependency. This causes `cargo test` to fail at compile time for the entire `twerk-core` crate.

---

## Tier 0 — Static Analysis

### [FAIL] Banned Pattern Scan

**LETHAL FINDINGS:**

1. **Banned assertion: `assert!(result.is_ok())`**
   - `crates/twerk-core/src/trigger.rs:1719` — `fire_returns_job_id_when_trigger_is_active_and_broker_available` uses `assert!(result.is_ok())` instead of `assert_eq!(result, Ok(...))`
   - `crates/twerk-core/tests/trigger_registry_test.rs:362` — same violation in integration test

2. **Loop in test body (Holzmann Rule 2)**
   - `crates/twerk-core/src/trigger.rs:1137-1150` — `is_valid_transition_self_is_valid_for_all_states` uses nested `for` loops over states and variants instead of cartesian product or proptest

3. **Kani proofs module non-compilation**
   - `crates/twerk-core/src/trigger.rs:2076-2134` — `#[cfg(test)] mod kani_proofs` uses `#[kani::proof]` and `kani::any()` and `kani::assert()` which require the kani crate (not in Cargo.toml dev-dependencies)
   - This is NOT a conditional compilation issue — the kani attributes are NOT gated by a feature flag, only by `#[cfg(test)]` which is true for cargo test

### [FAIL] Test Naming Violations

**LETHAL FINDINGS** (30 violations across multiple files):
- `crates/twerk-core/src/node.rs:76` — `fn test_node_clone()`
- `crates/twerk-core/src/node.rs:92` — `fn test_node_status_from_str()`
- `crates/twerk-core/src/redact/tests.rs:12,25,36,44,52,63,74,87,98,109,130,154,171` — 13 `fn test_*` functions
- `crates/twerk-core/src/stats.rs:44,64,71,78,88,95` — 6 `fn test_*` functions
- `crates/twerk-core/src/host.rs:26,35` — 2 `fn test_*` functions
- `crates/twerk-core/src/hash.rs:22,30` — 2 `fn test_*` functions
- `crates/twerk-core/src/uuid.rs:69,75,90,95,105` — 5 `fn test_*` functions

These should use `#[test]` attribute, not `fn test_*` naming convention.

### [PASS] Mock Interrogation
No mocks found in twerk-core/src/.

### [PASS] Integration Test Purity
Integration tests in `/tests/` directory use `twerk_core::trigger` public API correctly. No `use crate::internal` paths found.

### [FAIL] Error Variant Completeness

**LETHAL FINDINGS:**

The contract specifies 19 TriggerError variants. The implementation has 20 (including `InvalidConfiguration` and `TriggerNotActive` which are NOT in contract) and is MISSING `InvalidTimezone` which IS in contract.

**Contract says REMOVED:**
- `TriggerNotActive` — replaced by `TriggerDisabled`
- `InvalidConfiguration` — replaced by `JobCreationFailed`

**Contract says ADDED:**
- `InvalidTimezone` — MISSING from implementation

**Tests using old API (examples):**
- `crates/twerk-core/src/trigger.rs:857` — `TriggerError::TriggerNotActive` display test
- `crates/twerk-core/src/trigger.rs:880` — `TriggerError::InvalidConfiguration` display test
- `crates/twerk-core/src/trigger.rs:1205,1223` — inline tests expect `InvalidConfiguration`
- `crates/twerk-core/src/trigger.rs:1762` — expects `TriggerNotActive(TriggerState::Paused)`
- `crates/twerk-core/tests/trigger_registry_test.rs:102-107` — expects `InvalidConfiguration`
- `crates/twerk-core/tests/trigger_registry_test.rs:122-125` — expects `InvalidConfiguration`
- `crates/twerk-core/tests/trigger_registry_test.rs:429-434` — expects `TriggerNotActive`

### [PASS] Density Audit

| Metric | Value |
|--------|-------|
| Public functions (twerk-core) | 99 |
| Unit tests (twerk-core) | 193 |
| Ratio | 1.95x |

Note: Overall ratio is low, but for the trigger.rs module specifically, there are ~90 tests for the trigger module and its types. The ratio concern is secondary to the compilation failure.

---

## Tier 1 — Execution

### [FAIL] Clippy: Cannot Run

Cannot execute due to compilation failure from kani proofs.

### [FAIL] nextest: Cannot Run

Cannot execute due to compilation failure from kani proofs.

### [FAIL] Ordering Probe: Cannot Run

Cannot execute due to compilation failure from kani proofs.

### [NA] Insta: Not Present

`insta` not in Cargo.toml.

---

## Tier 2 — Coverage

### Cannot Execute

Coverage cannot be measured because the test suite does not compile.

---

## Tier 3 — Mutation

### Cannot Execute

Mutation testing cannot be performed because the test suite does not compile.

---

## LETHAL FINDINGS

1. **`crates/twerk-core/src/trigger.rs:1719`** — `assert!(result.is_ok())` is a banned assertion (use `assert_eq!(result, Ok(...))`)

2. **`crates/twerk-core/src/trigger.rs:1137-1150`** — Loop in test body (`is_valid_transition_self_is_valid_for_all_states`)

3. **`crates/twerk-core/src/trigger.rs:2076-2134`** — Kani proofs module references unavailable `kani` crate, causing compile failure for all tests in the crate

4. **`crates/twerk-core/src/trigger.rs:857`** — Test `trigger_error_trigger_not_active_displays_correctly` tests `TriggerNotActive` which is NOT in contract

5. **`crates/twerk-core/src/trigger.rs:880`** — Test `trigger_error_invalid_configuration_displays_correctly` tests `InvalidConfiguration` which is NOT in contract

6. **`crates/twerk-core/src/trigger.rs:1205,1223`** — Inline tests expect `TriggerError::InvalidConfiguration` but contract specifies `JobCreationFailed`

7. **`crates/twerk-core/src/trigger.rs:1762`** — `fire_returns_trigger_not_active_when_trigger_is_paused` expects `TriggerNotActive` but contract specifies `TriggerDisabled`

8. **`crates/twerk-core/tests/trigger_registry_test.rs:362`** — `assert!(result.is_ok())` is a banned assertion

9. **`crates/twerk-core/tests/trigger_registry_test.rs:102-107`** — Expects `InvalidConfiguration` but contract specifies `JobCreationFailed`

10. **`crates/twerk-core/tests/trigger_registry_test.rs:122-125`** — Expects `InvalidConfiguration` but contract specifies `JobCreationFailed`

11. **`crates/twerk-core/tests/trigger_registry_test.rs:429-434`** — Expects `TriggerNotActive` but contract specifies `TriggerDisabled`

12. **30 test naming violations** — `fn test_*` instead of `#[test]` attribute in node.rs, redact/tests.rs, stats.rs, host.rs, hash.rs, uuid.rs

---

## MAJOR FINDINGS (3)

1. **Implementation-Contract Mismatch**: The implementation has `InvalidConfiguration` (line 149-150) and `TriggerNotActive` (line 164-165) which are NOT in the contract. The contract says these were removed and replaced with `JobCreationFailed` and `TriggerDisabled` respectively.

2. **Missing Contract Variant**: The contract specifies `InvalidTimezone` variant which is NOT present in the implementation.

3. **9 Test Failures Observed**: Running `cargo test -p twerk-core --test trigger_registry_test` shows 9 tests failing because they assert against the old API:
   - `trigger_registry_register_fails_for_disabled_state` — expects `InvalidConfiguration`, gets `JobCreationFailed`
   - `trigger_registry_register_fails_for_error_state` — expects `InvalidConfiguration`, gets `JobCreationFailed`
   - `trigger_registry_fire_fails_for_paused_trigger` — expects `TriggerNotActive`, gets `TriggerDisabled`
   - And 6 more

---

## MANDATE

The following MUST be resolved before resubmission:

### Immediate Blockers (MUST FIX)

1. **Kani proofs module** — Either:
   - Remove the `kani_proofs` module entirely, OR
   - Gate it behind a feature flag (`#[cfg(kani)]`), OR
   - Add `kani` as a dev-dependency

2. **All tests asserting old error variants** — Update to use contract-specified variants:
   - `InvalidConfiguration` → `JobCreationFailed`
   - `TriggerNotActive` → `TriggerDisabled`
   - Add test for `InvalidTimezone` (contract new variant)

3. **Remove tests for non-existent variants** — Delete:
   - `trigger_error_trigger_not_active_displays_correctly`
   - `trigger_error_invalid_configuration_displays_correctly`
   - Any test using `TriggerError::TriggerNotActive` or `TriggerError::InvalidConfiguration`

4. **Fix banned assertion** at `trigger.rs:1719` and `trigger_registry_test.rs:362`:
   - Change `assert!(result.is_ok())` to `assert_eq!(result, Ok(expected_job_id))`

5. **Fix loop in test body** at `trigger.rs:1136-1151`:
   - Replace nested `for` loops with cartesian product test via rstest, or proptest

6. **Fix test naming violations** — Convert all `fn test_*` to use `#[test]` attribute in:
   - node.rs (2 violations)
   - redact/tests.rs (13 violations)
   - stats.rs (6 violations)
   - host.rs (2 violations)
   - hash.rs (2 violations)
   - uuid.rs (5 violations)

### Resubmission Requirement

After fixing, the suite MUST:
1. Compile without errors (`cargo test -p twerk-core --lib`)
2. Pass all tests (`cargo test -p twerk-core --test trigger_registry_test`)
3. Pass clippy with zero warnings
4. Pass Tier 0 through Tier 3 of this review

**Status: REJECTED — Full re-review required from Tier 0 after fixes.**
