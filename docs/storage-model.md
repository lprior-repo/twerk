# Twerk Storage Model

Status: draft

Audience: storage implementers, runtime implementers, observability implementers, and AI agents.

## Purpose

This document defines the logical persisted model for Twerk. The first implementation targets Fjall, but the contract is written in terms of logical keyspaces and records.

Storage must support:

- Durable run admission
- Immutable event journal
- Replay and recovery
- Step outputs and errors
- Logs and traces
- Wait and ask resumption
- Collect/reduce/repeat checkpointing
- Secret taint metadata
- Retention and compaction policy

## Fjall Constraints

Twerk uses Fjall as an embedded single-process store.

Operational constraints:

- One process opens the database at a time.
- Atomic multi-keyspace writes use one batch.
- Integer key parts use big-endian bytes for ordering.
- Events are logically append-only.
- Compaction means physical SST files are not an audit log.
- Retention must be explicit; do not rely on old LSM files.

## Key Design Rules

- Use fixed-width binary prefixes for hot records.
- Use ULID/UUID-style run IDs with sortable encoding where useful.
- Event sequence numbers are monotonically increasing per run.
- Event keys sort by run ID then sequence.
- Secondary indexes are updated atomically with primary records.
- Large payloads are stored as blobs and referenced by digest or blob ID.

## Logical Keyspaces

| Keyspace | Purpose |
|---|---|
| `workflow_snapshots` | Immutable workflow source and compiled IR. |
| `workflow_index` | Latest workflow name/digest pointers. |
| `runs` | Run projection by run ID. |
| `run_status_index` | Runs by status and creation time. |
| `run_events` | Immutable per-run event journal. |
| `step_states` | Current step projection. |
| `attempts` | Step attempt records and leases. |
| `step_outputs` | Redacted or referenced step outputs. |
| `errors` | Structured validation/runtime errors. |
| `logs` | Structured log records. |
| `traces` | Timing and span records. |
| `timers` | Due-time index for waits/retries/repeats. |
| `asks` | Durable human prompt records. |
| `dedupe` | Trigger uniqueness and idempotency receipts. |
| `collect_state` | Cursor and collection checkpoints. |
| `reduce_state` | Accumulator checkpoints. |
| `repeat_state` | Repeat attempt checkpoints. |
| `taint` | Secret taint metadata. |
| `payload_blobs` | Large payload data. |
| `retention` | Expiration indexes. |

## Workflow Snapshot Record

Key:

```text
[workflow_digest]
```

Value:

```json
{
  "workflow_digest": "sha256:...",
  "compiled_ir_digest": "sha256:...",
  "name": "issue_triage",
  "language_version": "twerk/v1",
  "source_yaml": "...",
  "compiled_ir": {},
  "action_versions": {},
  "created_at": "2026-04-29T00:00:00Z"
}
```

Snapshots are immutable. New workflow edits create new digests.

## Run Record

Key:

```text
[run_id]
```

Value:

```json
{
  "run_id": "run_01h",
  "workflow_name": "issue_triage",
  "workflow_digest": "sha256:...",
  "compiled_ir_digest": "sha256:...",
  "status": "running",
  "trigger": {
    "kind": "webhook",
    "unique": "github-delivery-id"
  },
  "input_ref": "blob_or_inline",
  "created_at": "2026-04-29T00:00:00Z",
  "updated_at": "2026-04-29T00:00:01Z",
  "terminal_at": null,
  "result_ref": null,
  "error_ref": null
}
```

## Event Record

Key:

```text
[run_id | seq_be_u64]
```

Value:

```json
{
  "seq": 42,
  "type": "StepSucceeded",
  "run_id": "run_01h",
  "step_id": "classify",
  "attempt": 1,
  "time": "2026-04-29T00:00:00Z",
  "data": {},
  "redaction": {}
}
```

Rules:

- Events are immutable.
- Event append must be atomic with projection updates.
- Per-run sequence numbers must not skip during normal operation.
- Rebuilding a projection from events must produce the same projection state.

## Step State Record

Key:

```text
[run_id | step_id]
```

Value:

```json
{
  "run_id": "run_01h",
  "step_id": "classify",
  "status": "succeeded",
  "attempts": 1,
  "started_at": "...",
  "finished_at": "...",
  "output_ref": "...",
  "error_ref": null,
  "scope": "top"
}
```

## Attempt And Lease Record

Key:

```text
[run_id | step_id | attempt_be_u32]
```

Value:

```json
{
  "attempt": 1,
  "status": "running",
  "lease_id": "lease_01h",
  "owner": "runtime-1",
  "claimed_at": "...",
  "expires_at": "...",
  "idempotency_key": "...",
  "resolved_with_ref": "...",
  "output_ref": null,
  "error_ref": null
}
```

## Output Storage

Step outputs are stored inline only if below the inline threshold.

Large outputs are blobbed.

Value:

```json
{
  "run_id": "run_01h",
  "step_id": "classify",
  "content_type": "application/json",
  "size": 128,
  "inline": {
    "label": "bug",
    "confidence": 0.91
  },
  "blob_ref": null,
  "taint_ref": null
}
```

Rules:

- Outputs are validated before storage.
- Outputs over limit fail the step.
- Secret-tainted output metadata is stored separately and consulted during redaction.

## Error Record

Key:

```text
[run_id | seq_or_error_id]
```

Value:

```json
{
  "code": "ACTION_FAILED",
  "message": "Action failed",
  "step": "classify",
  "retryable": true,
  "details": {},
  "source": {
    "path": "steps[1]",
    "line": 24,
    "column": 5
  }
}
```

## Logs

Key:

```text
[run_id | time_be_u64 | log_seq_be_u32]
```

Value:

```json
{
  "level": "info",
  "run_id": "run_01h",
  "step_id": "classify",
  "attempt": 1,
  "message": "calling action ai.classify",
  "fields": {}
}
```

Logs are redacted before user-visible persistence unless runtime policy stores encrypted raw logs separately.

## Traces

Key:

```text
[run_id | span_start_be_u64 | span_id]
```

Value:

```json
{
  "span_id": "span_01h",
  "parent_span_id": null,
  "kind": "step.action",
  "step_id": "classify",
  "started_at": "...",
  "duration_ms": 184,
  "attributes": {}
}
```

## Timers

Key:

```text
[due_time_be_u64 | run_id | step_id | timer_id]
```

Value:

```json
{
  "timer_id": "timer_01h",
  "kind": "wait",
  "run_id": "run_01h",
  "step_id": "wait_for_job",
  "due_at": "2026-04-29T00:05:00Z",
  "status": "pending"
}
```

## Ask Prompts

Key:

```text
[run_id | step_id]
```

Secondary index:

```text
[status | timeout_be_u64 | run_id | step_id]
```

Value:

```json
{
  "run_id": "run_01h",
  "step_id": "approval",
  "question": "Approve deploy?",
  "choices": ["approve", "reject"],
  "to": "role:deploy_approver",
  "status": "pending",
  "created_at": "...",
  "timeout_at": "...",
  "answered_by": null,
  "answer": null
}
```

## Dedupe And Receipts

Key:

```text
[scope | key_hash]
```

Value:

```json
{
  "scope": "webhook:issue_triage",
  "key_hash": "sha256:...",
  "run_id": "run_01h",
  "created_at": "...",
  "expires_at": "..."
}
```

Dedupe records need explicit retention. Compaction is not a retention policy.

## Collect State

Key:

```text
[run_id | step_id]
```

Value:

```json
{
  "cursor": "next_cursor",
  "page": 12,
  "items": 1200,
  "items_ref": "blob_or_inline",
  "limits": {
    "pages": 500,
    "items": 50000,
    "time_deadline": "..."
  },
  "status": "running"
}
```

## Reduce State

Key:

```text
[run_id | step_id]
```

Value:

```json
{
  "index": 42,
  "accumulator_ref": "blob_or_inline",
  "input_digest": "sha256:...",
  "status": "running"
}
```

## Repeat State

Key:

```text
[run_id | step_id]
```

Value:

```json
{
  "attempts": 7,
  "last_output_ref": "...",
  "next_due_at": "...",
  "limits": {
    "times": 60,
    "deadline": "..."
  },
  "status": "waiting"
}
```

## Taint Metadata

Taint records describe secret-derived fields.

Value:

```json
{
  "value_ref": "step_output:run_01h:step",
  "paths": [
    {
      "path": "headers.Authorization",
      "source": "secret.github_token"
    }
  ]
}
```

Rules:

- Taint follows object/list paths.
- Interpolating a tainted value taints the full resulting string.
- Taint metadata must be available to result evaluation and redaction.

## Payload Blobs

Blob keys:

```text
[blob_id]
```

Blob records include digest, size, content type, created time, and retention class.

Normal step outputs should remain small. Blob storage is for payloads that are allowed by runtime policy but too large to inline.

## Atomic Write Groups

These changes must commit in one batch:

- Run admission event, run projection, queue record, dedupe receipt.
- Step success event, step projection, output record, downstream queue records.
- Step failure event, error record, retry/timer record or failure projection.
- Ask answer event, ask projection, step output, next queue record.
- Timer resume event, timer projection, next queue record.

## Retention And Compaction

Retention classes:

| Class | Suggested default |
|---|---:|
| Completed run projections | 30 days |
| Event journal | 30 days or policy |
| Logs | 7 days |
| Traces | 7 days |
| Dedupe receipts | 24 hours to 30 days by trigger |
| Workflow snapshots | Keep while referenced by any run |
| Blobs | Shortest owning record retention |

Deletion must be logical and policy-driven. Fjall compaction may reclaim space later.

## Backup And Recovery

Backups must include all keyspaces from the same point in time. Restoring only projections without events is not sufficient for audit-grade recovery.

Recovery priority:

1. Recover from clean projections.
2. Rebuild damaged projections from event journal.
3. Mark unrecoverable runs failed with storage corruption diagnostics.

## Open Questions

- Should raw unredacted payloads ever be stored encrypted for admin-only diagnostics?
- Should logs be append-only events only, or separate log keyspace plus event references?
- What are v1 default retention windows?
