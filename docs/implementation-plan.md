# Twerk Implementation Plan

Status: draft

Audience: product owners, implementers, QA agents, and AI coding agents.

## Purpose

This plan turns the Twerk architecture docs into an implementation sequence. The goal is to build a small, fast, correct single-binary runtime before expanding integrations.

The implementation strategy is:

```text
Strict language -> validator -> action registry -> compiled IR -> Fjall runtime -> CLI observability -> server ingress -> UI builder
```

Do not start with a broad integration catalog. Start with correctness, durability, observability, and performance.

## Phase 0: Repository And Project Skeleton

Goal: create the executable and core crates/modules.

Deliverables:

- `twerk` CLI binary.
- Core library for language model.
- Validator module.
- Action registry module.
- Runtime module.
- Fjall storage module.
- CLI output helpers.
- Test fixture directory for workflows.

Acceptance criteria:

- `twerk --help` works.
- `twerk doctor --json` reports version and storage availability.
- CI runs format, lint, and tests.
- No runtime behavior depends on shell actions.

## Phase 1: YAML Parser And Validator

Goal: reject bad workflows before execution.

Implement:

- Restricted YAML profile.
- Duplicate-key rejection.
- Closed top-level and step schemas.
- Name/ID validation.
- One-primitive-per-step rule.
- Input/type schema normalization.
- Reference parser.
- Expression parser.
- Control-flow graph builder.
- Diagnostics with line/column and JSON pointers.

CLI:

```bash
twerk validate flow.yaml
twerk validate flow.yaml --json
twerk explain flow.yaml --json
twerk graph flow.yaml --json
```

Acceptance criteria:

- Invalid YAML returns exit code `2`.
- Duplicate keys are rejected.
- Unknown fields are rejected.
- All diagnostics include `code`, `message`, and source location when available.
- Golden tests cover every validation error code in `docs/validator-contract.md`.

## Phase 2: Built-In Action Registry

Goal: make `do` real without arbitrary shell.

Implement built-in actions:

| Action | Purpose |
|---|---|
| `http.get` | HTTP GET with typed query/header/body output. |
| `http.post` | HTTP POST with typed JSON body. |
| `json.pick` | JSON Pointer/path extraction. |
| `json.merge` | Deterministic object merge. |
| `text.template` | Bounded text rendering. |
| `webhook.reply` | Webhook response action. |
| `log.info` | Structured log event. |
| `system.noop` | Test no-op action. |

Defer:

- `shell.run`
- Docker/Kubernetes actions
- Large provider catalogs

CLI:

```bash
twerk actions list --json
twerk actions show http.post --json
twerk actions test http.post --input input.json --dry-run
```

Acceptance criteria:

- Every action has manifest, input schema, output schema, retry safety, capabilities, timeout, and mock behavior.
- Validator rejects unknown `with` fields.
- Dry-run can mock every built-in action.

## Phase 3: Compiled IR

Goal: parse and validate once, run many times.

Implement compiled IR:

- Stable workflow digest.
- Stable compiled IR digest.
- Step table with numeric indexes.
- Edge table.
- Input mapping plan.
- Expression bytecode or AST.
- Action bindings.
- Output schemas.
- Capability summary.
- Source location map.

Acceptance criteria:

- Same workflow normalizes to same digest independent of YAML formatting.
- IR serialization is deterministic.
- `twerk explain --json` can expose IR summary without leaking secrets.

## Phase 4: Fjall Storage Layer

Goal: create durable event and projection storage.

Implement keyspaces from `docs/storage-model.md`:

- `workflow_snapshots`
- `workflow_index`
- `runs`
- `run_status_index`
- `run_events`
- `step_states`
- `attempts`
- `step_outputs`
- `errors`
- `logs`
- `traces`
- `timers`
- `asks`
- `dedupe`
- structured primitive state keyspaces
- `payload_blobs`

Acceptance criteria:

- One atomic batch can append event, update run projection, update step projection, and enqueue work.
- Big-endian keys preserve scan order.
- Crash/restart recovery tests pass.
- Storage refuses oversized keys/values before Fjall errors leak to users.

## Phase 5: Runtime Admission And Scheduler

Goal: accepted means durable.

Implement:

- Manual run admission.
- Input mapping.
- Secret availability check by metadata.
- Queue capacity checks.
- Durable run creation batch.
- Scheduler claim loop.
- Step state machine.
- Attempt lease records.
- Completion/failure transitions.
- Cancellation request handling.

CLI:

```bash
twerk run flow.yaml --input input.json --jsonl
twerk inspect <run_id> --json
twerk events <run_id> --jsonl
```

Acceptance criteria:

- If `run` returns a run ID, `inspect` can find it after process restart.
- Completed steps do not rerun during normal replay.
- Incomplete side-effecting attempts can be retried at least once.
- Queue full returns `QUEUE_FULL` and does not accept a run.

## Phase 6: Core Primitives

Goal: support the language primitives with durable state.

Implement in order:

1. `set`
2. `do`
3. `finish`
4. `try_again`
5. `on_error`
6. `choose`
7. `together`
8. `for_each`
9. `wait` time waits
10. `collect`
11. `reduce`
12. `repeat`
13. `ask`
14. `wait` event waits

Acceptance criteria:

- Every primitive has unit tests, golden event tests, and restart recovery tests.
- `together` output order is deterministic.
- `for_each` output order matches input order.
- `collect`, `reduce`, and `repeat` enforce hard limits.
- `on_error` sees `$error` and cannot recurse into itself.

## Phase 7: CLI Observability

Goal: make runs fully inspectable by humans and AI.

Implement commands from `docs/cli-observability.md`:

- `validate`
- `explain`
- `graph`
- `dry-run`
- `run`
- `watch`
- `inspect`
- `events`
- `logs`
- `trace`
- `replay`
- `bundle`
- `actions`
- `schema`
- `test`
- `doctor`
- `serve`

Acceptance criteria:

- Every command has `--json` or `--jsonl` when useful.
- Exit codes match `docs/cli-observability.md`.
- Redaction is enabled by default.
- Debug bundles contain enough context for AI repair without raw secrets.

## Phase 8: Server Mode

Goal: support webhooks, event ingestion, ask answers, and UI access.

Implement endpoints from `docs/server-api.md`:

- run creation and inspection
- JSONL run watch
- workflow webhooks
- event submission
- ask answer endpoints
- action/workflow discovery
- queue/health endpoints

Acceptance criteria:

- Webhook `unique` dedupe returns existing run ID.
- Queue full returns `503 QUEUE_FULL`.
- Accepted HTTP run creation means durable.
- API auth and capability checks are enforced.

## Phase 9: Security And Policy

Goal: prevent ambient authority.

Implement:

- Capability policy engine.
- Secret alias binding metadata.
- Secret taint tracking.
- Redaction pipeline.
- Webhook HMAC verification.
- Egress policy.
- File policy.
- Shell/process deny-by-default.
- Audit events.

Acceptance criteria:

- Secret values never appear in normal logs, traces, errors, or result.
- `shell.run` is disabled by default in server mode.
- Policy-denied actions fail before execution.
- Audit events exist for secret resolution, denied capability, ask answer, cancel, replay, and no-redact access.

## Phase 10: UI Builder

Goal: build Step Functions-style authoring and run inspection over YAML.

Implement in stages:

1. Read-only graph from YAML.
2. Validation panel with source-linked diagnostics.
3. Action palette from registry.
4. Inspector forms from schemas.
5. Drag/drop sequential steps.
6. Builders for `choose`, `together`, `for_each`, `collect`, `reduce`, `repeat`, `wait`, `ask`.
7. Side-by-side YAML editing.
8. Run inspector overlay.
9. AI repair/generate actions.

Acceptance criteria:

- UI writes valid YAML only.
- No UI-only workflow semantics exist.
- Layout sidecar contains presentation only.
- Data picker never suggests unavailable or secret-forbidden values.

## Phase 11: Performance Pass

Goal: make Twerk a throughput monster.

Measure:

- Validation latency.
- Compile latency.
- Manual run admission throughput.
- Webhook admission throughput.
- Scheduler step throughput.
- Fjall batch latency.
- Restart recovery time.
- JSONL streaming overhead.
- Memory per active run.

Targets should be set after first benchmarks, but architecture constraints are fixed:

- Parse once.
- Validate once.
- Compile once.
- Use in-memory compiled IR.
- Persist state transitions in atomic Fjall batches.
- Group commits when durability mode allows.
- Avoid process starts for core primitives.
- Avoid JS engines in the hot path.
- Use bounded queues everywhere.

Acceptance criteria:

- Benchmarks are reproducible.
- Performance regressions fail CI after baselines are established.
- Trace output identifies scheduler, storage, action, and serialization time separately.

## Phase 12: Provider Integrations

Goal: expand usefulness without compromising the core.

Add integrations only after action contracts are stable.

Suggested order:

1. GitHub issues/comments/status.
2. Slack messages.
3. Email send.
4. S3/object storage metadata actions.
5. Common AI provider HTTP-native actions.
6. Database query actions with strict safety.

Acceptance criteria:

- Each provider action has manifest, tests, dry-run mock, and security capabilities.
- Provider actions do not require shell.
- Provider actions declare idempotency and retry safety.

## Phase 13: Hardening

Goal: make v1 shippable.

Required hardening:

- Fuzz YAML parser and expression parser.
- Property-test control-flow graph validation.
- Crash-test Fjall write groups.
- Chaos-test scheduler leases.
- Mutation-test validator diagnostics.
- Red-team secret redaction.
- Load-test webhook admission.
- Soak-test waits and asks.
- Verify backup/restore.

Acceptance criteria:

- No known secret leakage path.
- No accepted run can disappear after restart.
- No unbounded queue or loop exists.
- Every runtime error has stable code and inspection path.
- Docs examples pass `twerk test` once implementation exists.

## Cut Lines

Do not include in v1 unless the core is already stable:

- Distributed workers.
- RabbitMQ/Postgres.
- Docker/Kubernetes default runtime.
- Arbitrary JS/Python expressions.
- Freeform n8n-style canvas semantics.
- Unbounded pagination.
- Arbitrary graph cycles.
- Multi-trigger workflows.
- Mutable global variables.
- Exactly-once external side effects claim.

## First Vertical Slice

The first end-to-end slice should be:

```text
manual trigger -> inputs -> set -> http.post mock -> result -> inspect -> events -> bundle
```

Minimum workflow:

```yaml
version: twerk/v1
name: first_slice

when:
  manual: {}

inputs:
  message: text

steps:
  - id: payload
    set:
      text: $input.message

  - id: send
    do: http.post
    with:
      url: https://example.com/echo
      json:
        text: $payload.text

result:
  status: $send.status
```

This proves the complete contract without shell, UI, provider sprawl, or server complexity.

## Done Definition For v1

Twerk v1 is done when:

- Language spec is implemented and validator-compatible.
- CLI validates, dry-runs, runs, inspects, traces, replays, and bundles.
- Runtime uses Fjall for durable event/state storage.
- Accepted means durable.
- Webhook and manual triggers work.
- Core primitives work with bounded semantics.
- Built-in actions cover HTTP, JSON, text, log, webhook reply, and no-op.
- Security policy denies shell by default.
- UI can build and inspect workflows without hidden semantics.
- Examples in `docs/examples.md` pass as executable fixtures or are explicitly marked as integration-only.
