# TriggerRegistry (twerk-3y3) — Defects Found

## Status: REJECTED

The implementation fails 3 hard constraints and requires rewrite before approval.

---

## CRITICAL DEFECTS

### DEFECT-1: Production `.unwrap()` in `fire()` (Line 528)

**Location:** `crates/twerk-core/src/trigger.rs:528`

**Code:**
```rust
let _permit = permit.unwrap();
```

**Severity:** CRITICAL — Panic vector in production code

**Problem:** The semaphore `acquire()` returns `Result<SemaphorePermit, ...>`. While the code checks `permit.is_err()` to handle the closed semaphore case, it then unwraps the permit unconditionally. If the improbable occurs and the semaphore is closed between the check and unwrap, this panics.

**Fix Required:** Replace with proper error handling:
```rust
let _permit = permit.map_err(|_| TriggerError::ConcurrencyLimitReached)?;
```

---

### DEFECT-2: `fire()` exceeds function length limit (Line 510-560)

**Location:** `crates/twerk-core/src/trigger.rs:510-560`

**Function Length:** 50 lines (HARD LIMIT: 25 lines)

**Severity:** CRITICAL — Violates Farley Engineering Constraint

**Problem:** The `fire()` async function is exactly 2x the allowed length. It handles: datastore availability, broker availability, concurrency limiting, trigger lookup, and state validation all in one function.

**Fix Required:** Decompose into smaller functions:
- Extract trigger validation into `validate_trigger_for_fire()`
- Extract concurrency permit acquisition into separate step
- Keep the orchestration minimal

---

### DEFECT-3: `is_valid_transition()` exceeds function length limit (Line 564-592)

**Location:** `crates/twerk-core/src/trigger.rs:564-592`

**Function Length:** 28 lines (HARD LIMIT: 25 lines)

**Severity:** MODERATE — Pure function but still violates constraint

**Problem:** The state transition matrix match expression is verbose and exceeds the limit.

**Fix Required:** Reduce verbosity or extract transition table. Acceptable borderline case for pure state machine logic.

---

## MINOR DEFECTS

### DEFECT-4: Unnecessary `mut` on RwLock guard (Line 402)

**Location:** `crates/twerk-core/src/trigger.rs:402`

**Code:**
```rust
let mut triggers = self.triggers.write();
```

**Problem:** `RwLock::write()` returns a guard that doesn't require `mut` for basic operations. The `mut` is unnecessary.

**Fix:** Remove `mut`.

---

## SUMMARY

| Defect | Location | Type | Severity |
|--------|----------|------|----------|
| 1 | Line 528 | Panic vector (.unwrap()) | CRITICAL |
| 2 | Lines 510-560 | fire() 50 lines | CRITICAL |
| 3 | Lines 564-592 | is_valid_transition() 28 lines | MODERATE |
| 4 | Line 402 | Unnecessary mut | MINOR |

## REQUIRED ACTIONS

1. Fix line 528 with proper error handling
2. Decompose `fire()` into smaller functions
3. Simplify `is_valid_transition()` or accept borderline status
4. Remove unnecessary `mut` on line 402

**After fixes, re-submit for review.**
