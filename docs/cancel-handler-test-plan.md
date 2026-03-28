# Test Plan: Cancel Handler

## Summary

- **Behaviors identified**: 12
- **Trophy allocation**: 4 unit / 6 integration / 2 e2e
- **Proptest invariants**: 2
- **Fuzz targets**: 1
- **Kani harnesses**: 0 (no unsafe arithmetic or critical state machine)

---

## 1. Behavior Inventory

| # | Behavior |
|---|----------|
| 1 | `NewCancelHandler` returns a handler func that closes over datastore and broker |
| 2 | `handle` marks job as CANCELLED when state is RUNNING or SCHEDULED |
| 3 | `handle` is no-op when job state is PENDING, COMPLETED, FAILED, or CANCELLED |
| 4 | `handle` notifies parent job to cancel when job has a ParentID |
| 5 | `handle` returns error when parent task lookup fails |
| 6 | `handle` returns error when parent job lookup fails |
| 7 | `handle` logs and continues when broker publish for parent job fails |
| 8 | `cancelActiveTasks` fetches active tasks and cancels each |
| 9 | `cancelActiveTasks` returns error when GetActiveTasks fails |
| 10 | `cancelActiveTasks` cancels sub-job when task has SubJob |
| 11 | `cancelActiveTasks` requeues task to node queue when task has NodeID but no SubJob |
| 12 | `cancelActiveTasks` returns error on UpdateTask, SubJob fetch, SubJob publish, Node fetch, or Task publish failure |

---

## 2. Trophy Allocation

| Behavior | Layer | Rationale |
|----------|-------|-----------|
| 1 | Integration | Constructor with real deps; verify handler func returned |
| 2 | Unit | State transition logic; pure assertion on JobState |
| 3 | Unit | No-op guard; verify UpdateJob not called for terminal states |
| 4 | Integration | Cross-component: datastore + broker pub/sub |
| 5, 6 | Unit | Error path; verify specific error returned |
| 7 | Integration | Error handling with logging; verify continue-on-failure |
| 8 | Integration | Task iteration; real datastore + broker |
| 9 | Unit | Error boundary; specific error propagation |
| 10 | Integration | Sub-job cascade; real publish |
| 11 | Integration | Task requeue; real broker publish |
| 12 | Unit | Error aggregation; multiple failure modes |

**Target**: ~60% integration (6), ~40% unit (5), ~5% e2e (1) — simplified for handler-only scope.

---

## 3. BDD Scenarios

### Behavior 1: NewCancelHandler returns handler func

```
### Scenario: cancel_handler_returns_handler_func
Given: a valid Datastore and Broker
When: NewCancelHandler is called
Then: a non-nil job.HandlerFunc is returned
And: the handler func is of type func(context.Context, job.EventType, *tork.Job) error
```

---

### Behavior 2: handle marks RUNNING job as CANCELLED

```
### Scenario: handle_marks_running_job_cancelled
Given: a job with ID "job-1" in RUNNING state
And: a datastore that supports UpdateJob
And: a broker (may be nil or stub)
When: the cancel handler.handle is invoked
Then: the job is updated to state CANCELLED
And: UpdateJob was called exactly once with the job ID
```

```
### Scenario: handle_marks_scheduled_job_cancelled
Given: a job with ID "job-1" in SCHEDULED state
When: the cancel handler.handle is invoked
Then: the job is updated to state CANCELLED
```

---

### Behavior 3: handle no-ops for terminal states

```
### Scenario: handle_does_nothing_when_job_pending
Given: a job with ID "job-1" in PENDING state
When: the cancel handler.handle is invoked
Then: UpdateJob is not called
And: no error is returned

### Scenario: handle_does_nothing_when_job_completed
Given: a job with ID "job-1" in COMPLETED state
When: the cancel handler.handle is invoked
Then: UpdateJob is not called

### Scenario: handle_does_nothing_when_job_failed
Given: a job with ID "job-1" in FAILED state
When: the cancel handler.handle is invoked
Then: UpdateJob is not called

### Scenario: handle_is_idempotent_when_already_cancelled
Given: a job with ID "job-1" in CANCELLED state
When: the cancel handler.handle is invoked
Then: UpdateJob is not called (already cancelled)
```

---

### Behavior 4: handle notifies parent job when ParentID exists

```
### Scenario: handle_cancels_parent_job_when_job_has_parent
Given: a job "job-1" with ParentID "parent-task-1"
And: parent task "parent-task-1" belongs to job "parent-job-1"
And: broker is a mock that tracks PublishJob calls
When: the cancel handler.handle is invoked on job-1
Then: the parent job "parent-job-1" is published to broker with state CANCELLED
And: broker.PublishJob was called exactly once with the parent job
```

---

### Behavior 5: handle returns error when parent task lookup fails

```
### Scenario: handle_returns_error_when_parent_task_not_found
Given: a job with ParentID "nonexistent-task"
And: datastore.GetTaskByID returns ErrTaskNotFound
When: the cancel handler.handle is invoked
Then: an error is returned containing "error fetching parent task"
```

---

### Behavior 6: handle returns error when parent job lookup fails

```
### Scenario: handle_returns_error_when_parent_job_not_found
Given: a job with ParentID "parent-task-1"
And: datastore.GetTaskByID succeeds but returns task with JobID "nonexistent-job"
And: datastore.GetJobByID returns ErrJobNotFound
When: the cancel handler.handle is invoked
Then: an error is returned containing "error fetching parent job"
```

---

### Behavior 7: handle logs and continues when broker publish fails for parent job

```
### Scenario: handle_continues_when_parent_broker_publish_fails
Given: a job with a valid ParentID pointing to a parent job
And: broker.PublishJob returns an error
When: the cancel handler.handle is invoked
Then: the error is logged (not returned)
And: cancelActiveTasks is still called
And: no error is returned from handle
```

---

### Behavior 8: cancelActiveTasks fetches and cancels active tasks

```
### Scenario: cancel_active_tasks_marks_all_active_tasks_cancelled
Given: a job "job-1" with 3 active tasks
And: each task is in RUNNING state
When: cancelActiveTasks is called
Then: each task is updated to state CANCELLED in the datastore
And: datastore.UpdateTask was called 3 times
```

```
### Scenario: cancel_active_tasks_handles_empty_task_list
Given: a job "job-1" with no active tasks
When: cancelActiveTasks is called
Then: no error is returned
And: UpdateTask is not called
```

---

### Behavior 9: cancelActiveTasks returns error when GetActiveTasks fails

```
### Scenario: cancel_active_tasks_returns_error_when_get_active_tasks_fails
Given: a job "job-1"
And: datastore.GetActiveTasks returns an error
When: cancelActiveTasks is called
Then: an error is returned containing "error getting active tasks for job"
```

---

### Behavior 10: cancelActiveTasks cancels sub-job when task has SubJob

```
### Scenario: cancel_active_tasks_cancels_subjob_task
Given: a task with SubJob.ID "subjob-1" and NodeID ""
When: cancelActiveTasks is called
Then: the sub-job "subjob-1" is fetched and its state is set to CANCELLED
And: broker.PublishJob is called with the cancelled sub-job
```

---

### Behavior 11: cancelActiveTasks requeues to node when task has NodeID but no SubJob

```
### Scenario: cancel_active_tasks_requeues_to_node_queue
Given: a task with NodeID "node-1" and SubJob is nil
And: the node "node-1" has Queue "default"
When: cancelActiveTasks is called
Then: broker.PublishTask is called with the task to queue "default"
```

---

### Behavior 12: cancelActiveTasks propagates errors

```
### Scenario: cancel_active_tasks_returns_error_when_update_task_fails
Given: a job with one active task
And: datastore.UpdateTask returns an error for that task
When: cancelActiveTasks is called
Then: an error is returned containing "error cancelling task"

### Scenario: cancel_active_tasks_returns_error_when_subjob_not_found
Given: a task with SubJob.ID "nonexistent-subjob"
And: datastore.GetJobByID returns ErrJobNotFound
When: cancelActiveTasks is called
Then: an error is returned

### Scenario: cancel_active_tasks_returns_error_when_subjob_publish_fails
Given: a task with SubJob.ID "subjob-1"
And: datastore.GetJobByID succeeds
And: broker.PublishJob returns an error
When: cancelActiveTasks is called
Then: an error is returned containing "error publishing cancellation for sub-job"

### Scenario: cancel_active_tasks_returns_error_when_node_not_found
Given: a task with NodeID "nonexistent-node" and no SubJob
And: datastore.GetNodeByID returns ErrNodeNotFound
When: cancelActiveTasks is called
Then: an error is returned

### Scenario: cancel_active_tasks_returns_error_when_task_publish_fails
Given: a task with NodeID "node-1" and no SubJob
And: broker.PublishTask returns an error
When: cancelActiveTasks is called
Then: an error is returned
```

---

## 4. Proptest Invariants

### Invariant 1: Job state after handle is deterministic

```
Property: For any job in RUNNING or SCHEDULED state, after handle() the state is CANCELLED
Strategy: Generate arbitrary job with state in [RUNNING, SCHEDULED]
Anti-invariant: Jobs in other states remain unchanged
```

### Invariant 2: All active tasks are cancelled

```
Property: cancelActiveTasks leaves no task in an active state (CREATED, PENDING, SCHEDULED, RUNNING)
Strategy: Generate job with N active tasks; verify each task.State == CANCELLED after call
Anti-invariant: Any task not updated remains in active state
```

---

## 5. Fuzz Targets

```
### Fuzz Target: handle with arbitrary job state
Input type: *tork.Job with randomized State, ParentID, and sub-job references
Risk: Panic on nil pointer dereference, logic error in state transition
Corpus seeds:
  - Job in RUNNING with valid ParentID
  - Job in SCHEDULED with SubJob task
  - Job in terminal state (COMPLETED, FAILED, CANCELLED)
  - Job with empty ParentID and no tasks
```

---

## 6. Kani Harnesses

Not applicable. The cancel handler performs no unsafe arithmetic, pointer dereferences in hot paths, or exhaustive state machine transitions that require formal verification. Property testing (proptest) is sufficient for invariants.

---

## 7. Mutation Checkpoints

| Mutation | Must be caught by |
|----------|-------------------|
| Remove `u.State = tork.JobStateCancelled` in handle | `handle_marks_running_job_cancelled` |
| Change `!=` to `==` in state guard | `handle_does_nothing_when_job_pending` |
| Remove parent job publish block | `handle_cancels_parent_job_when_job_has_parent` |
| Change parent publish error from log-only to return | `handle_continues_when_parent_broker_publish_fails` |
| Remove task loop in cancelActiveTasks | `cancel_active_tasks_marks_all_active_tasks_cancelled` |
| Change SubJob publish to return error instead of log | Not currently tested (error is returned but not specifically asserted) |
| Remove NodeID requeue branch | `cancel_active_tasks_requeues_to_node_queue` |

**Threshold**: 85% mutation kill rate minimum.

---

## 8. Combinatorial Coverage Matrix

| Scenario | Input State | Has ParentID | Has Active Tasks | Has SubJob Task | Has NodeID Task | Expected |
|----------|-------------|--------------|-------------------|-----------------|-----------------|----------|
| job_running_cancelled | RUNNING | false | 0 | false | false | CANCELLED |
| job_running_cancelled_with_parent | RUNNING | true | 0 | false | false | CANCELLED + parent notified |
| job_scheduled_cancelled | SCHEDULED | false | 0 | false | false | CANCELLED |
| job_pending_noop | PENDING | false | 0 | false | false | no-op |
| job_completed_noop | COMPLETED | false | 0 | false | false | no-op |
| job_failed_noop | FAILED | false | 0 | false | false | no-op |
| job_cancelled_idempotent | CANCELLED | false | 0 | false | false | no-op |
| active_tasks_cancelled | RUNNING | false | 3 | false | false | all CANCELLED |
| subjob_task_cancelled | RUNNING | false | 1 | true | false | subjob CANCELLED |
| node_task_requeued | RUNNING | false | 1 | false | true | task requeued |
| get_active_tasks_error | RUNNING | false | - | - | - | error propagated |
| update_task_error | RUNNING | false | 1 | false | false | error propagated |

---

## Open Questions

1. **Broker publish on parent failure**: The code logs and continues. Should this be configurable? A failed parent notification could leave the system in an inconsistent state where a sub-job is cancelled but the parent continues.

2. **SubJob publish failure is fatal**: Unlike parent job publish failure, sub-job publish failure returns an error. Is this intentional? It stops the cancellation of remaining tasks.

3. **Task ordering**: cancelActiveTasks processes tasks sequentially. If task 1 publish fails, tasks 2-N are never processed. Should this be parallelized with error aggregation?

4. **GetActiveTasks definition**: What constitutes an "active" task? The code uses `ds.GetActiveTasks()` but the definition of active (CREATED, PENDING, SCHEDULED, RUNNING) lives in `TaskStateActive`. Verify this matches cancellation intent.
