# Defects Report: twerk-d7p types.rs

**Reviewer**: black-hat-reviewer  
**Date**: 2026-04-13  
**Files Reviewed**: `crates/twerk-core/src/types.rs`  
**Red Queen Report**: `.beads/twerk-d7p/red-queen-report.md`  
**Verdict**: **REJECTED — 5 Phase Review Failed**

---

## Executive Summary

The Red Queen adversarial testing correctly identified **3 critical bugs**. The black-hat review confirms all 5 phases fail. The implementation violates contract.md's explicit serialization contract.

**Critical Defects:**
1. Port deserialization accepts `0` (should reject)
2. Progress deserialization accepts `-0.001` (should reject)
3. Progress deserialization accepts `100.001` (should reject)

**Root Cause**: `#[serde(transparent)]` + `From<T>` implementations bypass validating constructors.

---

## Phase 1: Contract Parity — FAIL

### Contract Violations

**contract.md line 41-42** (Port Serialization Contract):
> Deserialize accepts raw `u16` and validates via constructor

**contract.md line 142-143** (Progress Serialization Contract):
> Deserialize accepts raw `f64` and validates via constructor

### Violations Found

#### [D1] Port From<u16> Bypasses Validation
**Location**: `types.rs:76-81`

```rust
impl From<u16> for Port {
    fn from(value: u16) -> Self {
        // Called after validation - internal use only
        Self(value)  // NO VALIDATION!
    }
}
```

**Problem**: The comment is FALSE. Serde's `#[serde(transparent)]` uses `From::from()` directly during deserialization. It does NOT call `Port::new()`.

**Contract says**: "Deserialize validates via constructor"  
**Implementation does**: Deserialize bypasses constructor via `From`

#### [D2] Progress From<f64> Bypasses Validation
**Location**: `types.rs:293-297`

```rust
impl From<f64> for Progress {
    fn from(value: f64) -> Self {
        Self(value)  // NO VALIDATION!
    }
}
```

**Problem**: Same as Port. Serde uses `From::from()` directly, bypassing `Progress::new()` which validates NaN and range 0.0..=100.0.

---

## Phase 2: Farley Constraints — PASS

- All functions < 25 lines ✓
- No function > 5 parameters ✓
- No I/O in calculations ✓

---

## Phase 3: Functional Rust (Big 6) — FAIL

### Parse, Don't Validate — VIOLATED

The `From<T>` implementations do not parse. They directly construct without validation. This is the **opposite** of Parse, Don't Validate.

**Correct pattern** (what `FromStr` does at lines 83-94):
```rust
impl FromStr for Port {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: u16 = s.parse().map_err(|_| ...)?;
        Self::new(value)  // ✅ Validates!
    }
}
```

**Broken pattern** (what `From<u16>` does at lines 76-81):
```rust
impl From<u16> for Port {
    fn from(value: u16) -> Self {
        Self(value)  // ❌ No validation!
    }
}
```

---

## Phase 4: Ruthless Simplicity & DDD — PASS

- No `unwrap()`, `expect()`, `panic!()` in newtype code ✓
- No `let mut` in public APIs ✓
- Error types properly defined ✓

---

## Phase 5: Bitter Truth — FAIL

### The Comment Lies (Line 78)

```rust
// Called after validation - internal use only
Self(value)
```

**This comment is deceptive.** Nothing in the type system ensures this `From` is only called after validation. Serde calls it directly during deserialization of untrusted input.

### The Sniff Test

The code **fails the sniff test**. It looks like a developer who:
1. Read about newtypes and validation
2. Implemented `new()` with validation
3. Added `From<T>` for "convenience"
4. Added `#[serde(transparent)]`
5. Did NOT understand that `#[serde(transparent)]` + `From<T>` = bypass

This is junior-developer clever, not senior-developer honest.

---

## Required Fixes

1. **Remove the deceptive `From` implementations** for `Port` and `Progress`, OR
2. **Implement custom serde** with `deserialize_with` that calls the validating constructor

The Red Queen report correctly recommends:
> 1. Remove `From<u16>` for `Port` — Force use of `Port::new()` which validates
> 2. Remove `From<f64>` for `Progress` — Force use of `Progress::new()` which validates  
> 3. Implement custom serde with `deserialize_with` to call the validating constructor

---

## Impact Assessment

| Severity | Description |
|----------|-------------|
| CRITICAL | Malformed JSON can create `Port(0)` which crashes network operations |
| CRITICAL | Malformed JSON can create `Progress(-0.001)` or `Progress(100.001)` causing downstream calculation errors |
| HIGH | Contract explicitly promises validation on deserialize — promise broken |

---

## Verdict

**STATUS: REJECTED**

The code must be rewritten. The serialization contract is broken. Invalid states are representable. The contract explicitly says "Deserialize validates via constructor" but the implementation bypasses the constructor entirely.

---

*Review conducted by black-hat-reviewer per AGENTS.md mandate*
