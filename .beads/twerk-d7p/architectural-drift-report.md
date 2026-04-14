# Architectural Drift Report: twerk-d7p/crates/twerk-core/src/types.rs

**Date:** Mon Apr 13 2026  
**Agent:** Architectural Drift Agent  
**Status:** REFACTORED

---

## Executive Summary

The `types.rs` file (445 lines) exceeded the 300-line limit and required splitting into separate module files. All types have been extracted into individual files within a `types/` directory module.

---

## Findings

### 1. Line Count Violation

| File | Lines | Status |
|------|-------|--------|
| `types.rs` (original) | 445 | ❌ EXCEEDED 300 |
| `types/mod.rs` | 18 | ✅ |
| `types/port.rs` | 98 | ✅ |
| `types/retry_limit.rs` | 74 | ✅ |
| `types/retry_attempt.rs` | 65 | ✅ |
| `types/progress.rs` | 89 | ✅ |
| `types/task_count.rs` | 73 | ✅ |
| `types/task_position.rs` | 66 | ✅ |

### 2. Scott Wlaschin DDD Analysis

#### Primitive Obsession: ✅ GOOD
All domain primitives are properly wrapped in newtypes:
- `Port(u16)` - validated 1-65535 range
- `RetryLimit(u32)` - no upper bound restriction
- `RetryAttempt(u32)` - represents current attempt number
- `Progress(f64)` - validated 0.0-100.0 range, NaN rejected
- `TaskCount(u32)` - no upper bound restriction
- `TaskPosition(i64)` - supports negative for relative offsets

#### Type Encapsulation: ✅ GOOD
Each type has:
- Private inner field
- Smart constructor with validation (`new()` returning `Result`)
- Accessor method (`.value()`)
- `Deref` implementation for ergonomic access
- `AsRef` for generic bounds
- `From<T>` for conversions
- `Display` implementation
- Serde support (`Serialize`, `Deserialize`)
- Error types with `thiserror`

#### Design Issues Identified (Non-Blocking)

1. **Unused Error Variants**: `RetryAttemptError::Invalid` and `TaskPositionError::Invalid` are defined but their constructors (`new()`) always succeed since u32/i64 have no invalid bit patterns. This is a mild anti-pattern but not blocking.

2. **Validation-in-Constructor Pattern**: `RetryLimit::new()` and `TaskCount::new()` always succeed (u32 is always non-negative), but they return `Result` for API consistency. The actual validation happens in `from_option()`. This is acceptable for API uniformity.

---

## Refactoring Performed

### Before
```
crates/twerk-core/src/types.rs (445 lines - monolithic)
```

### After
```
crates/twerk-core/src/types/
├── mod.rs           (18 lines)  - Module re-exports
├── port.rs          (98 lines)  - Port type
├── retry_limit.rs   (74 lines)  - RetryLimit type  
├── retry_attempt.rs (65 lines)  - RetryAttempt type
├── progress.rs      (89 lines)  - Progress type
├── task_count.rs    (73 lines)  - TaskCount type
└── task_position.rs  (66 lines)  - TaskPosition type
```

### Files Created
- `types/port.rs`
- `types/retry_limit.rs`
- `types/retry_attempt.rs`
- `types/progress.rs`
- `types/task_count.rs`
- `types/task_position.rs`
- `types/mod.rs`

### Files Removed
- `types.rs` (replaced by `types/` directory module)

### Files Modified
- `lib.rs` - unchanged, still uses `pub mod types;` which now resolves to `types/mod.rs`

---

## Verification

```bash
$ cargo check -p twerk-core --lib
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
```

---

## Recommendations

1. **Consider simplifying error types** for `RetryAttempt` and `TaskPosition` - their `new()` functions always succeed, making the error types unnecessary unless used by `from_option()` equivalents.

2. **Pre-existing test issues** in `trigger/tests.rs`, `trigger_registry_test.rs`, and `red_queen_trigger_error.rs` are unrelated to this refactoring. These tests have compilation errors due to:
   - `TriggerId` constructor being private
   - `TriggerError` variants not matching test expectations

---

## Conclusion

**STATUS: REFACTORED**

The `types.rs` file has been successfully split into separate module files, each under 300 lines. The refactoring maintains full API compatibility and the library compiles successfully.
