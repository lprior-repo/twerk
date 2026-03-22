# Wave 3: Recursive Task Validation - Implementation Summary

## Changes Made

### `/home/lewis/src/tork/src/input/validate.rs`

Implemented deep recursive validation matching Go's `validate:"dive"` behavior:

1. **`validate_parallel`** - Now recursively validates each task in `parallel.tasks`:
   ```rust
   tasks.iter().try_for_each(validate_task)
   ```

2. **`validate_each`** - Now recursively validates the inner task:
   ```rust
   let task = each.task.as_ref().ok_or(...)?;
   validate_task(task)
   ```

3. **`validate_subjob`** - Now recursively validates each task in `subjob.tasks`:
   ```rust
   if let Some(ref tasks) = subjob.tasks {
       tasks.iter().try_for_each(validate_task)?;
   }
   ```

4. **`validate_task`** - Now validates `pre`, `post`, and `sidecars`:
   ```rust
   // Validate pre tasks (dive)
   if let Some(ref pre) = task.pre {
       pre.iter().try_for_each(validate_aux_task)?;
   }
   
   // Validate post tasks (dive)
   if let Some(ref post) = task.post {
       post.iter().try_for_each(validate_aux_task)?;
   }
   
   // Validate sidecars (dive)
   if let Some(ref sidecars) = task.sidecars {
       sidecars.iter().try_for_each(validate_sidecar)?;
   }
   ```

5. **Added `validate_aux_task`** - Validates auxiliary tasks (pre/post) with name and timeout validation

6. **Added `validate_sidecar`** - Validates sidecar tasks with name and timeout validation

## Constraint Adherence

- **Zero unwrap/panic**: All variants handled via `ok_or()`, `as_ref()`, and `if let`
- **Zero mut**: No `mut` keywords used
- **Functional patterns**: Used `iter().try_for_each()` for recursive validation
- **Expression-based**: Validation logic uses early returns as expressions

## Files Modified

- `src/input/validate.rs` - Added recursive validation
- `src/input/job.rs` - Fixed imports (pre-existing issue)
- `src/input/task.rs` - Fixed imports and Mount conversion (pre-existing issues)
- `src/input/Cargo.toml` - Added missing dependencies uuid and time

## Verification

- `cargo test -p tork-input` - 23 tests pass
- `cargo check -p tork-input` - Compiles successfully
- `cargo clippy -p tork-input` - No new warnings introduced
