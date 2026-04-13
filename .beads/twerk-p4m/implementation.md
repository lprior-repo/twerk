bead_id: twerk-p4m
bead_title: data: Define TriggerState enum and TriggerId type in twerk-core
phase: state-3-implementation
updated_at: 2026-04-13T23:45:00Z

# Implementation Summary

## Changed Files

| File | Change |
|------|--------|
| `crates/twerk-core/src/trigger.rs` | Replaced stub `Display` and `FromStr` impls with real implementations |
| `crates/twerk-core/src/id.rs` | Implemented `TriggerId::new()` validation; custom `Deserialize` impl; fixed `IdError::TooLong` Display message; fixed proptest strategy regex bug |

## TriggerState Implementation (`trigger.rs`)

### Display
- Pattern match on all 4 variants, writing the `SCREAMING_SNAKE_CASE` name via `f.write_str(name)`.
- Expression-based: each arm returns the exact string expected by tests.

### FromStr
- Converts input to uppercase via `s.to_uppercase()` then pattern-matches against `"ACTIVE"`, `"PAUSED"`, `"DISABLED"`, `"ERROR"`.
- All other inputs (including empty, whitespace, partial matches, trailing whitespace) fall through to `Err(ParseTriggerStateError(s.to_string()))`.
- Case-insensitive per contract [INV-TS-5].

### Pre-existing (unchanged)
- Enum derives: `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default`.
- `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` — serde serialization handled by derive.
- `#[default]` on `Active` — Default handled by derive.
- `ParseTriggerStateError` — Display, Error, Clone, PartialEq all already correct.

## TriggerId Implementation (`id.rs`)

### TriggerId::new() — Validation Logic
Validation order per contract specification:
1. Empty check → `IdError::Empty`
2. Too short check (`len < 3`) → `IdError::TooShort(len)`
3. Too long check (`len > 64`) → `IdError::TooLong(len)`
4. Character validation → `IdError::InvalidCharacters`

Constants `MIN_LENGTH = 3` and `MAX_LENGTH = 64` are private associated constants on `TriggerId`.

All checks are pure (no I/O, no mutation). Input string is stored exactly as provided (no trim, no case mutation).

### Custom Deserialize Implementation
`TriggerId` cannot use derive `Deserialize` with `#[serde(transparent)]` because transparent deserialization bypasses validation. A hand-written `impl<'de> Deserialize<'de> for TriggerId` was added that:
1. Deserializes the inner `String` from the serde data format.
2. Calls `TriggerId::new(s)` to validate.
3. Maps `IdError` to `serde::de::Error::custom(err)` on failure.

This ensures serde deserialization rejects invalid values (empty, too short, too long, invalid chars) — satisfying test-plan behaviors 54-57.

### IdError::TooShort(usize) — New Variant
Already present in the stub (added during test-writing phase). The `thiserror` attribute is:
```rust
#[error("ID is too short: {0} characters (minimum 3)")]
TooShort(usize),
```

### IdError::TooLong Display Message Fix
Changed from `"ID exceeds maximum length of {MAX_ID_LENGTH} characters: {0} characters"` to `"ID is too long: {0} characters (maximum {MAX_ID_LENGTH})"` to include the substring "too long" required by test-plan behavior 70 and the serde rejection tests (behaviors 54-57).

### Pre-existing (unchanged)
- `From<String>` and `From<&str>` — infallible, bypass validation (per contract [P-TI-4]).
- `Display`, `AsRef<str>`, `Deref`, `Borrow<str>` — transparent delegation to inner `String`.
- `FromStr` — delegates to `Self::new(s)`.
- `Default` — derives `Default`, yielding empty string (per contract [INV-TI-5]).
- `Serialize` — transparent, derives normally.

## Proptest Strategy Fix
Fixed `proptest_trigger_id_rejects_invalid_chars` test strategy: changed from a bare regex string (which had unescaped `[]` causing `ClassUnclosed` parse error) to `proptest::sample::select(vec![...])` with individual char literals. This is a test infrastructure fix (invalid regex), not a behavioral change.

## Constraint Adherence

| Constraint | Status |
|------------|--------|
| Data → Calc → Actions | All validation logic is pure calculation (no I/O, no mutation). |
| Zero `mut` in source | No `let mut` in implementation code. |
| Zero `unwrap`/`expect`/`panic!` in source | None in non-test code. |
| Make illegal states unrepresentable | `TriggerState` is an enum; `TriggerId` validates at construction boundary. |
| Expression-based | All impl bodies use match/early-return expressions. |
| Clippy clean | Passes `cargo clippy` with strict lints. |

## Test Results

```
397 tests run: 397 passed, 0 skipped
```
