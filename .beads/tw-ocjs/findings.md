# Findings: tw-ocjs - DAG Dependency Tests

## Issue
Test scheduler respects task dependencies before execution in `crates/twerk-app/src/engine/coordinator/scheduler/dag.rs`.

## Work Performed

### 1. Pre-existing Test File
The `dag.rs` test file already existed with comprehensive tests covering:
- Circular dependency rejection
- Direct cycle rejection
- B waits for A completion
- C waits for B
- Failure propagation (A fails → B and C cancelled)
- Topological ordering
- Multiple tasks with no dependencies

### 2. Compilation Errors Found and Fixed

**Error 1: `&&str` vs `Into<String>` mismatch** (`dag.rs:39`)
- `create_task_with_deps` was passing `&&str` to `TaskId::new` which expects `impl Into<String>`
- Fix: Dereference with `*s`

**Error 2: Use of moved value** (`dag.rs:66-68`)
- `result.unwrap_err()` moved `result`, then `result.err()` was called on moved value
- Fix: Use `result.as_ref().unwrap_err()` to borrow

**Error 3: Async recursion without boxing** (`mod.rs:172`)
- `propagate_cancellation` is an async fn that recursively calls itself
- Rust requires boxing for recursive async fns
- Fix: Refactored to iterative approach using a queue

**Error 4: Wrong job_id passed to get_all_tasks_for_job** (`mod.rs:186`)
- `propagate_cancellation` was calling `get_all_tasks_for_job(&task_id.to_string())` with a task ID
- The method expects a job ID
- Fix: Extract job_id from the failed task first, then use that for the query

**Error 5: submit_dag scheduled all tasks immediately** (`mod.rs:145-148`)
- `submit_dag` was calling `schedule_task` for ALL tasks in topological order
- Tasks with dependencies should be `Pending` until dependencies complete
- Fix: Only call `schedule_task` for root tasks (no dependencies); others are set to `Pending`

### 3. Tests Verified
All 7 DAG tests pass:
```
dag_submit_rejects_circular_dependency_a_depends_on_b_which_depends_on_a
dag_submit_rejects_direct_cycle_a_depends_on_b_which_depends_on_a
dag_b_waits_for_a_when_a_completes_before_b
dag_c_waits_for_b_when_b_waits_for_a
dag_when_a_fails_b_and_c_are_cancelled
dag_topological_order_respected_across_multiple_levels
dag_multiple_tasks_can_run_when_no_dependencies
```

All 21 scheduler tests pass.

## Files Changed
- `crates/twerk-app/src/engine/coordinator/scheduler/dag.rs` - Fixed `&&str` and moved value errors
- `crates/twerk-app/src/engine/coordinator/scheduler/mod.rs` - Fixed async recursion and job_id bug; updated submit_dag to respect dependencies
