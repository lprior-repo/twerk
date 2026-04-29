# Twerk CLI And Observability Contract

Status: draft

Audience: CLI implementers, runtime implementers, workflow authors, UI builders, and AI agents.

## Purpose

Twerk is AI-native. The CLI must expose enough structured truth for an AI agent to validate, explain, dry-run, execute, inspect, debug, repair, and replay workflows without guessing.

The CLI has one core rule:

```text
Every human-facing command has a stable machine-readable twin.
```

Pretty output is for people. `--json` and `--jsonl` are contracts for automation.

## CLI Principles

- Machine output is stable and complete.
- Human output is concise and readable.
- Errors have stable codes, source locations, and suggestions.
- Long-running operations stream JSON Lines.
- Every run can produce a complete debug bundle.
- Every action contract is inspectable.
- Redaction is on by default.
- No progress spinners or terminal control codes in machine mode.
- Every command is safe for AI agents to run non-interactively.

## Core AI Loop

The core AI repair and execution loop should be trivial:

```bash
twerk validate flow.yaml --json
twerk explain flow.yaml --json
twerk dry-run flow.yaml --example basic --json
twerk run flow.yaml --example basic --jsonl
twerk inspect <run_id> --json
twerk bundle <run_id> --json
```

## Top-Level Commands

```bash
twerk validate flow.yaml
twerk explain flow.yaml
twerk graph flow.yaml
twerk dry-run flow.yaml --example basic
twerk run flow.yaml --input input.json
twerk watch <run_id>
twerk inspect <run_id>
twerk events <run_id>
twerk logs <run_id>
twerk trace <run_id>
twerk replay <run_id>
twerk bundle <run_id>
twerk actions list
twerk actions show github.issue.comment
twerk schema workflow
twerk test flow.yaml
twerk doctor
twerk serve
```

## Universal Flags

Every command should support the relevant subset of these flags:

```bash
--json
--jsonl
--pretty
--no-color
--quiet
--verbose
--trace
--redact
--no-redact
--output <path>
--fail-fast
--include <section>
--exclude <section>
```

Defaults:

- Human terminals default to `--pretty`.
- Non-TTY contexts should prefer plain output unless `--pretty` is explicit.
- `--redact` is default.
- `--no-redact` is admin-only or local-debug-only.

Most important AI flags:

```bash
--json
--jsonl
--trace
--include diagnostics,graph,events,outputs,logs
```

## Exit Codes

Exit codes must be stable.

| Code | Meaning |
|---:|---|
| `0` | Success. |
| `1` | Runtime failure. |
| `2` | Validation failure. |
| `3` | Input mapping or type failure. |
| `4` | Secret, permission, or capability failure. |
| `5` | Queue or backpressure failure. |
| `6` | Timeout. |
| `7` | Cancelled. |
| `10` | Internal runtime error. |

## Machine Output Rules

`--json` emits one JSON document.

`--jsonl` emits one JSON object per line.

Machine output must:

- Never include ANSI color.
- Never include progress spinners.
- Never include prompts.
- Include stable field names.
- Include `schema_version` where the shape may evolve.
- Include `workflow`, `run_id`, and `step` identifiers where applicable.
- Redact secrets unless `--no-redact` is explicitly authorized.
- Preserve native value types.
- Include `source` paths and line/column data for validation diagnostics when available.

## `twerk validate`

Purpose: parse and validate a workflow before execution.

```bash
twerk validate flow.yaml
twerk validate flow.yaml --json
```

Human output should prioritize actionable diagnostics.

Machine output should return all errors where practical:

```json
{
  "schema_version": "twerk.validate/v1",
  "ok": false,
  "workflow": "issue_triage",
  "errors": [
    {
      "code": "UNKNOWN_REFERENCE",
      "message": "Step 'respond' references missing output '$classify.label'",
      "path": "steps[2].with.body.label",
      "line": 31,
      "column": 16,
      "severity": "error",
      "suggestion": "Did you mean '$classify.body.label'?"
    }
  ],
  "warnings": [],
  "summary": {
    "steps": 4,
    "actions": 2,
    "secrets": ["github_token"]
  }
}
```

Validation phases should be visible in `--trace`:

```text
parse, schema, names, references, types, control_flow, secrets, action_contracts, limits
```

## `twerk explain`

Purpose: explain what the workflow will do without running it.

```bash
twerk explain flow.yaml
twerk explain flow.yaml --json
```

It should answer:

- What starts this workflow?
- What inputs are required?
- What secrets are needed?
- What actions can run?
- What can retry?
- What can wait or ask?
- What can fail?
- What result is returned?

Example machine output:

```json
{
  "schema_version": "twerk.explain/v1",
  "workflow": "issue_triage",
  "trigger": {
    "kind": "webhook",
    "path": "/github",
    "method": "POST"
  },
  "inputs": [
    {
      "name": "body",
      "from": "request.body",
      "is": "object",
      "required": true
    }
  ],
  "steps": [
    {
      "id": "classify",
      "primitive": "do",
      "action": "ai.classify",
      "depends_on": ["title"],
      "may_retry": true,
      "may_fail": true,
      "outputs": ["label", "confidence"]
    }
  ],
  "result": {
    "label": "$classify.label"
  }
}
```

## `twerk graph`

Purpose: expose the compiled graph and visual topology.

```bash
twerk graph flow.yaml --format mermaid
twerk graph flow.yaml --format dot
twerk graph flow.yaml --json
```

JSON output:

```json
{
  "schema_version": "twerk.graph/v1",
  "workflow": "issue_triage",
  "nodes": [
    {
      "id": "title",
      "kind": "set",
      "line": 18
    },
    {
      "id": "classify",
      "kind": "do",
      "action": "ai.classify",
      "line": 23
    }
  ],
  "edges": [
    {
      "from": "title",
      "to": "classify",
      "kind": "success"
    }
  ]
}
```

Graph edge kinds:

```text
success, error, choose, otherwise, branch, iteration, retry, wait_resume, ask_resume, finish
```

## `twerk dry-run`

Purpose: resolve inputs and planned execution without side effects.

```bash
twerk dry-run flow.yaml --example basic
twerk dry-run flow.yaml --input input.json
twerk dry-run flow.yaml --example basic --json
```

Dry run must show:

- Input mapping results.
- Type validation results.
- Steps that would run.
- Steps that would skip.
- Resolved `with` values.
- Actions that would produce side effects.
- Redacted secret use.
- Planned retries, waits, asks, loops, pagination, and parallel branches.

Example output:

```json
{
  "schema_version": "twerk.dry_run/v1",
  "ok": true,
  "mode": "dry_run",
  "input": {
    "body": {
      "issue": {
        "title": "Bug report"
      }
    }
  },
  "plan": [
    {
      "step": "title",
      "would_run": true,
      "resolved": {
        "set": {
          "text": "Bug report"
        }
      }
    },
    {
      "step": "classify",
      "would_run": true,
      "action": "ai.classify",
      "side_effect": false,
      "skipped_reason": "dry_run"
    }
  ]
}
```

## `twerk run`

Purpose: start a workflow run.

```bash
twerk run flow.yaml --input input.json
twerk run flow.yaml --input input.json --watch
twerk run flow.yaml --input input.json --json
twerk run flow.yaml --input input.json --jsonl
```

Human output:

```text
Run accepted: run_01h
Workflow: issue_triage
Digest: sha256:...

✓ title        2ms
✓ classify    184ms
✓ respond     18ms

Succeeded in 221ms
```

AI streaming output:

```jsonl
{"event":"run.accepted","run_id":"run_01h","workflow":"issue_triage","digest":"sha256:..."}
{"event":"step.started","run_id":"run_01h","step":"title","attempt":1}
{"event":"step.succeeded","run_id":"run_01h","step":"title","duration_ms":2,"output":{"text":"Bug report"}}
{"event":"step.started","run_id":"run_01h","step":"classify","attempt":1}
{"event":"step.succeeded","run_id":"run_01h","step":"classify","duration_ms":184,"output":{"label":"bug","confidence":0.91}}
{"event":"run.succeeded","run_id":"run_01h","duration_ms":221,"result":{"label":"bug"}}
```

Accepted means durable. If `twerk run` returns success with a run ID, the run was persisted before acknowledgement.

## `twerk watch`

Purpose: watch a run in progress.

```bash
twerk watch run_01h
twerk watch run_01h --jsonl
```

`watch` should stream the same event model as `run --watch --jsonl`.

It should be reconnect-safe. If the client reconnects, it can resume from a sequence number:

```bash
twerk watch run_01h --from-seq 42 --jsonl
```

## `twerk inspect`

Purpose: return complete run state.

```bash
twerk inspect run_01h
twerk inspect run_01h --json
```

This is the AI truth source for a run.

```json
{
  "schema_version": "twerk.inspect/v1",
  "run_id": "run_01h",
  "workflow": {
    "name": "issue_triage",
    "version": "twerk/v1",
    "digest": "sha256:..."
  },
  "status": "succeeded",
  "started_at": "2026-04-28T12:00:00Z",
  "finished_at": "2026-04-28T12:00:01Z",
  "input": {},
  "steps": [
    {
      "id": "classify",
      "status": "succeeded",
      "attempts": 1,
      "started_at": "2026-04-28T12:00:00Z",
      "finished_at": "2026-04-28T12:00:00Z",
      "resolved_with": {
        "text": "Bug report"
      },
      "output": {
        "label": "bug",
        "confidence": 0.91
      },
      "logs": [],
      "error": null
    }
  ],
  "result": {
    "label": "bug"
  }
}
```

Optional include filters:

```bash
twerk inspect run_01h --include steps,outputs,errors --json
twerk inspect run_01h --exclude logs --json
```

## `twerk events`

Purpose: expose the durable event journal.

```bash
twerk events run_01h
twerk events run_01h --jsonl
twerk events run_01h --from-seq 12 --jsonl
```

Example:

```jsonl
{"seq":1,"type":"RunAccepted","run_id":"run_01h"}
{"seq":2,"type":"RunStarted","run_id":"run_01h"}
{"seq":3,"type":"StepStarted","step":"title","attempt":1}
{"seq":4,"type":"StepSucceeded","step":"title"}
```

Events must be stable enough for debugging and AI inspection, but they are not the public workflow language.

## `twerk logs`

Purpose: show structured logs for a run or step.

```bash
twerk logs run_01h
twerk logs run_01h --step classify
twerk logs run_01h --jsonl
```

Example:

```jsonl
{"level":"info","run_id":"run_01h","step":"classify","message":"calling action ai.classify"}
{"level":"error","run_id":"run_01h","step":"classify","code":"ACTION_FAILED","message":"timeout"}
```

Log records must include correlation fields where available:

```text
run_id, workflow, step, attempt, action, event_seq, trace_id
```

## `twerk trace`

Purpose: show timing, storage, queueing, retries, and action latency.

```bash
twerk trace run_01h
twerk trace run_01h --json
```

Example:

```json
{
  "schema_version": "twerk.trace/v1",
  "run_id": "run_01h",
  "timeline": [
    {
      "at": "2026-04-28T12:00:00.001Z",
      "kind": "queue.wait",
      "duration_ms": 3
    },
    {
      "at": "2026-04-28T12:00:00.004Z",
      "kind": "step.action",
      "step": "classify",
      "duration_ms": 184
    },
    {
      "at": "2026-04-28T12:00:00.188Z",
      "kind": "fjall.commit",
      "duration_ms": 1
    }
  ],
  "summary": {
    "total_ms": 221,
    "action_ms": 202,
    "scheduler_ms": 5,
    "storage_ms": 3
  }
}
```

Trace kinds should include:

```text
queue.wait, scheduler.ready, step.resolve, step.action, step.output_validate,
retry.sleep, wait.sleep, ask.pending, fjall.commit, fjall.read, result.evaluate
```

## `twerk replay`

Purpose: rerun a workflow from a controlled point.

```bash
twerk replay run_01h
twerk replay run_01h --from-step classify
twerk replay run_01h --json
```

Replay must be explicit about what is reused versus rerun:

```json
{
  "schema_version": "twerk.replay/v1",
  "source_run": "run_01h",
  "new_run": "run_01j",
  "reused_steps": ["title"],
  "rerun_steps": ["classify", "respond"],
  "reason": "from-step classify"
}
```

Replay must never silently reuse or rerun side effects. The command should require confirmation in human mode and explicit flags in machine mode when rerunning side-effecting steps.

## `twerk bundle`

Purpose: emit everything an AI or human needs to debug a workflow or run.

```bash
twerk bundle run_01h --output debug-bundle.json
twerk bundle run_01h --json
twerk bundle flow.yaml --example basic --json
```

Bundle shape:

```json
{
  "schema_version": "twerk.bundle/v1",
  "workflow_yaml": "...",
  "workflow_digest": "sha256:...",
  "validation": {},
  "compiled_graph": {},
  "action_contracts": {},
  "input": {},
  "events": [],
  "steps": [],
  "logs": [],
  "trace": {},
  "errors": [],
  "suggested_fixes": []
}
```

`bundle` is the one-command AI context export.

## `twerk actions`

Purpose: expose action registry contracts.

```bash
twerk actions list
twerk actions list --json
twerk actions show ai.classify --json
twerk actions test ai.classify --with input.json --json
```

Action contract output:

```json
{
  "schema_version": "twerk.action/v1",
  "name": "ai.classify",
  "title": "Classify text",
  "description": "Classifies text into a label and confidence score.",
  "inputs": {
    "text": "text"
  },
  "outputs": {
    "label": "text",
    "confidence": "number"
  },
  "secrets": [],
  "retry": {
    "safe": true
  },
  "side_effect": false
}
```

Action metadata should include UI hints:

```json
{
  "ui": {
    "category": "AI",
    "icon": "spark",
    "color": "purple"
  }
}
```

## `twerk schema`

Purpose: emit schemas for tooling and AI generation.

```bash
twerk schema workflow
twerk schema action
twerk schema run
twerk schema workflow --json
```

Schemas should be versioned and stable. They should be suitable for editor completion, frontend form generation, and AI validation.

## `twerk test`

Purpose: run executable examples from workflow YAML.

```bash
twerk test flow.yaml
twerk test flow.yaml --example basic
twerk test flow.yaml --json
```

Test output:

```json
{
  "schema_version": "twerk.test/v1",
  "workflow": "issue_triage",
  "ok": false,
  "examples": [
    {
      "name": "bug_report",
      "status": "failed",
      "expected": {
        "result": {
          "label": "bug"
        }
      },
      "actual": {
        "result": {
          "label": "support"
        }
      },
      "diff": [
        {
          "path": "result.label",
          "expected": "bug",
          "actual": "support"
        }
      ]
    }
  ]
}
```

## `twerk doctor`

Purpose: verify local runtime health.

```bash
twerk doctor
twerk doctor --json
```

Checks:

- Fjall database access
- Action registry load
- Secrets backend access
- Webhook listener config
- Disk space
- File permissions
- Runtime config
- Schema version support
- Clock sanity
- Port availability

Example:

```json
{
  "schema_version": "twerk.doctor/v1",
  "ok": true,
  "checks": [
    {
      "name": "fjall_database",
      "status": "ok",
      "details": {
        "path": ".twerk/db"
      }
    }
  ]
}
```

## `twerk serve`

Purpose: start the single-binary runtime.

```bash
twerk serve
twerk serve --addr 127.0.0.1:8787
twerk serve --jsonl
```

Structured server events:

```jsonl
{"event":"server.started","addr":"127.0.0.1:8787"}
{"event":"workflow.loaded","name":"issue_triage","digest":"sha256:..."}
{"event":"webhook.accepted","workflow":"issue_triage","run_id":"run_01h"}
{"event":"queue.full","workflow":"issue_triage","code":"QUEUE_FULL"}
```

Server mode must expose the same observability model as CLI run mode.

## JSONL Event Stream

All JSONL events should include:

```json
{
  "event": "step.succeeded",
  "seq": 12,
  "time": "2026-04-28T12:00:00Z",
  "run_id": "run_01h",
  "workflow": "issue_triage"
}
```

Common event names:

```text
run.accepted
run.queued
run.started
run.waiting
run.asking
run.succeeded
run.failed
run.cancelled
step.pending
step.skipped
step.started
step.retrying
step.waiting
step.asking
step.succeeded
step.failed
step.cancelled
collect.page.started
collect.page.succeeded
collect.limit_reached
repeat.attempt.started
repeat.attempt.succeeded
together.branch.started
together.branch.succeeded
together.branch.failed
fjall.commit
queue.full
```

## Observability Contract

Every run must expose:

- Workflow snapshot
- Workflow digest
- Compiled graph
- Input mapping
- Resolved step inputs
- Redacted secret usage
- Step outputs
- State transitions
- Attempts and retries
- Waits and asks
- Loop, collect, reduce, and repeat state
- Parallel branch state
- Partial failure data
- Logs
- Timings
- Errors
- Final result

## Error Diagnostics

Every validation and runtime diagnostic should include:

```json
{
  "code": "UNKNOWN_REFERENCE",
  "message": "Unknown reference '$classfy.label'",
  "path": "steps[2].with.label",
  "line": 24,
  "column": 14,
  "severity": "error",
  "retryable": false,
  "suggestion": "Did you mean '$classify.label'?",
  "docs": "https://twerk.dev/docs/errors/UNKNOWN_REFERENCE"
}
```

AI agents need codes and source paths more than prose.

## Redaction Rules

Redaction must apply to:

- Logs
- Errors
- Traces
- Events
- Inspection output
- Debug bundles
- Example output
- UI previews

Redacted values should retain shape when possible:

```json
{
  "token": "[REDACTED:secret.github_token]"
}
```

Secret-tainted derived values should be marked:

```json
{
  "authorization_header": "[REDACTED:tainted]"
}
```

## AI Repair Loop

The CLI should make this repair loop reliable:

```text
AI writes YAML.
AI runs validate --json.
AI reads diagnostics.
AI edits YAML.
AI runs dry-run --json.
AI runs test --json.
AI runs run --jsonl.
AI inspects run --json.
AI exports bundle if anything fails.
```

The CLI must never require a human-only UI to understand a workflow failure.

## Non-Interactive Safety

Commands that can perform side effects must be safe for automation.

Rules:

- Machine mode must not prompt.
- Potentially destructive operations require explicit flags.
- Rerunning side-effecting steps requires explicit acknowledgment flags.
- `--jsonl` streams must be flush-friendly.
- Errors must be emitted as JSON in machine mode even on failure.

Example:

```bash
twerk replay run_01h --from-step charge_card --allow-side-effects --json
```

Without the explicit flag, replay should fail with a structured error.

## Open Questions

- Should `twerk run --dry` be an alias for `twerk dry-run`?
- Should `twerk bundle` include source code for local actions or only contracts?
- Should `twerk inspect` default to omitting logs for large runs?
- Should event journal names match internal Rust enum names or stable public names?
- Should `--no-redact` require an environment variable or config policy in addition to the flag?
