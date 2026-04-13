bead_id: twerk-p4m
bead_title: data: Define TriggerState enum and TriggerId type in twerk-core
phase: state-1.7-test-plan-review-retry1
updated_at: 2026-04-13T19:30:00Z

# Test Plan Review: TriggerState Enum & TriggerId Type (v2 — Retry 1)

**Reviewer:** Test Inquisitor (Mode 1 — Plan Inquisition)
**Inputs:** `contract.md` (state-1-contract), `test-plan.md` (state-1.5-test-planning-retry1)
**Previous review:** `test-plan-review.md` (state-1.7-test-plan-review) — REJECTED (5 LETHAL, 4 MAJOR, 7 MINOR)

---

## Previous Defect Verification

### LETHAL Defects from Previous Review

| # | Previous Finding | Fixed in v2? | Evidence |
|---|---|---|---|
| L1 | test-plan.md:272 — bare `is Err` for TriggerState deserialize rejection | ✅ FIXED | test-plan.md:344 now asserts `result.is_err() == true And: result.unwrap_err().to_string().contains("unknown variant") == true` |
| L2 | test-plan.md:448 — bare `is Err` for TriggerId 2-char serde rejection | ✅ FIXED | test-plan.md:588 asserts `result.is_err() == true And: result.unwrap_err().to_string().contains("too short") == true` |
| L3 | test-plan.md:458 — bare `is Err` for TriggerId 65-char serde rejection | ✅ FIXED | test-plan.md:612 asserts `result.is_err() == true And: result.unwrap_err().to_string().contains("too long") == true` |
| L4 | IdError::Empty/TooLong/InvalidCharacters Display through TriggerId::new() missing | ✅ FIXED | test-plan.md:710-741 adds 4 dedicated BDD scenarios (behaviors 69-72) testing Display messages through TriggerId::new() path |
| L5 | Test density 1.29x (18 tests / 14 functions) | ✅ FIXED | test-plan.md:15 claims 78 unit tests / 14 functions = 5.57x. Counted: 66 BDD scenarios + 6 proptest invariants = 72 named test functions |

### MAJOR Defects from Previous Review

| # | Previous Finding | Fixed in v2? | Evidence |
|---|---|---|---|
| M1 | From<String>/From<&str> trait tests missing | ✅ FIXED | test-plan.md:644-658 adds `trigger_id_from_string_bypasses_validation()` and `trigger_id_from_str_bypasses_validation()` |
| M2 | TriggerState FromStr boundaries (empty, whitespace, prefix, trailing) missing | ✅ FIXED | test-plan.md:278-308 adds 4 BDD scenarios: reject empty string, whitespace-only, prefix "ACTIV", trailing whitespace "ACTIVE " |
| M3 | 64-char max boundary happy path missing | ✅ FIXED | test-plan.md:431-437 adds `trigger_id_new_accepts_exactly_64_chars()` |
| M4 | Whitespace trimming / case preservation tests missing | ✅ FIXED | test-plan.md:527-547 adds `trigger_id_new_rejects_leading_whitespace()`, `trigger_id_new_rejects_trailing_whitespace()`, `trigger_id_new_preserves_mixed_case()` |

### MINOR Defects from Previous Review

| # | Previous Finding | Fixed in v2? | Evidence |
|---|---|---|---|
| m1 | 64-char happy path only in matrix | ✅ FIXED | Now a BDD scenario (behavior 34) |
| m2 | Null byte only in matrix | ✅ FIXED | Now BDD scenario (behavior 45, test-plan.md:511) |
| m3 | 100-char rejection only in matrix | ✅ FIXED | Now BDD scenario (behavior 41, test-plan.md:483) |
| m4 | AsRef/Deref/Borrow/Clone missing | ✅ FIXED | test-plan.md:662-696 adds 4 trait impl tests |
| m5 | Public API surface test missing | ❌ NOT FIXED | No test verifying `twerk_core::TriggerState` resolves |
| m6 | Explicit loop prohibition missing | ✅ FIXED | test-plan.md:1097-1101 explicitly bans copying loop patterns from id.rs |
| m7 | IdError::TooShort Display assertion fragile | ✅ FIXED | test-plan.md:731 now asserts both `contains("too short")` AND `contains("2")` |

---

## VERDICT: APPROVED

---

### Axis 1 — Contract Parity

**[PASS]**

Every `pub fn` and trait impl in the contract signatures has at least one BDD scenario:

| Contract Item | Scenario(s) | Status |
|---|---|---|
| `TriggerState` serde (4 variants) | Behaviors 1-4 | ✅ |
| `TriggerState::default()` | Behavior 5 | ✅ |
| `Display for TriggerState` (4 variants) | Behaviors 6-9 | ✅ |
| `FromStr for TriggerState` (valid, 4 cases) | Behaviors 10-13 | ✅ |
| `FromStr for TriggerState` rejection (unknown) | Behavior 14 | ✅ |
| `FromStr for TriggerState` rejection (empty) | Behavior 15 | ✅ |
| `FromStr for TriggerState` rejection (whitespace) | Behavior 16 | ✅ |
| `FromStr for TriggerState` rejection (prefix) | Behavior 17 | ✅ |
| `FromStr for TriggerState` rejection (trailing) | Behavior 18 | ✅ |
| `TriggerState` JSON deserialization (4 valid) | Behaviors 19-22 | ✅ |
| `TriggerState` JSON deserialization rejection | Behavior 23 | ✅ |
| `Display == serde` roundtrip | Behavior 24 | ✅ |
| `ParseTriggerStateError` Display | Behavior 25 | ✅ |
| `ParseTriggerStateError` `std::error::Error` | Behavior 26 | ✅ |
| `ParseTriggerStateError` PartialEq | Behavior 27 | ✅ |
| `ParseTriggerStateError` Clone | Behavior 28 | ✅ |
| `TriggerState` Copy | Behavior 29 | ✅ |
| `TriggerState` PartialEq/Eq | Behaviors 30-31 | ✅ |
| `TriggerState` Hash | Behavior 32 | ✅ |
| `TriggerId::new()` valid (3, 64, dash, CJK) | Behaviors 33-36 | ✅ |
| `TriggerId::new()` Empty → `IdError::Empty` | Behavior 37 | ✅ |
| `TriggerId::new()` TooShort(1,2) | Behaviors 38-39 | ✅ |
| `TriggerId::new()` TooLong(65,100) | Behaviors 40-41 | ✅ |
| `TriggerId::new()` InvalidChars (@, space, emoji, null) | Behaviors 42-45 | ✅ |
| `TriggerId` input preservation | Behavior 46 | ✅ |
| `TriggerId` rejects whitespace | Behaviors 47-48 | ✅ |
| `TriggerId` preserves mixed case | Behavior 49 | ✅ |
| `TriggerId::as_str()` | Behavior 50 | ✅ |
| `Display for TriggerId` | Behavior 51 | ✅ |
| `TriggerId` serde serialize | Behavior 52 | ✅ |
| `TriggerId` serde deserialize valid | Behavior 53 | ✅ |
| `TriggerId` serde deserialize reject (4 cases) | Behaviors 54-57 | ✅ |
| `TriggerId` Default | Behavior 58 | ✅ |
| `FromStr for TriggerId` (valid + invalid) | Behaviors 59-60 | ✅ |
| `From<String>` for TriggerId (bypass) | Behavior 61 | ✅ |
| `From<&str>` for TriggerId (bypass) | Behavior 62 | ✅ |
| `AsRef<str>` for TriggerId | Behavior 63 | ✅ |
| `Deref` for TriggerId | Behavior 64 | ✅ |
| `Borrow<str>` for TriggerId | Behavior 65 | ✅ |
| `Clone` for TriggerId | Behavior 66 | ✅ |
| `PartialEq` for TriggerId | Behavior 67 | ✅ |
| `Eq+Hash` for TriggerId | Behavior 68 | ✅ |
| `IdError::Empty` Display through TriggerId::new() | Behavior 69 | ✅ |
| `IdError::TooLong` Display through TriggerId::new() | Behavior 70 | ✅ |
| `IdError::TooShort` Display through TriggerId::new() | Behavior 71 | ✅ |
| `IdError::InvalidCharacters` Display through TriggerId::new() | Behavior 72 | ✅ |

**Full parity.** All 14 public functions/traits covered. All 4 error variants (Empty, TooShort, TooLong, InvalidCharacters) have exact variant assertions. ParseTriggerStateError has exact variant assertions.

---

### Axis 2 — Assertion Sharpness

**[PASS]**

Audited every "Then:" clause in all 66 BDD scenarios:

- **TriggerState error assertions:** All 5 FromStr rejection scenarios (14-18) assert `Err(ParseTriggerStateError(String::from("...")))` with exact inner value. ✅
- **TriggerState serde rejection (behavior 23, line 344):** Asserts `result.is_err() == true` AND `result.unwrap_err().to_string().contains("unknown variant") == true`. The `is_err()` is used as a guard before `unwrap_err()` — this is acceptable. The concrete assertion is on the error message content. ✅
- **TriggerId error assertions:** All validation error paths (37-45) assert exact `Err(IdError::ExactVariant(...))` variants. ✅
- **TriggerId serde rejections (54-57):** All assert `result.is_err() == true` AND `result.unwrap_err().to_string().contains("...")` with specific substring. Serde produces `serde_json::Error` (not a domain type), so message content assertion is the correct approach. ✅
- **IdError Display scenarios (69-72):** All assert variant match AND message content. ✅
- **No bare `is_ok()` found.** ✅
- **No bare `is_err()` as sole assertion.** ✅

One minor concern: test-plan.md:578 uses `result.is_ok() == true` as a guard before `result.unwrap().as_str()`. This is functionally equivalent to `assert!(result.is_ok())` but the `And:` clause asserts a concrete value, so the test has a real assertion. **MINOR** — acceptable pattern since the concrete assertion follows immediately.

---

### Axis 3 — Trophy Allocation

**[PASS]**

**Public functions/traits in scope: 14** (verified from contract signatures):
1. `TriggerState::default()`
2. `Display for TriggerState`
3. `FromStr for TriggerState`
4. `TriggerId::new()`
5. `TriggerId::as_str()`
6. `Display for TriggerId`
7. `FromStr for TriggerId`
8. `From<String> for TriggerId`
9. `From<&str> for TriggerId`
10. `AsRef<str> for TriggerId`
11. `Deref for TriggerId`
12. `Borrow<str> for TriggerId`
13. `Display for ParseTriggerStateError`
14. `Display for IdError` (all variants through TriggerId::new())

**Counted test functions:**
- Regular BDD tests: 66 (behaviors 1-72 minus the 6 proptest invariants = 66 named unit test functions)
- Proptest invariants: 6
- Kani harnesses: 2
- Static checks: 6

**Density calculation (unit tests only):** 72 / 14 = **5.14x** (≥ 5x threshold) ✅

Note: The plan claims "78 unit tests" in the summary (line 10). Actual count from BDD section is 72 test functions (66 regular + 6 proptest). The discrepancy of 6 is unexplained but irrelevant — 72/14 = 5.14x still exceeds the threshold. **No defect.**

**Proptest invariants:** 6 specified. `TriggerId::new()` has non-trivial input space — covered by length boundary invariant and character validation invariant. `TriggerState` covered by serde roundtrip and case-insensitivity invariants. **PASS.**

**Fuzz targets:** 2 specified (TriggerId, TriggerState). Noted as deferred pending fuzz infrastructure. Proptest provides equivalent coverage. **ACCEPTABLE.**

**Integration/unit ratio:** 100% unit. Justified — pure data types, zero I/O. **ACCEPTABLE.**

---

### Axis 4 — Boundary Completeness

**[PASS]**

#### TriggerId::new() boundaries

| Boundary | Explicitly named? | Status |
|---|---|---|
| Min valid: 3 chars | ✅ Behavior 33 (`"abc"`) | OK |
| Max valid: 64 chars | ✅ Behavior 34 (`"a".repeat(64)`) | OK |
| Below min: 0 (empty) | ✅ Behavior 37 | OK |
| Below min: 1 char | ✅ Behavior 39 | OK |
| Below min: 2 chars | ✅ Behavior 38 | OK |
| Above max: 65 chars | ✅ Behavior 40 | OK |
| Above max: 100 chars | ✅ Behavior 41 | OK |
| Invalid chars: @ | ✅ Behavior 42 | OK |
| Invalid chars: space | ✅ Behavior 43 | OK |
| Invalid chars: emoji | ✅ Behavior 44 | OK |
| Invalid chars: null byte | ✅ Behavior 45 | OK |
| Whitespace: leading | ✅ Behavior 47 | OK |
| Whitespace: trailing | ✅ Behavior 48 | OK |
| Case preservation | ✅ Behavior 49 | OK |
| CJK alphanumeric | ✅ Behavior 36 | OK |
| Dash + underscore | ✅ Behavior 35 | OK |

**0 missing boundaries.** All explicitly named in BDD scenarios.

#### TriggerState::from_str boundaries

| Boundary | Explicitly named? | Status |
|---|---|---|
| All 4 valid names (lower, upper, mixed) | ✅ Behaviors 10-13 | OK |
| Unknown string | ✅ Behavior 14 | OK |
| Empty string | ✅ Behavior 15 | OK |
| Whitespace-only | ✅ Behavior 16 | OK |
| Prefix of valid name | ✅ Behavior 17 | OK |
| Trailing whitespace | ✅ Behavior 18 | OK |

**0 missing boundaries.**

---

### Axis 5 — Mutation Survivability

**[PASS]**

All 26 critical mutations from the mutation testing checkpoint table (test-plan.md:937-965) have named catching tests. Spot-checking the most dangerous ones:

| Mutation | Catching Test | Status |
|---|---|---|
| `default()` returns `Paused` | `trigger_state_default_returns_active` | ✅ Kill |
| `Display` returns `"active"` (lowercase) | `trigger_state_display_formats_active` | ✅ Kill |
| `FromStr` fails `.to_uppercase()` | `trigger_state_parses_lowercase_active` | ✅ Kill |
| `new()` `< 3` → `< 2` (accepts 2-char) | `trigger_id_new_returns_err_too_short_when_input_is_2_chars` | ✅ Kill |
| `new()` `> 64` → `> 65` (accepts 65-char) | `trigger_id_new_returns_err_too_long_when_input_is_65_chars` | ✅ Kill |
| `new()` `<= 64` → `< 64` (rejects 64-char) | `trigger_id_new_accepts_exactly_64_chars` | ✅ Kill |
| `new()` skips char validation | `trigger_id_new_returns_err_invalid_characters_when_input_has_at_sign` | ✅ Kill |
| `new()` adds `.trim()` | `trigger_id_new_rejects_leading_whitespace` | ✅ Kill |
| `new()` adds `.to_lowercase()` | `trigger_id_new_preserves_mixed_case` | ✅ Kill |
| `Empty` Display omits "empty" | `trigger_id_new_returns_err_empty_displays_correct_message` | ✅ Kill |
| `TooLong` Display omits length | `trigger_id_new_returns_err_too_long_displays_correct_message` | ✅ Kill |
| `TooShort` Display omits length | `trigger_id_new_returns_err_too_short_displays_correct_message` | ✅ Kill |
| `InvalidChars` Display omits "invalid" | `trigger_id_new_returns_err_invalid_chars_displays_correct_message` | ✅ Kill |
| `FromStr for TriggerId` bypasses validation | `trigger_id_from_str_rejects_short_string` | ✅ Kill |
| `FromStr for TriggerState` accepts trailing space | `trigger_state_parse_rejects_trailing_whitespace` | ✅ Kill |
| `FromStr for TriggerState` accepts prefix "ACTIV" | `trigger_state_parse_rejects_prefix_of_valid_name` | ✅ Kill |
| `serde(transparent)` removed | `trigger_id_serializes_as_plain_json_string` | ✅ Kill |
| `Copy` derive removed | `trigger_state_is_copy_and_zero_sized_heap` | ✅ Kill |
| `From<String>` impl removed | `trigger_id_from_string_bypasses_validation` (won't compile) | ✅ Kill |

**0 surviving mutations identified.** The mutation checkpoint table (Section 7) is comprehensive and every entry has a named killer.

---

### Axis 6 — Holzmann Plan Audit

**[PASS]**

- **Rule 2 (Bound every loop):** No loops in any BDD scenario. Implementation note 5 (test-plan.md:1097-1101) explicitly bans copying the loop pattern from id.rs. Proptest used instead. **PASS.**
- **Rule 5 (State assumptions):** Every scenario has explicit `Given` block with concrete values. **PASS.**
- **Rule 6 (Never swallow errors):** No `let _ =` or `.ok()` in any scenario. **PASS.**
- **Rule 7 (Narrow state):** All tests are unit tests with local state. No shared mutable state. **PASS.**
- **Rule 10 (Warnings as errors):** Static check S1 includes clippy. **PASS.**

---

## FINDINGS

### LETHAL FINDINGS (0)

None.

### MAJOR FINDINGS (0)

None.

### MINOR FINDINGS (2/5 threshold — not blocking)

1. **test-plan.md:10 vs actual count** — Summary claims "78 unit tests" but actual count of named test functions in the BDD section is 72 (66 regular + 6 proptest). The discrepancy of 6 is unexplained. If the test writer is padding the count, that's dishonest. If it's a counting error, it's sloppy. Either way, the actual count (72/14 = 5.14x) still meets threshold, so this is cosmetic. **MINOR.**

2. **test-plan.md — No public API surface test** — Previous review MINOR defect m5 not fixed. No test verifies that `twerk_core::TriggerState` resolves via the `pub use` in `lib.rs`. Module registration failure would be a compile error caught by `cargo check`, so static analysis catches this. **MINOR.**

---

## MANDATE

**None.** The test plan is approved for implementation.

### Post-approval reminders for the test writer:

1. **test-plan.md:1086-1088** — `proptest` must be added to `twerk-core/Cargo.toml` dev-dependencies before writing proptest tests.
2. **test-plan.md:1097-1101** — DO NOT copy loops from `id.rs`. Use proptest for combinatorial character validation.
3. **test-plan.md:1106-1109** — `TriggerId` is hand-written (no `define_id!` macro). All trait impls must be explicit.
4. **test-plan.md:1112-1114** — Serde deserialization error assertions must use message content matching (not variant matching, since serde produces `serde_json::Error`).
5. **Mode 2 review will re-audit from Tier 0** after implementation. All banned patterns, density, and mutation gates apply to the actual test code.
