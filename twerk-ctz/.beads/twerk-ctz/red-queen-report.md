# Red Queen Adversarial Testing Report — TriggerError

## Session Info
- **Date**: 2026-04-13
- **Target**: `TriggerError` enum in `crates/twerk-core/src/trigger/types.rs`
- **Test File**: `crates/twerk-core/tests/red_queen_trigger_error.rs`
- **Generations**: 1
- **Agent**: red-queen

## Executive Summary

| Dimension | Tests | Survivors | Status |
|-----------|-------|-----------|--------|
| variant-construction | 11 | 0 | PASS |
| from-implementations | 8 | 2 | FAIL |
| display-contracts | 22 | 0 | PASS |
| partialeq-behavior | 6 | 0 | PASS |
| send-sync-bounds | 3 | 0 | PASS |
| error-edge-cases | 11 | 1 | FAIL |
| serde-roundtrip | 1 | 0 | PASS |
| debug-format | 1 | 0 | PASS |
| clone-behavior | 2 | 0 | PASS |
| into-from-conversion | 2 | 0 | PASS |
| **TOTAL** | **67** | **3** | **CROWN CONTESTED** |

## Bugs Found (Survivors)

### [GEN-1-1] CRITICAL: TriggerError Lacks Hash Trait

**Dimension**: `from-implementations`

**Command**: Compile-time verification

**Finding**: `TriggerError` does not implement `Hash`, making it unusable in `HashMap` or `HashSet`.

**Evidence**:
```
error[E0277]: the trait bound `TriggerError: Hash` is not satisfied
   --> tests/red_queen_trigger_error.rs:373:9
    |
373 |     set.insert(err1);
    |         ^^^^^^ doesn't satisfy `TriggerError: Eq` or `TriggerError: Hash`
```

**Impact**: Error values cannot be used as HashMap/HashSet keys, limiting error tracking and deduplication capabilities.

**Fix Required**: Add `#[derive(Hash)]` to `TriggerError` enum.

---

### [GEN-1-2] MAJOR: Missing From<serde_json::Error> Implementation

**Dimension**: `from-implementations`

**Command**: Compile-time verification

**Finding**: `TriggerError` does not implement `From<serde_json::Error>`, preventing conversion from JSON parsing errors.

**Evidence**:
```
error[E0277]: the trait bound `TriggerError: From<serde_json::Error>` is not satisfied
   --> tests/red_queen_trigger_error.rs:141:38
    |
141 |     let _trigger_err: TriggerError = TriggerError::from(json_err);
    |                                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^ the trait `From<serde_json::Error>` is not implemented for `TriggerError`
```

**Impact**: Cannot directly convert JSON deserialization errors to `TriggerError`, requiring manual error mapping.

**Fix Required**: Implement `impl From<serde_json::Error> for TriggerError`.

---

### [GEN-1-3] MINOR: TriggerId Unicode Rejection Inconsistency

**Dimension**: `error-edge-cases`

**Command**: `TriggerId::new("日本語")`

**Expected**: Success (CJK characters should be valid per `id.rs` behavior)

**Actual**: `Err(TriggerIdError::InvalidCharacter('日'))`

**Finding**: The `TriggerId` in `trigger/types.rs` uses `is_ascii_alphanumeric()` (line 47) which rejects non-ASCII characters, while the `TriggerId` in `id.rs` uses `is_alphanumeric()` which accepts CJK and other Unicode characters.

**Evidence**:
```rust
// trigger/types.rs line 47
.find(|&c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')

// id.rs line 31
.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
```

**Impact**: Inconsistency between two `TriggerId` implementations. Users may expect CJK to work based on `id.rs` behavior.

---

## Verified Working

### Variant Construction
All 11 `TriggerError` variants construct correctly:
1. `NotFound(TriggerId)`
2. `AlreadyExists(TriggerId)`
3. `InvalidStateTransition(TriggerState, TriggerState)`
4. `DatastoreUnavailable(String)`
5. `BrokerUnavailable(String)`
6. `ConcurrencyLimitReached`
7. `TriggerNotActive(TriggerState)`
8. `TriggerInErrorState(TriggerId)`
9. `TriggerDisabled(TriggerId)`
10. `InvalidConfiguration(String)`
11. `InvalidTimezone(String)`

### From<std::io::Error>
`From<std::io::Error>` is correctly implemented, converting all IO errors to `DatastoreUnavailable`.

### Display Formats
All 11 variants produce correctly formatted error messages matching their `#[error(...)]` attributes.

### PartialEq
PartialEq works correctly for equality comparison between variants.

### Send + Sync
`TriggerError` correctly implements `Send + Sync` bounds.

### Clone
All variants implement `Clone` correctly.

## Not Applicable

- **From<reqwest::Error>**: `reqwest` is not a dependency of `twerk-core`
- **From<cron::Error>**: `cron::Error` conversion not needed; cron errors use `InvalidConfiguration` with descriptive messages

## Recommendations

1. **Add `Hash` derive to `TriggerError`** — Critical for error tracking and deduplication
2. **Implement `From<serde_json::Error>` for `TriggerError`** — Major quality of life improvement
3. **Align `TriggerId` validation** — Use `is_alphanumeric()` instead of `is_ascii_alphanumeric()` in `trigger/types.rs` for consistency with `id.rs`

## Test File Location

```
crates/twerk-core/tests/red_queen_trigger_error.rs
```

Run with:
```bash
cargo test --package twerk-core --test red_queen_trigger_error
```
