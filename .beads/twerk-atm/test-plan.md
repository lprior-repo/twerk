# Martin Fowler Test Plan: Graceful Shutdown Implementation

## Overview

This test plan covers exhaustive testing for the `stop()` method implementation across all runtime adapters (Shell, Podman, Docker, Mock). Tests follow Martin Fowler's principles: expressive names, Given-When-Then structure, and contract-first verification.

---

## Happy Path Tests

### ShellRuntimeAdapter

#### test_shell_stop_terminates_running_process_gracefully
**Given:** A ShellRuntimeAdapter with a running shell process
**And:** The process is executing a long-running command (e.g., `sleep 100`)
**And:** The process ID is tracked in active_processes map
**When:** `stop()` is called with the task containing the tracked PID
**Then:**
- The process receives SIGTERM signal
- Process exits within graceful timeout (30s)
- `stop()` returns `Ok(ExitCode(0))`
- Process ID is removed from active_processes map
- Temporary script file is deleted

#### test_shell_stop_handles_already_completed_process
**Given:** A ShellRuntimeAdapter with a process that has already completed
**And:** The process ID is tracked in active_processes map
**When:** `stop()` is called with the task
**Then:**
- `stop()` returns `Ok(ExitCode(<actual_exit_code>))`
- No error is raised for already-completed process
- Process ID is removed from active_processes map

#### test_shell_stop_is_idempotent
**Given:** A ShellRuntimeAdapter with a stopped process
**And:** The process ID was removed from active_processes map
**When:** `stop()` is called multiple times with the same task
**Then:**
- First call returns `Ok(ExitCode(...))`
- Subsequent calls return `Ok(ExitCode(0))` (idempotent)
- No errors or panics occur

### PodmanRuntimeAdapter

#### test_podman_stop_stops_container_gracefully
**Given:** A PodmanRuntimeAdapter with a running container
**And:** The container is executing a long-running command
**And:** The container ID is tracked in active_containers map
**When:** `stop()` is called with the task containing the tracked container ID
**Then:**
- Container receives stop signal (SIGTERM)
- Container exits within graceful timeout (30s)
- `stop()` returns `Ok(ExitCode(0))`
- Container ID is removed from active_containers map
- Container is removed from podman

#### test_podman_stop_with_volumes_cleanup
**Given:** A PodmanRuntimeAdapter with a container mounted with volumes
**And:** The container is running
**When:** `stop()` is called
**Then:**
- Container is stopped gracefully
- All mounted volumes are unmounted
- Container is removed
- `stop()` returns `Ok(ExitCode(0))`

#### test_podman_stop_handles_nonexistent_container
**Given:** A PodmanRuntimeAdapter
**When:** `stop()` is called with a task containing a container ID that doesn't exist
**Then:**
- `stop()` returns `Ok(ExitCode(0))` (idempotent)
- No error is raised for missing container
- No panics or crashes occur

### MockRuntime

#### test_mock_stop_is_noop
**Given:** A MockRuntime instance
**When:** `stop()` is called with any task
**Then:**
- `stop()` returns `Ok(ExitCode(0))`
- No operations are performed
- No side effects occur

---

## Error Path Tests

### Precondition Violations

#### test_shell_stop_rejects_task_not_running_created
**Given:** A ShellRuntimeAdapter
**And:** A task in CREATED state
**When:** `stop()` is called with the task
**Then:**
- `stop()` returns `Err(ShutdownError::TaskNotRunning("CREATED"))`
- No process termination occurs

#### test_shell_stop_rejects_task_not_running_completed
**Given:** A ShellRuntimeAdapter
**And:** A task in COMPLETED state
**When:** `stop()` is called with the task
**Then:**
- `stop()` returns `Err(ShutdownError::TaskNotRunning("COMPLETED"))`
- No process termination occurs

#### test_shell_stop_rejects_empty_task_id
**Given:** A ShellRuntimeAdapter with active_processes map
**When:** `stop()` is called with a task having empty ID
**Then:**
- `stop()` returns `Err(ShutdownError::InvalidTaskId(""))`
- No state modifications occur

#### test_shell_stop_rejects_untracked_task
**Given:** A ShellRuntimeAdapter with an empty active_processes map
**When:** `stop()` is called with a task containing an untracked PID
**Then:**
- `stop()` returns `Err(ShutdownError::ProcessNotFound("<task_id>"))`
- No termination attempt is made

### Timeout Scenarios

#### test_shell_stop_times_out_and_forces_kill
**Given:** A ShellRuntimeAdapter
**And:** A process that does not respond to SIGTERM (e.g., stuck process)
**And:** Graceful timeout is set to 2 seconds
**When:** `stop()` is called with the task
**Then:**
- SIGTERM is sent to process
- After 2s, SIGKILL is sent
- `stop()` returns `Ok(ExitCode(-9))` (SIGKILL exit code)
- Process is terminated

#### test_podman_stop_times_out_and_forces_kill
**Given:** A PodmanRuntimeAdapter
**And:** A container that does not respond to stop signal
**And:** Graceful timeout is set to 2 seconds
**When:** `stop()` is called with the task
**Then:**
- Container stop signal is sent
- After 2s, container is force-removed
- `stop()` returns `Ok(ExitCode(-9))`
- Container is removed from podman

#### test_shell_stop_respects_custom_timeout
**Given:** A ShellRuntimeAdapter
**And:** Graceful timeout is set to 5 seconds via env var
**And:** A process that exits in 3 seconds
**When:** `stop()` is called with the task
**Then:**
- SIGTERM is sent to process
- Process exits within 3s (before timeout)
- `stop()` returns `Ok(ExitCode(0))`
- No force kill occurs

### Signal Errors

#### test_shell_stop_handles_signal_error
**Given:** A ShellRuntimeAdapter
**And:** A process that cannot receive signals (kernel error)
**When:** `stop()` is called with the task
**Then:**
- `stop()` returns `Err(ShutdownError::SignalError("failed to send signal"))`
- Process state is preserved

#### test_shell_stop_handles_kill_error
**Given:** A ShellRuntimeAdapter
**And:** A process that cannot be killed (permission denied)
**When:** `stop()` is called with the task
**Then:**
- `stop()` returns `Err(ShutdownError::TerminationFailed("permission denied"))`
- Error message is descriptive

### Cleanup Errors

#### test_shell_stop_handles_cleanup_failure
**Given:** A ShellRuntimeAdapter
**And:** A process that terminated successfully
**And:** Temporary script file is read-only (cannot be deleted)
**When:** `stop()` is called with the task
**Then:**
- `stop()` returns `Err(ShutdownError::CleanupFailed("permission denied"))`
- Process is terminated despite cleanup failure
- Exit code is still recorded

#### test_podman_stop_handles_volume_unmount_failure
**Given:** A PodmanRuntimeAdapter
**And:** A container with mounted volumes
**And:** One volume is busy (cannot be unmounted)
**When:** `stop()` is called with the task
**Then:**
- `stop()` returns `Err(ShutdownError::CleanupFailed("volume busy"))`
- Container is stopped
- Attempt to unmount remaining volumes

---

## Edge Case Tests

### Boundary Values

#### test_shell_stop_handles_zero_timeout
**Given:** A ShellRuntimeAdapter
**And:** Graceful timeout is set to 0 seconds
**When:** `stop()` is called with a running task
**Then:**
- Process is immediately killed (no grace period)
- `stop()` returns `Ok(ExitCode(-9))`

#### test_shell_stop_handles_maximum_timeout
**Given:** A ShellRuntimeAdapter
**And:** Graceful timeout is set to 3600 seconds (1 hour)
**And:** A process that exits in 1 second
**When:** `stop()` is called with the task
**Then:**
- Process exits within 1s
- `stop()` returns `Ok(ExitCode(0))`
- No unnecessary waiting

### Concurrent Access

#### test_shell_stop_concurrent_calls_thread_safe
**Given:** A ShellRuntimeAdapter with a running process
**When:** 10 threads simultaneously call `stop()` with the same task
**Then:**
- Only one termination occurs
- First call returns actual exit code
- Subsequent calls return `Ok(ExitCode(0))`
- No race conditions or panics
- active_processes map is consistently updated

#### test_shell_stop_concurrent_run_and_stop
**Given:** A ShellRuntimeAdapter
**When:** `run()` and `stop()` are called concurrently with the same task
**Then:**
- No panics or data races
- Either process is stopped or run completes
- State is consistent

### Empty/Null States

#### test_shell_stop_handles_null_env_vars
**Given:** A ShellRuntimeAdapter with null environment variables
**When:** `stop()` is called with a task
**Then:**
- `stop()` completes without errors
- Null handling is graceful

#### test_shell_stop_handles_empty_command
**Given:** A ShellRuntimeAdapter with empty command array
**When:** `stop()` is called with a task
**Then:**
- `stop()` completes without errors
- No assumptions about command existence

### State Transitions

#### test_shell_stop_updates_task_state_to_stopped
**Given:** A ShellRuntimeAdapter
**And:** A task in RUNNING state
**When:** `stop()` is called and succeeds
**Then:**
- Task state is updated to STOPPED
- State transition is recorded

#### test_podman_stop_updates_task_state_to_stopped
**Given:** A PodmanRuntimeAdapter
**And:** A task in RUNNING state
**When:** `stop()` is called and succeeds
**Then:**
- Task state is updated to STOPPED
- State transition is recorded

---

## Contract Verification Tests

### Precondition Verification

#### test_precondition_task_must_be_running
**Given:** Tasks in various states (CREATED, PENDING, SCHEDULED, RUNNING, STOPPED, COMPLETED, FAILED)
**When:** `stop()` is called for each task
**Then:**
- RUNNING task: `stop()` proceeds
- Non-RUNNING tasks: `stop()` returns `Err(ShutdownError::TaskNotRunning(...))`

#### test_precondition_process_must_be_tracked
**Given:** A ShellRuntimeAdapter with tracked process P1
**When:** `stop()` is called for P1 and for untracked process P2
**Then:**
- P1: `stop()` proceeds
- P2: `stop()` returns `Err(ShutdownError::ProcessNotFound(...))`

#### test_precondition_task_id_must_be_valid
**Given:** Tasks with IDs ("", "valid-id", "123", "task-abc-123")
**When:** `stop()` is called for each task
**Then:**
- Empty ID: `stop()` returns `Err(ShutdownError::InvalidTaskId(""))`
- Valid IDs: `stop()` proceeds (if process tracked)

### Postcondition Verification

#### test_postcondition_process_terminated
**Given:** A ShellRuntimeAdapter with a running process P1
**When:** `stop()` is called for P1
**And:** `stop()` completes successfully
**Then:**
- Process P1 no longer exists in process table
- `ps -p <pid>` returns error
- Process state is `defunct` or non-existent

#### test_postcondition_state_cleaned_up
**Given:** A ShellRuntimeAdapter with process P1 in active_processes
**When:** `stop()` is called for P1
**And:** `stop()` completes successfully
**Then:**
- P1 is removed from active_processes map
- `active_processes.contains_key(P1)` returns `false`

#### test_postcondition_resources_cleaned_up
**Given:** A ShellRuntimeAdapter that creates temp files
**When:** `stop()` is called for a task
**And:** `stop()` completes successfully
**Then:**
- Temporary script file is deleted
- No temp files remain in /tmp
- Disk space is freed

#### test_postcondition_task_state_updated
**Given:** A task in RUNNING state
**When:** `stop()` is called and succeeds
**Then:**
- Task state field equals "STOPPED"
- State transition is recorded in audit log (if applicable)

### Invariant Verification

#### test_invariant_no_zombie_processes
**Given:** A ShellRuntimeAdapter with multiple running processes
**When:** `stop()` is called for each process
**And:** All `stop()` calls complete
**Then:**
- No zombie processes exist in process table
- All processes are in `exited` state
- Parent process count is zero

#### test_invariant_timeout_enforced
**Given:** A ShellRuntimeAdapter with graceful timeout = 1s
**And:** A process that never exits
**When:** `stop()` is called for the process
**Then:**
- `stop()` completes within 1s + overhead
- Process is killed after timeout
- `stop()` does not hang indefinitely

#### test_invariant_signal_order_preserved
**Given:** A ShellRuntimeAdapter
**And:** A process that logs signals received
**When:** `stop()` is called for the process
**Then:**
- SIGTERM is sent first
- SIGKILL is only sent after timeout
- Signal order is logged and verifiable

---

## Integration Tests

### End-to-End Scenarios

#### test_e2e_shell_runtime_full_lifecycle
**Given:** A ShellRuntimeAdapter
**When:** Task is created with `run()` command
**And:** Task is started via `run()`
**And:** Task is stopped via `stop()`
**Then:**
- Task transitions: CREATED → RUNNING → STOPPED
- Process is started and terminated correctly
- Exit code is recorded
- Resources are cleaned up

#### test_e2e_podman_runtime_full_lifecycle
**Given:** A PodmanRuntimeAdapter
**When:** Task is created with container image
**And:** Task is started via `run()`
**And:** Task is stopped via `stop()`
**Then:**
- Container is created and started
- Container is stopped and removed
- Task transitions: CREATED → RUNNING → STOPPED
- Volumes are cleaned up

#### test_e2e_concurrent_tasks_isolation
**Given:** A ShellRuntimeAdapter
**When:** 5 tasks are started concurrently
**And:** Each task runs independently
**And:** `stop()` is called for task #3
**Then:**
- Only task #3 is stopped
- Other tasks continue running
- No cross-task interference

### Real-World Scenarios

#### test_realworld_user_interrupts_long_running_task
**Given:** A user is executing a long-running task (10 minute job)
**And:** User decides to cancel after 2 minutes
**When:** User calls `stop()` on the task
**Then:**
- Task is terminated within timeout
- User receives exit code
- Resources are released

#### test_realworld_system_shutdown_graceful
**Given:** System is shutting down
**And:** 10 tasks are running
**When:** System initiates graceful shutdown
**And:** `stop()` is called for each task
**Then:**
- All tasks are stopped
- Each task receives SIGTERM
- Tasks exit gracefully or are force-killed
- System shutdown completes without hanging

#### test_realworld_resource_constraints
**Given:** System with low memory
**When:** `stop()` is called on a memory-intensive task
**Then:**
- Task is terminated
- Memory is released
- System stability is maintained

---

## Mutation Tests

### Mutation Operators

#### test_mutation_stop_returns_ok_unchanged
**Mutant:** `stop()` returns `Ok(())` without termination
**Kill test:** `test_shell_stop_terminates_running_process_gracefully`
**Expected:** Test fails because process is still running

#### test_mutation_stop_no_cleanup
**Mutant:** `stop()` terminates but skips cleanup
**Kill test:** `test_postcondition_resources_cleaned_up`
**Expected:** Test fails because temp files still exist

#### test_mutation_stop_no_state_update
**Mutant:** `stop()` terminates but doesn't update task state
**Kill test:** `test_postcondition_task_state_updated`
**Expected:** Test fails because state is not STOPPED

#### test_mutation_skip_precondition_check
**Mutant:** `stop()` skips TaskNotRunning check
**Kill test:** `test_precondition_task_must_be_running`
**Expected:** Test fails because non-running task is "stopped"

---

## Proptest Tests

### Property-Based Testing

#### test_prop_stop_always_returns_result_type
**Property:** For any Task input, `stop()` returns `Result<ExitCode, ShutdownError>`
**Shrink:** Minimize task fields to find minimal failing case
**Arbitrary:** Generate random Task structs

#### test_prop_stop_idempotent_after_first_call
**Property:** After first successful `stop()` call, second call returns `Ok(ExitCode(0))`
**Shrink:** Reduce number of concurrent calls
**Arbitrary:** Generate tasks with various states

#### test_prop_timeout_bounds_execution_time
**Property:** `stop()` execution time ≤ graceful_timeout + force_timeout + overhead
**Shrink:** Minimize timeout values
**Arbitrary:** Generate various timeout configurations

#### test_prop_active_map_consistency
**Property:** After `stop()`, task ID is removed from active map
**Shrink:** Reduce number of active tasks
**Arbitrary:** Generate tasks with various IDs

---

## Test Execution Strategy

### Priority Order

1. **Critical (Priority 0):**
   - `test_shell_stop_terminates_running_process_gracefully`
   - `test_podman_stop_stops_container_gracefully`
   - `test_shell_stop_times_out_and_forces_kill`
   - `test_e2e_shell_runtime_full_lifecycle`

2. **High (Priority 1):**
   - All precondition violation tests
   - All cleanup failure tests
   - `test_shell_stop_concurrent_calls_thread_safe`

3. **Medium (Priority 2):**
   - Edge case tests
   - Mutation tests
   - Contract verification tests

4. **Low (Priority 3):**
   - Property-based tests
   - Real-world scenario tests

### Test Isolation

- Each test runs in isolated environment
- Shell tests use temp directories with cleanup
- Podman tests use unique container names
- Mock tests require no external dependencies

### CI/CD Integration

```yaml
# .github/workflows/test-shutdown.yml
name: Shutdown Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run shutdown tests
        run: cargo test --package twerk-app --lib worker::shell::tests
      - name: Run podman shutdown tests
        run: cargo test --package twerk-app --lib worker::podman::tests
```

---

## Test Coverage Goals

- **Line Coverage:** ≥ 90% for shell.rs and podman.rs stop() methods
- **Branch Coverage:** ≥ 85% (all error paths covered)
- **Mutation Score:** ≥ 80% (mutation tests kill mutants)
- **Contract Coverage:** 100% (all pre/post/invariant tests pass)

## Exit Criteria

Test plan is complete when:
- [ ] All happy path tests pass
- [ ] All error path tests pass
- [ ] All edge case tests pass
- [ ] All contract verification tests pass
- [ ] Integration tests pass
- [ ] Mutation tests achieve ≥ 80% kill rate
- [ ] Proptest properties hold
- [ ] No flaky tests (3 consecutive passes required)
