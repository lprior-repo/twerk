# Twerk Server API Contract

Status: draft

Audience: server implementers, CLI implementers, UI builders, integration authors, and AI agents.

## Purpose

Twerk can run as a single-binary server for webhooks, event ingestion, UI access, and run inspection. This document defines the HTTP API contract for that server.

The API mirrors the CLI observability model. Anything visible through CLI should be reachable through the API when server mode is enabled and authorized.

## Principles

- JSON request and response bodies by default.
- JSONL streams for long-running event feeds.
- Stable error object shape.
- Redaction on by default.
- Accepted run means durable.
- Webhook/event ingestion must apply backpressure explicitly.
- API endpoints must never expose raw secrets.

## Base Path And Versioning

Recommended base path:

```text
/api/v1
```

Webhook trigger paths may live outside `/api/v1` if configured by workflow `when.webhook.path`.

## Authentication

Auth is runtime policy, but endpoints must declare required capability.

Common capabilities:

```text
runs.start
runs.read
runs.cancel
runs.replay
events.read
logs.read
traces.read
bundles.read
asks.answer
workflows.read
workflows.write
admin.health
admin.queue
```

Unauthenticated webhook endpoints are allowed only when the workflow trigger declares its own verification policy or runtime explicitly permits anonymous webhooks.

## Error Shape

All non-2xx API errors return:

```json
{
  "error": {
    "code": "QUEUE_FULL",
    "message": "Run queue is full",
    "retryable": true,
    "details": {}
  }
}
```

## Status Codes

| Status | Meaning |
|---:|---|
| `200` | Success. |
| `201` | Resource created. |
| `202` | Run accepted durably. |
| `204` | Success with no body. |
| `400` | Invalid request or input mapping failure. |
| `401` | Missing authentication. |
| `403` | Authenticated but not allowed. |
| `404` | Resource not found. |
| `409` | Duplicate or state conflict. |
| `413` | Payload too large. |
| `422` | Workflow validation or semantic error. |
| `429` | Rate limited. |
| `503` | Queue full or runtime unavailable. |

## Start Run

```http
POST /api/v1/runs
```

Request:

```json
{
  "workflow": "issue_triage",
  "input": {
    "body": {}
  },
  "idempotency_key": "optional-client-key"
}
```

Response `202`:

```json
{
  "run_id": "run_01h",
  "workflow": "issue_triage",
  "workflow_digest": "sha256:...",
  "status": "queued"
}
```

The run ID is returned only after durable admission commit.

## List Runs

```http
GET /api/v1/runs?workflow=issue_triage&status=running&limit=50&cursor=...
```

Response:

```json
{
  "runs": [
    {
      "run_id": "run_01h",
      "workflow": "issue_triage",
      "status": "succeeded",
      "created_at": "...",
      "finished_at": "..."
    }
  ],
  "next_cursor": null
}
```

## Inspect Run

```http
GET /api/v1/runs/{run_id}
```

Query options:

```text
include=steps,outputs,errors,logs,trace
exclude=logs
redact=true
```

Response shape matches `twerk inspect --json`.

## Watch Run Events

```http
GET /api/v1/runs/{run_id}/watch
Accept: application/x-ndjson
```

Query:

```text
from_seq=42
```

Response is JSONL:

```jsonl
{"seq":43,"event":"step.started","run_id":"run_01h","step":"classify"}
{"seq":44,"event":"step.succeeded","run_id":"run_01h","step":"classify"}
```

## Run Events

```http
GET /api/v1/runs/{run_id}/events?from_seq=0&limit=1000
```

Returns durable event records.

## Run Logs

```http
GET /api/v1/runs/{run_id}/logs?step=classify&level=error
```

Returns structured logs.

For streaming:

```http
GET /api/v1/runs/{run_id}/logs/stream
Accept: application/x-ndjson
```

## Run Trace

```http
GET /api/v1/runs/{run_id}/trace
```

Response shape matches `twerk trace --json`.

## Debug Bundle

```http
GET /api/v1/runs/{run_id}/bundle
```

Response shape matches `twerk bundle --json`.

Bundles must be redacted unless admin policy explicitly permits unredacted export.

## Cancel Run

```http
POST /api/v1/runs/{run_id}/cancel
```

Request:

```json
{
  "reason": "user_requested"
}
```

Response:

```json
{
  "run_id": "run_01h",
  "status": "cancelling"
}
```

If already terminal, return `409` with `RUN_ALREADY_TERMINAL`.

## Replay Run

```http
POST /api/v1/runs/{run_id}/replay
```

Request:

```json
{
  "from_step": "classify",
  "allow_side_effects": false
}
```

Response:

```json
{
  "source_run": "run_01h",
  "new_run": "run_01j",
  "reused_steps": ["title"],
  "rerun_steps": ["classify", "respond"]
}
```

Side-effecting replay requires explicit authorization and `allow_side_effects: true`.

## Webhook Trigger

Workflow-configured webhook path:

```http
POST /github
```

Behavior:

- Match request path and method to loaded workflow triggers.
- Validate webhook auth if configured.
- Evaluate `unique` key if configured.
- Admit run durably.
- Return `202` with run ID.

Accepted response:

```json
{
  "run_id": "run_01h",
  "status": "queued"
}
```

Duplicate response:

```json
{
  "duplicate": true,
  "run_id": "run_01h"
}
```

Queue full response uses `503 QUEUE_FULL`.

## Event Trigger

```http
POST /api/v1/events
```

Request:

```json
{
  "name": "customer.created",
  "id": "evt_123",
  "body": {}
}
```

Response:

```json
{
  "accepted": [
    {
      "workflow": "customer_welcome",
      "run_id": "run_01h"
    }
  ]
}
```

Event auth is required unless runtime policy permits anonymous event injection.

## Ask Prompts

List pending prompts:

```http
GET /api/v1/asks?status=pending
```

Get one prompt:

```http
GET /api/v1/runs/{run_id}/asks/{step_id}
```

Answer prompt:

```http
POST /api/v1/runs/{run_id}/asks/{step_id}/answer
```

Request:

```json
{
  "answer": "approve",
  "comment": "Looks good"
}
```

Response:

```json
{
  "run_id": "run_01h",
  "step_id": "approval",
  "status": "answered"
}
```

Responder identity comes from authentication, not request body.

## Workflows

List loaded workflows:

```http
GET /api/v1/workflows
```

Get workflow detail:

```http
GET /api/v1/workflows/{name}
```

Validate workflow source:

```http
POST /api/v1/workflows/validate
Content-Type: application/yaml
```

Register or update workflow if server policy permits it:

```http
PUT /api/v1/workflows/{name}
Content-Type: application/yaml
```

Workflow mutation endpoints require admin/write capability and must create new immutable snapshots.

## Actions

```http
GET /api/v1/actions
GET /api/v1/actions/{name}
```

Response shape matches `twerk actions list/show --json`.

## Queue Status

```http
GET /api/v1/queue
```

Response:

```json
{
  "run_ready": 12,
  "step_ready": 42,
  "timers": 5,
  "asks": 2,
  "limits": {
    "max_run_ready": 100000
  }
}
```

Requires admin capability.

## Health Checks

Liveness:

```http
GET /healthz
```

Readiness:

```http
GET /readyz
```

Detailed health:

```http
GET /api/v1/health
```

Detailed health requires admin capability and should include storage, queue, registry, and scheduler status.

## Redaction

All API responses redact secrets by default.

Admin-only unredacted requests, if enabled, must use:

```text
?redact=false
```

and require an explicit admin capability plus runtime policy approval.

## Open Questions

- Should workflow registration be available in v1 server mode or CLI/file-load only?
- Should streaming use Server-Sent Events or only JSONL?
- Should webhook paths share the same auth middleware as API endpoints or trigger-specific auth only?
