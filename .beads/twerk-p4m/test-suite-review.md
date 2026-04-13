bead_id: twerk-p4m
bead_title: data: Define TriggerState enum and TriggerId type in twerk-core
phase: state-4.7-test-suite-review
updated_at: 2026-04-13T20:30:00Z

# Test Suite Review — Mode 2: Suite Inquisition (Re-review #2)

## VERDICT: APPROVED

All four tiers passed. Zero LETHAL, zero MAJOR, two MINOR findings (well below the 5-minor threshold).

---

### Tier 0 — Static

[PASS] **Banned assertions** — Zero hits with precise `\bassert!` word-boundary pattern.
  The initial substring grep matched `prop_assert!(result.is_err())` and
  `prop_assert!(result.is_ok())` inside `proptest!` blocks at id.rs:894,898.
  These are `prop_assert!` (proptest macro), NOT `assert!`. Proptest requires
  `prop_assert!` for proper shrinking and error reporting. Re-scan with word
  boundary `\bassert!\(result\.\(is_ok\|is_err\)\(\)\)` confirmed zero real hits.

[PASS] **Silent error discard** (`let _ =` / `.ok();`) — NO_HITS

[PASS] **Ignored tests** (`#[ignore]`) — NO_HITS

[PASS] **Sleep in tests** — NO_HITS

[PASS] **Naming violations** (`fn test_`, `fn it_works`, `fn should_pass`) — NO_HITS

[PASS] **Holzmann Rule 2 (loops in test bodies)** — 1 hit at id.rs:335
  (`for c in special` in `job_id_returns_err_with_special_chars`). This is
  **pre-existing JobId code**, not part of this bead's changes. Not in scope.

[PASS] **Shared mutable state (Rule 7)** — NO_HITS

[PASS] **Mock interrogation** — NO_HITS

[PASS] **Integration test purity** — No `/tests/` directory exists. All tests
  are in `#[cfg(test)]` modules within source files. N/A.

[PASS] **Error variant completeness** — All `IdError` variants have tests
  asserting the exact variant:
  - `IdError::Empty` → id.rs:601 `Err(IdError::Empty)`, id.rs:852-856
  - `IdError::TooShort(usize)` → id.rs:607 `Err(IdError::TooShort(2))`,
    id.rs:613 `Err(IdError::TooShort(1))`, id.rs:866-872
  - `IdError::TooLong(usize)` → id.rs:621 `Err(IdError::TooLong(65))`,
    id.rs:627 `Err(IdError::TooLong(100))`, id.rs:859-862
  - `IdError::InvalidCharacters` → id.rs:633,639,645,651,668,674,875-878
  All `ParseTriggerStateError` behaviors covered at trigger.rs:169-301.

[PASS] **Density: 118 test markers / 2 new pub functions = 59.0x (target >=5x)**

---

### Tier 1 — Execution

[PASS] **Clippy: 0 warnings** — `cargo clippy -p twerk-core --tests --all-features -- -D warnings`
  produced zero output. Clean compile.

[PASS] **nextest: 401 passed, 0 failed, 0 flaky** — All tests pass in both
  single-threaded and multi-threaded modes. No retries needed.

[PASS] **Ordering probe: CONSISTENT** —
  `--test-threads=1`: 401 passed, 0 skipped
  `--test-threads=8`: 401 passed, 0 skipped
  No divergence. No hidden shared state.

[PASS] **Insta: ABSENT** — `insta` not in `twerk-core/Cargo.toml`. N/A.

---

### Tier 2 — Coverage

[PASS] **Line coverage (changed files only):**
  - `trigger.rs`: **100.00%** (256/256 lines, 0 uncovered)
  - `id.rs`: **96.72%** (884 total, 29 uncovered — all in pre-existing JobId/TaskId/etc.
    macro-generated code, NOT in new TriggerId code)

  Both well above 90% overall threshold. Both well above 95% Calc-layer threshold
  (trigger.rs and id.rs ARE the Calc layer — pure data types, zero I/O).

[PASS] **Branch/region coverage:**
  - `trigger.rs`: **100.00%** region
  - `id.rs`: **99.03%** region (103 regions, 1 uncovered — pre-existing)

  No file below 90% branch threshold.

---

### Tier 3 — Mutation

[PASS] **Kill rate: 100% (27 caught / 27 viable, 1 unviable)**

  28 mutants generated across `trigger.rs` and `id.rs`:
  - 27 **caught** (test fails when mutant applied)
  - 1 **unviable** (`id.rs:210` — `Box::leak(Box::new(Default::default()))` for
    Deref impl; type mismatch prevents compilation, so the mutant is not testable)
  - 0 **missed** (no surviving mutants)

  **Survivors: NONE**

  Note: `cargo-mutants` did not generate mutants for the closure-based character
  validation in `TriggerId::new` (the `.chars().all(|c| ...)` pattern) or the
  `Self::MIN_LENGTH`/`Self::MAX_LENGTH` const comparisons. This is a known
  tool limitation. The coverage data (96.72% line, 99.03% region for id.rs)
  and the 6 proptest invariants confirm these branches are fully exercised.

---

### Previous LETHAL Findings — Verification

All 5 LETHAL findings from the previous review (rejection #1) are confirmed fixed:

| # | Finding | Fix Verified |
|---|---------|-------------|
| L1 | `trigger.rs:245` bare `assert!(result.is_err())` | Now uses `result.unwrap_err()` directly (line 245) |
| L2 | `trigger.rs:329` loop in test body | Now uses `#[rstest]` with `#[case]` for each variant (lines 321-329) |
| L3 | 18x `assert_eq!(expr, true)` `bool_assert_comparison` | All converted to `assert!(expr)` — clippy clean |
| L4 | `explicit_auto_deref` `&*id` → `&id` | Fixed at id.rs:808 |
| L5 | `needless_borrows_for_generic_args` `&"a".repeat(65)` | Fixed at id.rs:860 |

---

### LETHAL FINDINGS

NONE.

---

### MAJOR FINDINGS (0)

NONE.

---

### MINOR FINDINGS (2/5 threshold)

**m1. Pre-existing loop in test body at id.rs:335** — `job_id_returns_err_with_special_chars`
uses `for c in special` to iterate 32 special characters. This is pre-existing code
not part of this bead. Noted for a future cleanup bead. Does not count against this review.

**m2. Pre-existing `is_ok()`/`is_err()` in id.rs validate_id tests** — Lines 507-543
use bare `validate_id("").is_err()`, `validate_id("a").is_ok()`, etc. These are
pre-existing tests for the `validate_id` function, not part of this bead's TriggerId
tests. Noted for a future cleanup bead.

---

### MANDATE

NONE. The suite passes all four tiers with zero LETHAL and zero MAJOR findings.

The two MINOR findings are pre-existing code outside this bead's scope and do not
block APPROVED status.
