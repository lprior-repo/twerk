# ASL Module Test Suite Review — The Inquisition

**Date**: 2025-07-24
**Mode**: 2 — Suite Inquisition
**Scope**: 8 test files (359 tests) + adversarial (183 tests) = 542 tests
**Source under test**: `crates/twerk-core/src/asl/` (15 files) + `eval/intrinsics.rs` + `eval/data_flow.rs`

---

## VERDICT: **STATUS: REJECTED**

3 LETHAL categories (33 specific instances). 3 MAJOR categories (56+ instances).
Suite is not safe to ship. Rewrite required before re-review.

---

## Tier 0 — Static Analysis

### [FAIL] Banned Pattern Scan — 24 banned assertions

Every `assert!(x.is_ok())` / `assert!(x.is_err())` is a test that proves nothing about
the *value* — only that some result exists. If the implementation changes error type,
these tests keep passing and the bug ships.

**`assert!(*.is_ok())` — 9 instances (no value verification):**

| File | Line | Code | Severity |
|------|------|------|----------|
| `asl_machine_test.rs` | 267 | `assert!(m.validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 416 | `assert!(m.validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 430 | `assert!(m.validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 509 | `assert!(m.validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 537 | `assert!(m.validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 541 | `assert!(ps.branches()[0].validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 542 | `assert!(ps.branches()[1].validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 565 | `assert!(m.validate().is_ok())` | LETHAL |
| `asl_machine_test.rs` | 567 | `assert!(ms.item_processor().validate().is_ok())` | LETHAL |

**Fix**: Replace with `assert_eq!(m.validate(), Ok(()))` or assert structural properties.

**`assert!(*.is_err())` — 15 instances without exact variant check:**

| File | Line | Code | Severity |
|------|------|------|----------|
| `asl_transition_test.rs` | 208 | `assert!(result.is_err())` — JitterStrategy reject | LETHAL |
| `asl_transition_test.rs` | 216 | `assert!(result.is_err())` — JitterStrategy reject | LETHAL |
| `asl_container_test.rs` | 400 | `assert!(result.is_err())` — heartbeat >= timeout serde | LETHAL |
| `asl_container_test.rs` | 500 | `assert!(result.is_err())` — parallel empty branches serde | LETHAL |
| `asl_container_test.rs` | 717 | `assert!(result.is_err())` — map invalid tolerance serde | LETHAL |
| `asl_machine_test.rs` | 495 | `assert!(result.is_err())` — unknown state type serde | LETHAL |
| `asl_functions_test.rs` | 63 | `assert!(r.is_err())` — stringToJson invalid | LETHAL |
| `asl_functions_test.rs` | 121 | `assert!(r.is_err())` — jsonToString non-JSON | LETHAL |
| `asl_functions_test.rs` | 197 | `assert!(r.is_err())` — mathAdd wrong types | LETHAL |
| `asl_functions_test.rs` | 216 | `assert!(r.is_err())` — mathSub wrong types | LETHAL |
| `asl_functions_test.rs` | 239 | `assert!(r.is_err())` — hash invalid algo | LETHAL |
| `asl_functions_test.rs` | 285 | `assert!(r.is_err())` — arrayRange bad args | LETHAL |
| `asl_data_flow_test.rs` | 65 | `assert!(result.is_err())` — redundant (variant checked below via match) | MINOR |
| `asl_data_flow_test.rs` | 77 | `assert!(result.is_err())` — redundant (variant checked below via match) | MINOR |
| `asl_data_flow_test.rs` | 89 | `assert!(result.is_err())` — redundant (variant checked below via match) | MINOR |

**Note**: `asl_data_flow_test.rs` lines 65/77/89/150 are mitigated — each has a
`match result.unwrap_err() { DataFlowError::SpecificVariant { .. } => ... }` immediately
after. The `is_err()` is redundant but the variant IS verified. These are MINOR (cleanup).

**Fix**: Replace every `assert!(x.is_err())` with `assert!(matches!(x, Err(SpecificError::SpecificVariant { .. })))` or use `unwrap_err()` + `assert_eq!`.

---

### [FAIL] Holzmann Rule 2 — 6 Loops in Test Bodies

A test with a loop is a program. Programs have bugs. Use `rstest` parameterization
or proptest instead.

| File | Line | Loop | Assessment |
|------|------|------|------------|
| `asl_functions_test.rs` | 103 | `for _ in 0..20` | **LETHAL** — random test with hidden iteration. Use proptest. |
| `asl_transition_test.rs` | 231 | `for js in [Full, None]` | **LETHAL** — use `#[rstest]` parameterized test. |
| `asl_types_test.rs` | 1125 | `for input in &["timeout", ...]` | **LETHAL** — use `#[rstest]` case expansion. |
| `asl_types_test.rs` | 1304 | `for variant in &variants` | **LETHAL** — serde roundtrip loop. Use rstest. |
| `asl_types_test.rs` | 1374 | `for (input, expected) in &cases` | **LETHAL** — FromStr mapping loop. Use rstest. |
| `asl_types_test.rs` | 1402 | `for variant in &variants` | **LETHAL** — Display roundtrip loop. Use rstest. |

**Not counted**: `asl_validation_test.rs:86` — loop is in `build_machine()` helper (setup
code, not test assertion logic). Acceptable.

**Fix**: Convert each loop to `#[rstest]` parameterized tests or individual tests.
The `math_random_in_range` test at line 103 MUST become a proptest with statistical
bounds verification.

---

### [PASS] Mock Interrogation
No mockall or mock usage found. Clean.

### [PASS] Integration Test Purity
No `use crate::` in test files. All tests use public API via `use twerk_core::`.

### [PASS] Shared Mutable State
No `static mut`, `lazy_static!`, or `once_cell` in test code.

### [PASS] `#[ignore]` Tests
None found. All tests active.

### [PASS] Sleep / Non-Determinism
No `sleep` calls in test code.

### [FAIL] Error Variant Completeness

**Untested error variant:**

| Source | Variant | Status |
|--------|---------|--------|
| `eval/data_flow.rs` | `DataFlowError::InvalidPath { path, reason }` | **LETHAL** — NO test triggers this variant in `asl_data_flow_test.rs` |

**Weakly tested (string matching instead of variant matching):**

| File | Lines | Variants | Method |
|------|-------|----------|--------|
| `asl_states_test.rs` | 237,244,252,260 | `WaitDurationError::NoFieldSpecified`, `MultipleFieldsSpecified`, `EmptyTimestamp`, `InvalidJsonPath` | `err.to_string().contains(...)` — MAJOR |

**Fix**: The `WaitDuration` error tests must use `match` or `assert!(matches!(...))` on the
actual error variant. String matching is fragile — a Display impl change breaks nothing
and the test starts lying.

### [PASS] Density Audit
- **Tests**: 542 (359 in-scope + 183 adversarial covering same source)
- **Public functions**: 102
- **Ratio**: 542 / 102 = **5.3×** — PASSES (target ≥ 5×)

---

## Tier 1 — Compilation + Execution

### [FAIL] Clippy: 2 errors

```
error: approximate value of `f{32, 64}::consts::E` found
  --> crates/twerk-core/tests/asl_types_test.rs:906:35
      let br = BackoffRate::new(2.718);
                                ^^^^^
  --> crates/twerk-core/tests/asl_types_test.rs:922:27
      (de.value() - 2.718).abs() < f64::EPSILON
                    ^^^^^
```

`cargo clippy --tests --all-features -- -D warnings` produces 2 `approx_constant` errors.
Clippy failure = **LETHAL**.

**Fix**: Use a non-constant value like `2.5` or `1.618`, or use `std::f64::consts::E` explicitly
if Euler's number is the intended test value.

### [PASS] Tests Pass
All 862 tests across the crate pass (0 failures, 0 ignored).

```
asl_types_test:       107 passed
asl_transition_test:   50 passed
asl_states_test:       53 passed
asl_container_test:    36 passed
asl_machine_test:      35 passed
asl_validation_test:   16 passed
asl_data_flow_test:    24 passed
asl_functions_test:    38 passed
asl_adversarial_test: 183 passed
```

### [SKIP] Ordering Probe
Not executed (would require nextest). Tests are fast and share no state — low risk.

### [SKIP] Insta Staleness
Insta not used in this crate.

---

## Tier 2 — Coverage Analysis

Coverage tooling (llvm-cov) not executed. Manual coverage analysis performed by
cross-referencing every public function against test catalogs.

### Public API Coverage Map

**Fully covered (exact value assertions):**
- `StateName` — new/as_str/Display/FromStr/Serialize/Deserialize ✓
- `Expression` — new/as_str/serde ✓
- `JsonPath` — new/as_str/serde ✓
- `VariableName` — new/as_str/serde ✓
- `ImageRef` — new/as_str/serde ✓
- `ShellScript` — new/as_str/serde ✓
- `BackoffRate` — new/value/serde ✓
- `ErrorCode` — all 11 variants, Display, FromStr, Serialize, Deserialize ✓
- `Transition` — next/end/is_next/is_end/target_state/serde ✓
- `Retrier` — new + all 6 accessors + all 4 error variants ✓
- `Catcher` — new + all 4 accessors + EmptyErrorEquals ✓
- `ChoiceRule` — new + 3 accessors ✓
- `ChoiceState` — new + choices/default + EmptyChoices ✓
- `WaitDuration` — all 4 variants + 4 is_* methods + Display + serde ✓
- `WaitState` — new + duration/transition ✓
- `PassState` — new + result/transition ✓
- `SucceedState` — new + serde ✓
- `FailState` — new + error/cause/Display + serde ✓
- `TaskState` — new + 9 accessors + all 4 error variants ✓
- `ParallelState` — new + 3 accessors + EmptyBranches ✓
- `MapState` — new + 7 accessors + 2 error variants ✓
- `StateKind` — is_terminal (8 variants) + transition (8 variants) ✓
- `State` — struct fields + serde ✓
- `StateMachine` — validate + get_state + start_state + serde ✓
- `StateMachineError` — all 6 variants via matches!() ✓
- `ValidationReport` — analyze + is_clean ✓
- `DataFlowError` — PathNotFound ✓, NotAnObject ✓

**Covered but weakly (is_ok/is_err/is_some/is_none only):**
- `StateMachine::validate()` happy path — 9 tests use `is_ok()` without structural verification

**GAPS — untested public API:**

| Function/Variant | Source File | Status |
|-----------------|-------------|--------|
| `DataFlowError::InvalidPath` | `eval/data_flow.rs` | **UNTESTED** — no test triggers this variant |
| `ErrorCode::matches()` | `asl/error_code.rs` | Tested only in adversarial_test.rs (outside scope files) — covered |

---

## Tier 3 — Mutation Survivability Assessment

No `cargo-mutants` execution performed (tooling not available). Manual mutation analysis follows.

### Mutations That Would Survive

**M1: Lobotomize `StateMachine::validate()` → always return `Ok(())`**
- **9 tests survive**: All `assert!(m.validate().is_ok())` in `asl_machine_test.rs` (lines 267, 416, 430, 509, 537, 541, 542, 565, 567)
- **Which test catches it**: The 7 error-path tests DO catch this (lines 278-403). But the happy-path tests add zero mutation resistance.
- **Required fix**: Replace `is_ok()` with `assert_eq!(m.validate(), Ok(()))` — same thing. OR verify a structural property of the validated machine that wouldn't hold if validation was skipped.

**M2: Change intrinsic error → return `Ok(Value::Null)` instead of `Err`**
- **6 tests survive**: All `assert!(r.is_err())` in `asl_functions_test.rs` that don't check the error message
- **Required fix**: Each error test must verify the error string contains a meaningful identifier

**M3: Swap `>` to `>=` in `RetrierError::MaxDelayNotGreaterThanInterval` check**
- **Caught by**: `asl_transition_test.rs` tests at boundary values ✓ (tests exist for max_delay == interval)

**M4: Delete `DataFlowError::InvalidPath` branch**
- **No test catches this**: The `InvalidPath` variant has zero test coverage
- **Required test**: `data_flow_rejects_invalid_path_syntax`

**M5: Change `ChoiceStateError::EmptyChoices` → accept empty vec**
- **Caught by**: `asl_states_test.rs:116` — `assert_eq!(err, ChoiceStateError::EmptyChoices)` ✓

**M6: Return `Some(wrong_value)` from any accessor**
- **52 tests survive**: All `assert!(x.is_some())` / `assert!(x.is_none())` tests
- **Example**: `asl_container_test.rs:124` — `assert!(ts.env().get("API_KEY").is_some())` — survives if env returns `{"API_KEY": "WRONG"}`
- **Required fix**: Replace with `assert_eq!(ts.env().get("API_KEY"), Some(&Expression::new("secret").unwrap()))`

**M7: WaitDuration error Display message changes**
- **4 tests survive**: States test lines 237/244/252/260 use `err.to_string().contains(...)` — if the Display impl changes wording, these tests either break spuriously (false negative) or pass silently (false positive). Both are bad.
- **Required fix**: Match on the enum variant directly, not the string representation

### Happy Path Without Error Path (or Vice Versa)

| Test | Missing Path |
|------|-------------|
| `asl_functions_test.rs` — `string_to_json_invalid` | Checks `is_err()` only. No error content verification. |
| `asl_functions_test.rs` — `math_add_wrong_types` | Checks `is_err()` only. No error content verification. |
| `asl_functions_test.rs` — `math_sub_wrong_types` | Checks `is_err()` only. No error content verification. |
| `asl_functions_test.rs` — `hash_invalid_algo` | Checks `is_err()` only. No error content verification. |
| `asl_functions_test.rs` — `array_range_bad_args` | Checks `is_err()` only. No error content verification. |
| `asl_functions_test.rs` — `json_to_string_non_json` | Checks `is_err()` only. No error content verification. |

---

## LETHAL FINDINGS (33 instances — 4 categories)

| # | Category | Count | Files |
|---|----------|-------|-------|
| L1 | Banned `is_ok()`/`is_err()` assertions (no variant check) | 21 | machine, transition, container, functions |
| L2 | Loops in test bodies (Holzmann Rule 2) | 6 | functions, transition, types |
| L3 | `DataFlowError::InvalidPath` untested variant | 1 | data_flow |
| L4 | Clippy failure (`approx_constant`) | 2 | types |

## MAJOR FINDINGS (3 categories)

| # | Category | Count | Files |
|---|----------|-------|-------|
| M1 | Weak `is_some()`/`is_none()` assertions | 52 | container(20), machine(9), states(11), transition(1) |
| M2 | WaitDuration error tests use string matching | 4 | states |
| M3 | Intrinsic error tests check `is_err()` only (no message/type) | 6 | functions |

## MINOR FINDINGS (4 instances)

| # | Category | Count | Files |
|---|----------|-------|-------|
| m1 | Redundant `is_err()` before `match unwrap_err()` | 4 | data_flow |

---

## MANDATE

Before resubmission, **every item below must be addressed**. Resubmission triggers
full re-review from Tier 0. Not just the failing tier.

### REQUIRED FIXES

1. **Fix Clippy errors** in `asl_types_test.rs:906,922` — replace `2.718` with a non-constant
   value like `2.5` or use `std::f64::consts::E` explicitly.

2. **Replace all 21 bare `is_ok()`/`is_err()` assertions** with exact value/variant checks:
   - `asl_machine_test.rs` (9 lines): `assert_eq!(m.validate(), Ok(()))`
   - `asl_transition_test.rs:208,216`: Assert the serde error contains the expected rejection reason
   - `asl_container_test.rs:400,500,717`: Assert the serde error identifies the validation failure
   - `asl_machine_test.rs:495`: Assert the serde error identifies unknown state type
   - `asl_functions_test.rs` (6 lines): Verify each error string contains the function name or argument issue

3. **Eliminate all 6 loops in test bodies**:
   - `asl_functions_test.rs:103` → Convert `math_random_in_range` to a proptest with
     `prop_assert!((start..end).contains(&n))` for arbitrary `(start, end)` pairs
   - `asl_transition_test.rs:231` → Split into `js9_roundtrip_full` and `js9_roundtrip_none`
   - `asl_types_test.rs:1125` → Use `#[rstest]` with `#[case]` for each input string
   - `asl_types_test.rs:1304,1374,1402` → Use `#[rstest]` parameterization per variant

4. **Add test for `DataFlowError::InvalidPath`**:
   ```rust
   #[test]
   fn input_path_with_invalid_syntax_returns_invalid_path_error() {
       // Trigger a path that produces InvalidPath { path, reason }
       // Verify: assert!(matches!(err, DataFlowError::InvalidPath { path, .. } if path == "..."))
   }
   ```

5. **Replace 52 weak `is_some()`/`is_none()` assertions** with exact value checks:
   - Example: `assert!(ts.env().get("API_KEY").is_some())` →
     `assert_eq!(ts.env().get("API_KEY").map(|e| e.as_str()), Some("secret"))`

6. **Replace 4 WaitDuration string-matching error tests** with variant matching:
   - `asl_states_test.rs:237` → `assert!(matches!(err_inner, WaitDurationError::NoFieldSpecified))`
   - `asl_states_test.rs:244` → `assert!(matches!(err_inner, WaitDurationError::MultipleFieldsSpecified { .. }))`
   - `asl_states_test.rs:252` → `assert!(matches!(err_inner, WaitDurationError::EmptyTimestamp))`
   - `asl_states_test.rs:260` → `assert!(matches!(err_inner, WaitDurationError::InvalidJsonPath(_)))`
   (Note: these errors come through serde — may need to extract the inner error from the serde error.)

7. **Remove 4 redundant `is_err()` calls** in `asl_data_flow_test.rs:65,77,89,150` — the
   `match unwrap_err()` on the next line already covers this.

### NAMED TESTS REQUIRED FOR SURVIVING MUTANTS

| Required Test Name | Source | Mutation Killed |
|-------------------|--------|-----------------|
| `data_flow_rejects_invalid_path_syntax` | `asl_data_flow_test.rs` | M4: delete InvalidPath branch |
| `string_to_json_invalid_reports_parse_error_message` | `asl_functions_test.rs` | M2: change error to Ok(Null) |
| `math_add_wrong_types_reports_type_mismatch` | `asl_functions_test.rs` | M2: change error to Ok(Null) |
| `math_sub_wrong_types_reports_type_mismatch` | `asl_functions_test.rs` | M2: change error to Ok(Null) |
| `hash_invalid_algo_reports_algorithm_name` | `asl_functions_test.rs` | M2: change error to Ok(Null) |
| `array_range_bad_args_reports_expected_count` | `asl_functions_test.rs` | M2: change error to Ok(Null) |
| `json_to_string_non_json_reports_error_content` | `asl_functions_test.rs` | M2: change error to Ok(Null) |

---

**End of review. Suite rejected. Fix and resubmit for full Tier 0–3 re-review.**
