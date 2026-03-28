# Cancel Handler Contract Specification

## Context

- **Feature**: Cancel Handler - handles job cancellation requests
- **Domain Terms**:
  - `Job` - a unit of work that can be in states: `Running`, `Scheduled`, `Cancelled`, etc.
  - `Task` - a sub-unit of work within a job, can have `TaskStateCancelled`
  - `ParentID` - reference to a parent task when a job is spawned from a task
  - `SubJob` - a job spawned from a task
  - `Node` - worker node executing a task
  - `ActiveTasks` - tasks that are currently running or scheduled
- **Assumptions**:
  - Job cancellation is idempotent (cancelling an already-cancelled job is a no-op)
  - Only jobs in `Running` or `Scheduled` state can be cancelled
  - Active tasks are tasks that have `NodeID` set (assigned to a node) or have `SubJob`
- **Open Questions**:
  - What happens if parent task is not found? (Currently returns error)
  - Should cancellation errors for sub-jobs be fatal or logged? (Currently logged, not fatal)

## Preconditions

- [ ] `ctx` is a valid, non-nil context
- [ ] `job` is non-nil and has a valid `ID`
- [ ] `ds` (Datastore) is available and connected
- [ ] `broker` is available and connected
- [ ] Job exists in datastore (verified by `UpdateJob`)

## Postconditions

- [ ] Job state is set to `Cancelled` if it was in `Running` or `Scheduled` state
- [ ] Job state remains unchanged if it was in any other state
- [ ] All active tasks for the job are marked with `TaskStateCancelled`
- [ ] If job has a parent task, parent job is published to broker for cancellation
- [ ] If task has a sub-job, sub-job is published to broker for cancellation
- [ ] If task is running on a node, task is published to node's queue for cancellation
- [ ] Datastore is updated atomically for each entity

## Invariants

- [ ] Job state machine: only `Running` or `Scheduled` jobs can transition to `Cancelled`
- [ ] Task state machine: only active tasks can transition to `Cancelled`
- [ ] Parent-child relationships are preserved after cancellation
- [ ] No orphan tasks left after job cancellation
- [ ] Sub-job cancellation does not affect parent job directly (only via publish)

## Error Taxonomy

- `Error::DatastoreUnavailable` - datastore connection is unavailable or operation times out
- `Error::JobNotFound` - job with given ID does not exist in datastore
- `Error::TaskNotFound` - task with given ID does not exist in datastore
- `Error::NodeNotFound` - node with given ID does not exist in datastore
- `Error::ParentTaskNotFound` - parent task referenced by `ParentID` does not exist
- `Error::ParentJobNotFound` - parent job for a task does not exist
- `Error::PublishFailed` - broker failed to publish cancellation message for job or task
- `Error::SubJobNotFound` - sub-job referenced by task does not exist
- `Error::InvalidStateTransition` - attempted to cancel a job not in `Running` or `Scheduled` state
- `Error::ContextCancelled` - context was cancelled before operation completed
- `Error::TransactionFailed` - datastore transaction failed during update

## Contract Signatures

```rust
// Primary cancel handler
fn cancel_handler(ctx: Context, job: Job) -> Result<(), CancelError>

// Cancel all active tasks for a job
fn cancel_active_tasks(ctx: Context, job_id: JobId) -> Result<(), CancelError>

// Error type
enum CancelError {
    DatastoreUnavailable,
    JobNotFound { job_id: JobId },
    TaskNotFound { task_id: TaskId },
    NodeNotFound { node_id: NodeId },
    ParentTaskNotFound { parent_id: TaskId },
    ParentJobNotFound { job_id: JobId },
    PublishFailed { entity: String, id: String },
    SubJobNotFound { sub_job_id: JobId },
    InvalidStateTransition { current_state: JobState, expected_states: Vec<JobState> },
    ContextCancelled,
    TransactionFailed { reason: String },
}
```

## Non-goals

- [ ] Handling job restart or rescheduling after cancellation
- [ ] Graceful vs force cancellation (all cancellation is immediate)
- [ ] Partial cancellation (either all tasks are cancelled or none)
- [ ] Cancellation of completed or failed jobs

## State Transitions

```
Job State Machine:
  Running    -> Cancelled (valid)
  Scheduled  -> Cancelled (valid)
  Cancelled  -> Cancelled (idempotent, no-op)
  Completed  -> Cancelled (invalid - raises error)
  Failed     -> Cancelled (invalid - raises error)

Task State Machine:
  Running    -> Cancelled (valid)
  Scheduled  -> Cancelled (valid)
  Cancelled  -> Cancelled (idempotent, no-op)
  Completed  -> Cancelled (invalid - raises error)
  Failed     -> Cancelled (invalid - raises error)
```
