# Findings: tw-77a - arrayRange Unbounded Allocation DoS

## Issue
`array_range_fn()` in `crates/twerk-core/src/eval/intrinsics.rs:266` had no upper bound on
elements generated. A call like `arrayRange(0, 999999999, 1)` could allocate ~1 billion
elements, causing memory exhaustion DoS.

## Fix Applied

### 1. Added Upper Bound Constant (line 15)
```rust
const MAX_ARRAY_RANGE_ELEMENTS: usize = 100_000;
```

### 2. Added Bound Check in `array_range_fn`
Before iterating, the function now calculates the expected element count:
```rust
let diff = if step > 0 {
    end.saturating_sub(start)
} else {
    start.saturating_sub(end)
};
let step_abs = step.abs() as usize;
let num_elements = diff / step_abs + if diff % step_abs != 0 { 1 } else { 0 };
if num_elements > MAX_ARRAY_RANGE_ELEMENTS {
    return Err(format!(
        "arrayRange: result would have {} elements, maximum allowed is {}",
        num_elements, MAX_ARRAY_RANGE_ELEMENTS
    ));
}
```

### 3. Optimized Vector Allocation
Changed from `Vec::new()` to `Vec::with_capacity(num_elements)` to avoid
reallocation during push operations.

## Pre-existing Build Blocker
The `twerk-common` crate has a missing `slot` module referenced in `lib.rs:12`:
```rust
pub mod slot;  // File does not exist
```
This prevents `cargo check`/`cargo test` from running. This is a **pre-existing issue**
in the codebase at `origin/main`, not introduced by this fix.

## Files Changed
- `crates/twerk-core/src/eval/intrinsics.rs` - Added bound check

## Verification
Unable to run tests due to pre-existing `twerk-common::slot` module missing.
Code changes are syntactically correct and follow existing patterns.