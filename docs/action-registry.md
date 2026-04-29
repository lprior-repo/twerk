# Twerk Action Registry Contract

Status: draft

Audience: action authors, runtime implementers, validator implementers, UI builders, and AI agents.

## Purpose

The workflow language uses `do` to invoke registered actions. This document defines what an action is so validators, runtimes, CLIs, UIs, and AI agents do not guess.

An action is a versioned, typed, capability-declared unit of work with a stable name, input schema, output schema, security policy, retry behavior, and UI metadata.

## Core Requirements

Every action must declare:

- Name
- Version
- Title and description
- Input schema
- Output schema
- Secret requirements
- Side-effect classification
- Retry safety
- Idempotency behavior
- Timeout behavior
- Capability requirements
- Mock behavior for tests and dry runs
- UI metadata

## Naming

Action names use dotted lowercase identifiers.

```text
namespace.verb
namespace.resource.verb
```

Examples:

```text
http.get
http.post
json.pick
file.write
github.issue.comment
slack.message.send
ai.classify
shell.run
```

Rules:

- Names must match `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$`.
- Names are globally unique within the loaded registry.
- Built-in actions should use stable namespaces such as `http`, `json`, `file`, `webhook`, `ai`, and `system`.
- Unsafe actions must be obvious, for example `shell.run` or `process.exec`.

## Action Manifest

Canonical manifest shape:

```yaml
version: twerk/action/v1
name: github.issue.comment
action_version: 1.0.0
title: Add GitHub comment
description: Adds a comment to a GitHub issue or pull request.

inputs:
  is: object
  fields:
    token:
      is: text
      secret: true
    repo:
      is: text
    issue:
      is: number
    body:
      is: text
      max_length: 65536
  extra: reject

outputs:
  is: object
  fields:
    id: text
    url: text
  extra: reject

secrets:
  github_token:
    required: true
    accepted_inputs:
      - token

side_effect: external_write
retry:
  safety: requires_idempotency_key
  retryable_errors:
    - Http.Timeout
    - Http.RateLimited

idempotency:
  required: true
  field: idempotency_key
  default: "$run.id:$step.id"

timeout:
  default: 30s
  max: 2m

capabilities:
  - network.github
  - secrets.read.github_token

mock:
  mode: deterministic
  output:
    id: mock_comment
    url: https://example.invalid/comment/mock_comment

ui:
  category: GitHub
  icon: github
  color: gray
  recommended: true
```

## Input Schema

Action input schemas use the same canonical schema model as workflow inputs.

Supported fields:

```text
is, of, fields, extra, optional, nullable, default, enum, min, max,
min_length, max_length, pattern, secret, description
```

Rules:

- `with` must validate against the selected action input schema.
- Unknown `with` fields are rejected unless `extra: allow` is declared.
- Missing required fields are validation errors when statically knowable.
- Runtime-resolved values are validated before action execution.
- Secret-typed fields may receive secret-tainted values.
- Non-secret fields must reject secret-tainted values unless the action explicitly allows taint.

## Output Schema

Action outputs must be objects.

If the underlying implementation returns a scalar, the runtime wraps it:

```json
{ "value": "scalar" }
```

Rules:

- Output is validated before it is exposed to downstream steps.
- Output validation failure fails the step.
- Output fields may be marked `secret: true` or `tainted: true`.
- Secret output fields are redacted in logs, traces, events, bundles, and UI.
- Output size limits apply before redaction.

## Side-Effect Classification

Actions must declare one side-effect class.

| Class | Meaning | Examples |
|---|---|---|
| `pure` | Deterministic, no I/O. | `json.pick`, `template.render` |
| `local_read` | Reads local state only. | `file.read` |
| `local_write` | Mutates local state. | `file.write` |
| `external_read` | Reads external systems. | `http.get`, `github.issue.get` |
| `external_write` | Mutates external systems. | `http.post`, `slack.message.send` |
| `process` | Spawns a process. | `process.exec` |
| `unsafe_shell` | Runs through a shell. | `shell.run` |

Validators and UIs should use this field to warn about retries, replay, and security.

## Retry Safety

Action retry safety values:

| Value | Meaning |
|---|---|
| `idempotent` | Safe to retry without extra key. |
| `requires_idempotency_key` | Safe only with stable key. |
| `not_retry_safe` | Retry is rejected unless runtime policy overrides. |
| `unknown` | Treat as not retry safe. |

Rules:

- `try_again` on `not_retry_safe` or `unknown` actions is a validation error by default.
- `requires_idempotency_key` actions must receive or derive a stable key.
- A retry after unknown completion must reuse the same idempotency key.
- Retry safety must be visible through `twerk actions show`.

## Idempotency

Idempotency declaration:

```yaml
idempotency:
  required: true
  field: idempotency_key
  default: "$run.id:$step.id:$attempt.group"
```

Rules:

- Idempotency keys are stable across retry attempts of the same logical step execution.
- Keys may include run ID, step ID, workflow digest, trigger unique key, and item index.
- Keys must not include attempt number unless the action explicitly wants per-attempt uniqueness.
- Keys must not contain secret values.

## Timeout Behavior

Actions declare default and maximum timeout.

```yaml
timeout:
  default: 30s
  max: 5m
```

Rules:

- Runtime enforces the smaller of workflow, action, and global timeout limits.
- Timeout produces a structured action error.
- Timeout cancellation is best-effort for actions that cannot be interrupted.

## Capabilities

Capabilities are explicit permissions required by an action.

Examples:

```text
network.any
network.github
filesystem.read
filesystem.write
process.spawn
shell.run
secrets.read.github_token
webhook.reply
```

Rules:

- Runtime policy grants or denies capabilities.
- Workflows may declare required capabilities in future versions, but action manifests are the source of truth in v1.
- Denied capability fails validation if known before execution, otherwise fails before action execution.

## Secrets

Secrets are declared by logical name and accepted input fields.

```yaml
secrets:
  api_key:
    required: true
    accepted_inputs:
      - token
      - headers.Authorization
```

Rules:

- Actions cannot read arbitrary secrets.
- Secret access must be traceable to a declared action input or action binding.
- Secret values must not appear in action errors.

## Mock And Test Behavior

Actions must define test behavior for `twerk test` and `twerk dry-run`.

Mock modes:

| Mode | Meaning |
|---|---|
| `none` | No mock; dry-run only shows planned side effect. |
| `deterministic` | Returns declared deterministic output. |
| `fixture` | Reads named fixture data. |
| `contract_only` | Validates inputs but produces no output. |

Rules:

- Tests must not call external side-effecting systems by default.
- Mock outputs must validate against output schema.
- Mock outputs may not contain real secrets.

## Local And Custom Actions

Custom actions may be registered from:

- Built-in registry
- Project action manifest directory
- Runtime config
- Signed action package
- Native Rust plugin if runtime policy allows it
- HTTP action binding
- WASM component if runtime policy allows it

Recommended project layout:

```text
.twerk/actions/
  slack.send.yaml
  customer.lookup.yaml
```

Registration rules:

- Duplicate names are rejected unless version selection disambiguates them.
- Local action manifests are validated at startup and by `twerk actions list`.
- Unsafe action kinds require explicit runtime capability approval.

## Action Versions

Actions have semantic versions independent of workflow language version.

```yaml
action_version: 1.2.0
```

Rules:

- Breaking input/output schema changes require a major version bump.
- Workflow snapshots bind to the resolved action version used at run admission.
- In-flight runs resume with the original action version or fail with `ACTION_VERSION_UNAVAILABLE` if runtime policy cannot provide it.
- `twerk explain` and `twerk bundle` include resolved action versions.

## UI Metadata

UI metadata is non-semantic.

```yaml
ui:
  category: HTTP
  icon: globe
  color: blue
  recommended: true
  advanced: false
  unsafe: false
```

Rules:

- UI metadata must not affect execution.
- Shell/process actions should set `advanced: true` and `unsafe: true`.
- Forms are generated from schemas, not hand-coded UI assumptions.

## CLI Contract

Required commands:

```bash
twerk actions list --json
twerk actions show <action> --json
twerk actions test <action> --with input.json --json
```

`twerk actions show` must expose the full manifest after normalization and policy filtering.

## Validation Integration

The validator must use action contracts to check:

- Unknown `do` action names
- Unknown `with` fields
- Missing required inputs
- Type mismatches
- Secret usage
- Retry safety
- Capability availability when known
- Output reference existence where schema makes it knowable

## Open Questions

- Should custom actions be allowed to define new primitive-like UI containers, or only `do` actions?
- Should WASM be v1 or later?
- Should action packages require signatures in local-only mode?
