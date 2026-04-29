# Twerk Runtime Architecture Contract

Status: draft

Audience: runtime implementers, storage implementers, CLI implementers, server implementers, and AI agents.

## Purpose

This document defines how the Twerk single-binary runtime executes validated workflows durably, observably, and at high throughput.

The runtime contract bridges the language spec and the implementation:

```text
workflow YAML -> validated IR -> durable run admission -> scheduler -> action execution -> event journal -> inspection
```

## Core Guarantees

- Accepted means durable.
- Runs bind to immutable workflow snapshots.
- Step execution is at-least-once.
- Completed steps do not rerun during normal replay.
- External side effects are not exactly-once.
- All state transitions are evented.
- All queues are bounded.
- Restart recovery is deterministic.
- Observability data is queryable through CLI and API.

## Runtime Components

```text
Twerk single binary
├── Admission
├── Validator
├── Workflow snapshot store
├── Durable queue
├── Scheduler
├── Lease manager
├── Action executor
├── Timer service
├── Ask service
├── Event journal
├── Projection updater
├── CLI/API inspection layer
└── Fjall storage
```

## Run Admission Flow

Admission is the only path that creates a run.

Steps:

1. Receive trigger from CLI, webhook, schedule, event, or server API.
2. Resolve workflow definition.
3. Validate workflow if not already compiled.
4. Bind immutable workflow snapshot.
5. Map runtime source data into `inputs`.
6. Validate input types and size limits.
7. Resolve required secrets availability without exposing values.
8. Evaluate trigger uniqueness or idempotency key.
9. Check queue capacity and runtime policy.
10. Commit run creation, initial events, and queue record in one durable batch.
11. Acknowledge accepted run.

If the runtime returns a run ID, the admission batch has already committed.

Queue full behavior:

```text
CLI: non-zero exit with QUEUE_FULL
HTTP: 503 with structured QUEUE_FULL body
```

Duplicate webhook behavior:

```text
Return existing run ID, do not enqueue a new run.
```

## Workflow Snapshot Binding

Every run stores:

- Workflow name
- Workflow language version
- Workflow source digest
- Compiled IR digest
- Action versions resolved at admission
- Runtime config digest relevant to execution

In-flight runs always resume with the snapshot they started with. Editing a workflow affects future runs only.

## Queue Model

The queue is durable and bounded.

Logical queues:

| Queue | Purpose |
|---|---|
| run_ready | Runs ready to execute. |
| step_ready | Steps ready to execute. |
| timer_ready | Wait/retry/repeat wakeups ready to resume. |
| ask_pending | Human prompts waiting for answer. |
| action_leases | Claimed action attempts. |

Queue records must include priority, due time, run ID, step ID, attempt ID, and lease metadata where applicable.

Backpressure is applied before acceptance when the durable queue is full.

## Scheduler Loop

The scheduler loop is deterministic and projection-driven.

Pseudo-flow:

```text
poll ready records
claim lease atomically
load run projection
determine next executable step or continuation
emit StepStarted or continuation event
execute primitive or enqueue action attempt
commit resulting events and projections
repeat
```

The scheduler must never rely on in-memory state as the sole truth. In-memory caches are acceleration only.

## Lease And Claim Behavior

Each executable unit uses a lease.

Lease fields:

```text
lease_id
run_id
step_id
attempt
owner
claimed_at
expires_at
heartbeat_at
```

Rules:

- Claim is atomic.
- Expired leases are recoverable.
- Heartbeats are durable enough to prevent duplicate execution during healthy long-running actions.
- A recovered expired lease may cause at-least-once action execution.
- Idempotency keys are required for retry-safe external writes.

## Step Execution Lifecycle

Normal lifecycle:

```text
pending -> running -> succeeded
pending -> skipped
pending -> running -> retrying -> running -> succeeded
pending -> running -> failed
pending/running/waiting/asking -> cancelled
```

Execution sequence:

1. Resolve `if`.
2. If false, record `StepSkipped` and follow `then` or natural next.
3. Resolve step inputs and `with` values.
4. Validate action/input contract.
5. Record `StepStarted` or primitive-specific started event.
6. Execute primitive.
7. Validate output schema and size limits.
8. Record success/failure event.
9. Update projections and ready queues in the same batch.

## Primitive Execution

Pure primitives such as `set` execute in the scheduler without external worker overhead.

Side-effecting `do` actions execute through the action executor.

Control primitives manage sub-scopes:

- `choose` evaluates one branch.
- `for_each` creates isolated item scopes.
- `together` creates isolated branch scopes.
- `collect` creates page attempts and a scoped accumulator.
- `reduce` creates accumulator versions.
- `repeat` creates bounded attempts.
- `wait` creates timer or event subscription records.
- `ask` creates durable prompt records.
- `finish` records terminal status.

## Durable Event Journal

Every meaningful transition emits an immutable event.

Examples:

```text
RunAccepted
RunStarted
StepSkipped
StepStarted
StepSucceeded
StepFailed
StepRetryScheduled
WaitStarted
WaitResumed
AskCreated
AskAnswered
CollectPageStarted
CollectPageSucceeded
TogetherBranchFailed
RunSucceeded
RunFailed
RunCancelled
```

Event append and projection updates must commit atomically.

## Replay Semantics

Replay after restart:

1. Load latest run projection.
2. If projection is missing or suspect, rebuild from event journal.
3. Recreate ready timers, ask prompts, and queue records from durable state.
4. Reclaim expired leases according to lease policy.
5. Resume from the last durable state.

Completed steps are not rerun. Incomplete leased actions may be retried, causing at-least-once execution.

Manual replay creates a new run with a relationship to the source run. It must not mutate the original run.

## Restart Recovery

On process start:

1. Open Fjall database with exclusive process lock.
2. Recover or verify projections.
3. Scan non-terminal runs.
4. Rebuild due timer queue.
5. Rebuild ask pending index.
6. Requeue steps that were ready but not leased.
7. Mark expired leases for recovery.
8. Start scheduler loops.

Clean shutdown should stop admission first, drain or checkpoint active leases, persist shutdown marker, and close storage.

## Cancellation

Cancellation flow:

1. Record `RunCancellationRequested`.
2. Stop scheduling new steps for the run.
3. Cancel waiting timers and ask prompts.
4. Send best-effort cancellation to running actions.
5. Mark pending work cancelled.
6. Record terminal `RunCancelled` when local cleanup is complete or abandoned by policy.

Cancellation does not run `on_error` by default and does not undo external side effects.

## Timeouts

Timeout layers:

| Layer | Meaning |
|---|---|
| Action timeout | Maximum time for one action attempt. |
| Step timeout | Maximum time for a step including retries if configured. |
| Wait timeout | Maximum wait or event wait duration. |
| Ask timeout | Maximum prompt lifetime. |
| Run timeout | Maximum runtime for whole run if configured. |

Timeouts emit structured events and errors. Timeout cancellation is best-effort for running actions.

## Wait Resumption

Time waits create timer records with due timestamp.

Event waits create subscription records with match criteria and timeout.

On resume:

1. Record resume event.
2. Produce wait output.
3. Continue to `then` or natural next step.

Wait records must survive restart.

## Ask Resumption

`ask` creates a prompt record.

Prompt fields:

```text
run_id, step_id, question, choices, recipient, created_at, timeout_at, status
```

Answer flow:

1. Authenticate responder.
2. Validate answer.
3. Record `AskAnswered`.
4. Produce ask output.
5. Resume run.

Timeout emits `ASK_TIMEOUT` unless a default answer is configured.

## Parallel Branch Scheduling

`together` creates branch scopes after parent entry is durably recorded.

Failure modes:

| Mode | Runtime behavior |
|---|---|
| fast | First branch failure cancels unfinished branches. |
| after_all | All branches finish, parent fails if any failed. |
| collect | Parent succeeds with per-branch success/error values. |

Successful branch side effects are retained. Runtime never rolls them back automatically.

## Loop Checkpointing

`for_each` records:

- Input list digest
- Item count
- Per-item status
- Per-item output or error
- Concurrency and rate limit state

Output ordering follows input index.

## Collect Checkpointing

`collect` records:

- Current cursor
- Page index
- Page attempt state
- Items appended count
- Accumulator/blob reference
- Limits consumed

After restart, collection resumes from the last durable cursor and item count. Page side effects are handled like action side effects and require idempotency where applicable.

## Reduce Checkpointing

`reduce` records:

- Input list digest
- Current index
- Accumulator version
- Accumulator value or blob reference

After restart, reduce resumes at the next unprocessed index with the latest durable accumulator.

## Repeat Checkpointing

`repeat` records:

- Attempt count
- Last attempt output
- Until condition result
- Next due time
- Limits consumed

After restart, repeat resumes from the next due attempt or final output.

## Backpressure Behavior

Backpressure is explicit.

Admission rejects when:

- Durable run queue is full.
- Workflow concurrency limit is reached and policy is reject.
- Runtime global concurrency limit is saturated and queue policy is reject.
- Disk free space is below safety threshold.

Structured error:

```json
{
  "code": "QUEUE_FULL",
  "message": "Run queue is full",
  "retryable": true
}
```

## Observability Surface

Runtime must support the CLI/API contract for:

- `inspect`
- `events`
- `logs`
- `trace`
- `bundle`
- run streaming

All observability output is derived from durable state plus redacted log/trace projections.

## Performance Requirements

- Compile workflow once and run many times.
- Use in-memory scheduling indexes backed by durable Fjall records.
- Execute pure control/data primitives in-process.
- Avoid process spawning except explicit process/shell actions.
- Commit related events, projections, queue records, and receipts in one storage batch.
- Use bounded async queues only.

## Open Questions

- Should the first runtime have separate internal worker tasks or execute actions directly in scheduler-owned async tasks?
- Should timer recovery scan all timers on boot or maintain a due-time index only?
- What is the default global queue size for v1?
