# Test Suite Review — bead twerk-9td

## VERDICT: REJECTED

---

### Tier 0 — Static
[PASS] Banned pattern scan (no `assert!(result.is_ok())` etc.)
[PASS] Integration test purity (no `use crate::` in /tests/)
[PASS] Error variant completeness
[FAIL] **Density audit: 911 tests / 222 functions = 4.1x (target ≥5x)**
[FAIL] **Naming violations: 24 test functions in eval_test.rs use `fn test_` prefix**
[FAIL] **Loops in test bodies: 7+ files have `for ... in` loops in test functions**

### Tier 1 — Execution
[FAIL] **Clippy: 2 errors (cfg(kani) unexpected, manual_range_contains)**
[FAIL] **Tests: CANNOT COMPILE — 82 errors blocking all test execution**
[FAIL] Ordering probe: BLOCKED (tests don't compile)
[FAIL] Insta: BLOCKED (tests don't compile)

### Tier 2 — Coverage
[FAIL] **Coverage: BLOCKED — tests don't compile**

### Tier 3 — Mutation
[FAIL] **Mutation: BLOCKED — tests don't compile**

---

## LETHAL FINDINGS

### tests/eval_test.rs — Naming Violation (Holzmann Rule)
- **Line 43, 49, 62, 75, 88, 101, 114, 130, 137, 144, 154, 162, 169, 179, 190, 200, 214, 228, 242, 254** — 24 functions using `fn test_*` prefix instead of descriptive BDD names
- These are not in a separate `mod tests { ... }` inline block — they appear to be module-level test functions
- **REQUIRED**: Rename all to descriptive Given-When-Then style names (e.g., `fn empty_expression_returns_true` → `fn evaluating_empty_expression_yields_true_result`)

### crates/twerk-core/tests/red_queen_adversarial.rs, red_queen_gen2.rs — Compilation Errors
- **82 total errors** blocking test compilation
- `TriggerState` missing `Hash` trait (needed for HashMap::get)
- `TriggerState` missing `FromStr` trait (needed for .parse())
- `twerk_core::ParseTriggerStateError` does not exist (import unresolved)
- `TriggerState::default` does not exist
- **REQUIRED**: Either implement missing traits on TriggerState, or remove tests that depend on them

### crates/twerk-core/tests/ — Loops in Test Bodies (Holzmann Rule 2)
Files with `for ... in` loops directly in test functions:
- `validation_test.rs:78` — `for i in 1..=10`
- `validation_test.rs:93` — `for i in 0..=9`
- `asl_functions_test.rs:104` — `for _ in 0..20`
- `asl_transition_test.rs:233` — `for js in [JitterStrategy::Full, JitterStrategy::None]`
- `asl_types_test.rs:1125, 1304, 1374, 1402` — multiple for-loops
- `asl_validation_test.rs:58` — `for (name, state) in states`
- `red_queen_adversarial.rs` — 10+ instances
- **REQUIRED**: Convert all to rstest parametrize or proptest

### crates/twerk-core/src/trigger.rs:2063 — Clippy Error
- `#[cfg(kani)]` is not a recognized cfg condition
- **REQUIRED**: Replace with proper feature flag or remove

### crates/twerk-core/src/trigger.rs:41 — Clippy Error
- Manual `!RangeInclusive::contains` implementation
- **REQUIRED**: Use `(3..=64).contains(&len)` as clippy suggests

---

## MAJOR FINDINGS (6)

1. **Density Insufficient**: 911 tests / 222 functions = 4.1x (target ≥5x)
2. **TriggerState incomplete**: Missing Hash, FromStr, Default, Display/FromStr integration
3. **eval_test.rs naming**: All 24 test functions violate Holzmann naming
4. **Multiple files with loops**: 7+ test files violate Rule 2 (no loops in test bodies)
5. **red_queen_adversarial.rs**: 38 compilation errors alone
6. **red_queen_gen2.rs**: 30 compilation errors alone

---

## MINOR FINDINGS

1. `red_queen_adversarial.rs:880` and `red_queen_gen3.rs:525-531` — `let _ = err.to_string()` pattern (though technically not silent error suppression since to_string() returns String, not Result)
2. No insta snapshots detected (good - not in use)

---

## MANDATE

The following MUST exist before resubmission:

1. **TriggerState trait implementation**:
   - `impl Hash for TriggerState` (required by HashMap usage in tests)
   - `impl FromStr for TriggerState` (required by .parse() in tests)
   - `impl Default for TriggerState` or remove .default() calls
   - OR: Remove/redesign tests that require these traits

2. **eval_test.rs naming fix**: All 24 `fn test_*` → descriptive BDD names

3. **Loop elimination**: All `for ... in` in test functions → rstest/parametrize or proptest

4. **Clippy fixes**:
   - `#[cfg(kani)]` → proper feature flag
   - Manual range contains → std library method

5. **Density improvement**: Need ≥5x ratio (≥1110 tests for 222 functions OR reduce functions)

---

## NOTES

The user mentioned "5 new Display/AsRef tests were added" but the current suite has 82 compilation errors preventing ANY tests from running. The Display/AsRef tests in `asl_types_test.rs` appear well-structured but cannot execute due to other broken tests in the crate.

**Full re-run from Tier 0 required after any fix.**
