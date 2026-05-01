# Findings: tw-slbc - Test Engine::submit_task

## Bead Task
Write test for `Engine::submit_task` in `crates/twerk-app/src/engine/mod.rs`.

## Findings

### Existing Tests Verified
All 4 tests in `crates/twerk-app/tests/engine_submit_task_test.rs` pass:

1. **`engine_submit_task_returns_error_when_engine_not_running`** - Verifies `SubmitTaskError::NotRunning` is returned when engine is not running

2. **`engine_submit_task_returns_valid_task_handle`** - Creates engine, starts it, submits task, asserts returned `TaskHandle` has valid non-empty task_id

3. **`engine_submit_task_appears_in_pending_queue`** - Submits task to named queue, verifies queue info shows size >= 1

4. **`engine_submit_task_rejects_duplicate_task_id`** - Submits two tasks with same ID, asserts second is rejected with `SubmitTaskError::DuplicateTaskId`

### Implementation Analysis
`Engine::submit_task` is in `engine_registration.rs:213-239`:
- Returns `Err(SubmitTaskError::NotRunning)` if engine state != Running
- Returns `Err(SubmitTaskError::DuplicateTaskId(id))` if task ID already in `submitted_tasks` HashSet
- Uses `submitted_tasks: HashSet<TaskId>` (engine.rs:41) for deduplication
- Publishes to broker on success

### Discrepancy Note (from sentinel close)
Sentinel noted: "discrepancy between spec (rejected) and implementation (idempotent)"

**Analysis**: The implementation correctly REJECTS duplicates at the engine level via `submitted_tasks` HashSet. The behavior is NOT idempotent for duplicate task IDs - it properly returns an error. This appears to be a correct implementation.

### Conclusion
Tests already exist and correctly verify the specified behavior. No code changes required.
