# TriggerError Defects — Black Hat Review

## Session Info
- **Date**: 2026-04-13
- **Reviewer**: black-hat-reviewer
- **Target**: `TriggerError` enum in `crates/twerk-core/src/trigger/types.rs`
- **Status**: REJECTED

---

## CRITICAL DEFECTS

### [DEFECT-1] BLOCKER: Test Suite Does Not Compile

**Location**: `crates/twerk-core/src/trigger/tests.rs`

**Severity**: CRITICAL - Blocks all testing

**Issue**: 13 compilation errors due to API mismatch between implementation and tests.

| Error Type | Count | Description |
|------------|-------|-------------|
| `E0061` | 8 | Wrong number of arguments to variants |
| `E0599` | 5 | Nonexistent variants referenced |

**Specific Mismatches**:

| Test Location | Test Usage | Implementation Reality |
|---------------|------------|------------------------|
| Line 209 | `InvalidStateTransition(TriggerState, TriggerState)` | `InvalidStateTransition(String)` |
| Line 237 | `TriggerNotActive(TriggerState)` | Variant does not exist |
| Line 260 | `InvalidConfiguration(String)` | Variant does not exist |
| Line 642 | `InvalidConfiguration(...)` | Variant does not exist |
| Line 660 | `InvalidConfiguration(...)` | Variant does not exist |
| Line 876 | `InvalidStateTransition(TriggerState, TriggerState)` | `InvalidStateTransition(String)` |
| Lines 898, 920, 942, 964, 986 | Same `InvalidStateTransition` mismatch | `InvalidStateTransition(String)` |
| Line 1023, 1177, 1202 | `TriggerNotActive` | Variant does not exist |
| Line 1227 | `TriggerDisabled` | Works correctly |
| Line 1254 | `TriggerInErrorState` | Works correctly |

**Impact**: No tests can run. The entire trigger module is untested.

**Required Fix**: Tests must be rewritten to match the actual 19-variant implementation.

---

### [DEFECT-2] CRITICAL: InvalidStateTransition Signature Changed

**Location**: `types.rs:190`

**Current**:
```rust
#[error("invalid state transition: {0}")]
InvalidStateTransition(String),
```

**Test Expectation** (line 209):
```rust
TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error)
```

**Issue**: The variant was changed from 2-arity (two TriggerStates) to 1-arity (String), breaking all callers.

---

### [DEFECT-3] MAJOR: Missing Variants Referenced in Tests

| Missing Variant | Used At | Replacement Available |
|----------------|---------|----------------------|
| `TriggerNotActive` | Lines 237, 1023, 1177, 1202 | `TriggerDisabled` (different semantics) |
| `InvalidConfiguration` | Lines 260, 642, 660 | `InvalidCronExpression`, `InvalidInterval`, etc. |

**Issue**: Tests reference variants that were removed or renamed in the 19-variant implementation.

---

## PREVIOUSLY REPORTED (Now Fixed)

These issues from the previous review have been resolved:

| Defect | Status | Verification |
|--------|--------|--------------|
| Missing `Hash` trait | FIXED | Line 172: `#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]` |
| Missing `From<serde_json::Error>` | FIXED | Lines 239-242: Implemented as `PollingExpressionError` |

---

## CONTRACT VIOLATION SUMMARY

**Phase 1 - Contract Parity**: FAIL
- Implementation has 19 variants as specified
- BUT: Tests reference non-existent variants and wrong signatures
- Test parity is BROKEN

**Phase 2 - Farley Engineering Rigor**: PASS
- All functions under 25 lines
- No functions exceed 5 parameters
- Pure logic separated from I/O

**Phase 3 - NASA-Level Functional Rust**: PASS
- Sum types used correctly
- No boolean parameters
- Newtypes properly defined

**Phase 4 - Ruthless Simplicity & DDD**: PASS
- No unwrap/expect/panic in core logic
- CUPID properties satisfied

**Phase 5 - Bitter Truth (Legibility)**: PASS
- Code is readable and obvious
- No clever tricks

---

## VERDICT

**STATUS: REJECTED**

The implementation itself is technically correct (19 variants, Hash, From implementations present), but the TEST SUITE DOES NOT COMPILE. This is a catastrophic contract violation - the tests were written for an older/different API and have not been updated.

**Required Actions**:
1. Rewrite all broken test cases to use actual TriggerError variants
2. Fix `InvalidStateTransition` calls to use `String` instead of two `TriggerState` arguments
3. Remove or replace references to `TriggerNotActive` and `InvalidConfiguration`
4. Re-run full test suite to verify compilation and correctness

**Do not approve until `cargo test -p twerk-core --lib` passes without errors.**
