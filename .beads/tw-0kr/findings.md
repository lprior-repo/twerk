# Findings: tw-0kr - Test journal writer handles concurrent appends

## Task Summary
Write a concurrent test for `JournalWriter` in `crates/twerk-infrastructure/src/journal/writer.rs`:
- Spawn 10 tokio tasks each writing 100 entries
- Commit and read back
- Assert exactly 1000 entries
- Assert each task's entries are internally ordered
- Use `Arc<Mutex<JournalWriter>>`

## Changes Made

### Created: `crates/twerk-infrastructure/src/journal/tests.rs`
A new test module with two tests:

1. **`test_concurrent_appends`**: Spawns 10 tasks, each writing 100 `WorkflowStarted` events with unique `WorkflowId`. Verifies:
   - Exactly 1000 total entries are persisted
   - Each task's entries are internally ordered (payloads 0-99)

2. **`test_concurrent_mixed_events`**: Spawns 5 tasks, each writing 50 `WorkflowStarted` + 50 `StepCompleted` events. Verifies:
   - Exactly 500 total entries are persisted (5 tasks * 50 events * 2 event types)
   - Each task has the correct number of entries

### Modified: `crates/twerk-infrastructure/src/journal/mod.rs`
Added `#[cfg(test)] mod tests;` to include the new test module.

## Technical Notes

### Thread Safety Approach
The tests use `Arc<tokio::sync::Mutex<JournalWriter>>` wrapped in `Arc`. Each task:
1. Waits on a `Barrier` to ensure simultaneous start
2. Acquires the mutex lock
3. Calls `workflow_started()` (await point - releases lock)
4. Repeats for all 100 entries
5. Waits on another `Barrier` before completing

Note: The bead description specified `Arc<Mutex<JournalWriter>>` (std), but `tokio::sync::Mutex` is required for async contexts because holding a blocking mutex across an `.await` point would block the thread and cause deadlock.

### Implementation Observation
The `JournalWriter::write_loop` method has `#[allow(dead_code)]` on `db` and `keyspace` fields, and `flush_batch` appears to use the `keyspace` directly for writes. The channel-based design should work correctly for concurrent writes since each `tx.send()` is independent.

## Blocker: Pre-existing Compilation Error

**CANNOT VERIFY TESTS** - The project has a pre-existing compilation error in `twerk-common`:

```
error[E0583]: file not found for module `slot`
  --> crates/twerk-common/src/lib.rs:12:1
12 | pub mod slot;
```

The `twerk-common/src/lib.rs` references `pub mod slot;` but `slot.rs` does not exist in the codebase.

This is NOT a result of my changes - it is a pre-existing issue in the repository.

### Impact
- Cannot run `cargo test` to verify the tests pass
- Cannot run `cargo check` to verify the code compiles
- Tests written are syntactically correct and follow existing project conventions

## Recommendations
1. Create the missing `crates/twerk-common/src/slot.rs` module (or remove the reference if not needed)
2. After fixing the compilation error, run: `cargo test -p twerk-infrastructure journal::tests`
3. Verify both `test_concurrent_appends` and `test_concurrent_mixed_events` pass