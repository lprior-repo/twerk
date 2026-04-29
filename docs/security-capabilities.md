# Twerk Security And Capabilities Contract

Status: draft

Audience: runtime implementers, validator implementers, action authors, UI builders, operators, and AI agents.

## Purpose

Twerk workflows can call external APIs, read and write local files, send webhooks, wait for human approval, and run optional process actions. This document defines the security model that keeps the simple YAML language from becoming an unbounded automation escape hatch.

The core rule is:

```text
No workflow, action, API caller, UI user, or AI agent gets ambient authority.
```

Every sensitive operation is guarded by an explicit capability, a declared action contract, a runtime policy check, and an audit event.

## Security Principles

- Deny by default.
- Secrets are declared, never embedded.
- Capabilities are checked before use, not after failure.
- Unsafe actions are opt-in and visibly labeled.
- Runtime inputs are untrusted until validated.
- Human answers are untrusted until authenticated and schema-checked.
- Logs, traces, errors, bundles, and UI previews redact by default.
- Accepted runs bind to immutable workflow snapshots and policy snapshots.
- Local single-binary operation does not imply local workflows can do anything.

## Trust Boundaries

Twerk has these trust boundaries:

| Boundary | Description |
|---|---|
| Workflow author | Person or AI that writes workflow YAML. |
| Workflow deployer | Principal allowed to register or update workflows. |
| Run invoker | Principal or trigger that starts runs. |
| Runtime operator | Principal allowed to configure policies and secrets. |
| Action author | Person or team that provides action implementation and manifest. |
| Human approver | Principal that answers `ask` prompts. |
| Inspector | Principal that can read run state, logs, traces, and bundles. |
| Local host | Filesystem, network, processes, environment, and OS credentials. |
| External services | HTTP APIs, webhooks, AI providers, email, GitHub, Slack, and similar systems. |

The workflow language is not a security boundary by itself. The validator and runtime policy engine enforce the boundary.

## Principal Model

Every security decision should be evaluated against a principal.

Principal fields:

```json
{
  "id": "user_123",
  "kind": "user",
  "display": "Ada Lovelace",
  "roles": ["workflow_admin"],
  "groups": ["platform"],
  "capabilities": ["workflows.write", "runs.read"],
  "source": "local"
}
```

Principal kinds:

| Kind | Meaning |
|---|---|
| `user` | Human user. |
| `service` | Service account or automation token. |
| `trigger` | Runtime-created principal for webhooks, schedules, and events. |
| `anonymous` | Unauthenticated caller allowed only by explicit policy. |
| `system` | Internal runtime principal. |

## Capability Names

Capabilities use dotted lowercase names.

Common capabilities:

```text
workflows.read
workflows.write
workflows.delete
workflows.validate
workflows.register
runs.start
runs.read
runs.cancel
runs.replay
events.submit
events.read
logs.read
traces.read
bundles.read
asks.read
asks.answer
actions.read
actions.test
secrets.bind
secrets.read_metadata
admin.health
admin.queue
admin.policy
admin.storage
```

Action capabilities should be namespaced by resource:

```text
network.http
network.webhook_reply
file.read
file.write
process.exec
shell.run
ai.call
github.issue.write
slack.message.send
```

Rules:

- Capability names must match `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$`.
- Capability checks are exact unless runtime policy defines explicit inheritance.
- Wildcards are allowed only in operator policy, never in workflow YAML.
- A workflow cannot grant itself capabilities.

## Policy Evaluation Points

The runtime must evaluate policy at these points:

| Point | Required decision |
|---|---|
| Workflow validation | Are requested actions, secrets, triggers, and features allowed? |
| Workflow registration | Can this principal register this workflow and bind these secrets? |
| Run admission | Can this trigger or caller start this workflow? |
| Input mapping | Are requested runtime source roots allowed for this trigger? |
| Secret resolution | Can this workflow/action access this secret alias? |
| Action scheduling | Is this action allowed under current workflow policy? |
| Process launch | Is process or shell execution allowed for this workflow? |
| File access | Is the path inside allowed read/write roots? |
| Network call | Is destination allowed by egress policy? |
| Ask prompt creation | Is the approver target valid and authorized? |
| Ask answer | Can this principal answer this prompt? |
| Run inspection | Can this principal read this run, log, trace, or bundle? |
| Replay/cancel | Can this principal mutate run lifecycle? |

Policy failures must use stable machine codes and must not leak secrets.

## Runtime Policy Snapshot

Each accepted run binds to:

- Workflow snapshot digest.
- Action registry snapshot digest.
- Runtime policy snapshot digest.
- Secret binding snapshot metadata.

This prevents a run from resuming under a different policy after restart or workflow edits.

The runtime may deny future progress if an operator revokes a critical policy, but the denial must be recorded as an authorization event, not silently applied.

## Workflow Registration

Registering a workflow requires `workflows.register` or `workflows.write`.

Registration must validate:

- Workflow YAML schema.
- Trigger policy.
- Action existence.
- Action capabilities.
- Secret declarations.
- Capability requirements.
- Shell/process restrictions.
- Network/file restrictions where statically knowable.

Registration result should include a policy summary:

```json
{
  "workflow": "issue_triage",
  "digest": "sha256:...",
  "allowed": true,
  "capabilities": ["network.http", "github.issue.write"],
  "secrets": ["github_token"],
  "unsafe_actions": [],
  "warnings": []
}
```

## Trigger Security

### Manual

Manual runs require `runs.start` for the workflow.

The runtime should record the user as the invoker.

### Schedule

Schedule runs use a runtime `trigger` principal.

The workflow must have been registered by a principal allowed to create schedule triggers.

### Event

Event submission requires `events.submit` unless a runtime policy allows anonymous events for a specific event name.

Event name allowlists are recommended.

### Webhook

Webhook runs may be anonymous only when the workflow declares or runtime binds an explicit verification policy.

Supported verification policy examples:

```yaml
when:
  webhook:
    method: POST
    path: /github
    unique: request.header.X-GitHub-Delivery
    verify:
      hmac_sha256:
        header: X-Hub-Signature-256
        secret: github_webhook_secret
```

Rules:

- Webhook `verify.secret` must reference a declared secret alias.
- Verification happens before input mapping.
- Failed verification returns `401` or `403` with redacted error details.
- Duplicate webhook uniqueness checks happen after verification.
- Anonymous unverified webhook triggers must be disabled by default.

## Secret Model

Workflow YAML declares secret aliases, not values.

```yaml
secrets:
  github_token:
    required: true
```

Secret binding is runtime configuration:

```json
{
  "workflow": "issue_triage",
  "alias": "github_token",
  "provider": "env",
  "key": "GITHUB_TOKEN"
}
```

Rules:

- Secret values must never be stored in workflow YAML.
- Secret aliases must be validated at registration.
- Required secrets must exist before a step can observe them.
- Secret values are materialized only for actions that need them.
- Secret values are redacted before persistence into user-visible records.
- Secret-tainted values are blocked from final `result` by default.
- Secrets cannot be mapped from runtime `inputs.from`.
- Secrets cannot be interpolated into `ask.prompt` by default.

## Secret Taint

A value is secret-tainted if it is:

- A direct `$secrets.name` reference.
- A string/object/list containing a secret-tainted value.
- An action output marked secret by action schema.
- A derived value from a secret-tainted input unless a trusted declassifier clears it.

Taint metadata must be stored separately from redacted display values.

Default forbidden sinks:

- `result`
- logs
- traces shown to normal inspectors
- validation errors
- runtime errors
- ask prompts
- webhook replies unless explicitly allowed
- debug bundles unless encrypted and privileged

Allowed sinks:

- Action input fields whose schema declares `secret: true` or `accepts_secret: true`.
- Runtime verification logic, such as webhook HMAC checks.
- Encrypted internal secret storage, if implemented.

## Action Capability Contract

Each action manifest declares required capabilities.

```yaml
capabilities:
  - network.http
  - github.issue.write
```

The validator checks that workflow policy allows those capabilities.

The runtime checks again before scheduling each action.

If an action computes a dynamic destination, the runtime must enforce policy dynamically.

Example for HTTP egress:

```yaml
egress:
  allow:
    - https://api.github.com/*
    - https://hooks.slack.com/services/*
```

If the final resolved URL is not allowed, the step fails with `CAPABILITY_DENIED`.

## Shell And Process Policy

Shell is not a normal workflow authoring surface. It is an unsafe action family.

Recommended action split:

| Action | Meaning | Default server policy |
|---|---|---|
| `process.exec` | Executes argv without shell expansion. | Disabled unless allowed. |
| `shell.run` | Executes through a shell. | Disabled. |

Rules:

- Shell/process actions require explicit runtime capability.
- Shell/process actions must be hidden under Advanced/Unsafe in UI.
- Shell/process actions must declare timeout, output limit, working directory, environment allowlist, and secret handling.
- Shell/process actions must not inherit the full host environment by default.
- Shell/process actions must not receive secrets unless specific fields declare secret acceptance.
- Shell/process outputs are untrusted and size-limited.
- Shell/process stderr/stdout are redacted before persistence.

Preferred replacements:

- `http.get`, `http.post`
- `json.pick`, `json.map`, `json.merge`
- `file.read`, `file.write`
- provider-specific actions such as `github.issue.comment`
- action manifests that compile to structured HTTP calls

## Filesystem Policy

File actions require scoped roots.

Policy example:

```json
{
  "file": {
    "read_roots": ["/srv/twerk/workflows", "/srv/twerk/data/read"],
    "write_roots": ["/srv/twerk/data/write"],
    "deny_patterns": ["**/.ssh/**", "**/.env", "**/secrets/**"]
  }
}
```

Rules:

- Paths must be normalized before policy checks.
- Symlink traversal must not escape allowed roots.
- Relative paths are resolved against a runtime working directory, not process CWD.
- File action outputs should return metadata and artifact handles, not large inline blobs.
- Binary file content is not a normal workflow value.

## Network Policy

Network actions require explicit egress policy.

Policy should support:

- Scheme allowlist.
- Host allowlist.
- Port allowlist.
- Optional path prefix allowlist.
- DNS rebinding protection.
- Private address blocking unless explicitly allowed.
- Request body and response size limits.
- Timeout limits.

The runtime must check the resolved destination, not only the template string.

## Ask Approval Security

`ask` creates a durable authorization point.

Rules:

- `ask.to` must resolve to an allowed user, group, role, or queue.
- The runtime must authenticate the responder.
- The response must be schema-validated as untrusted data.
- Self-approval must be denied unless policy allows it.
- The audit log must record prompt, allowed choices/schema, responder, timestamp, and source.
- Prompt text must not include secrets or secret-tainted data.
- Delegation requires explicit policy.
- Timeout behavior must be explicit.

Ask answer capability:

```text
asks.answer
```

The runtime may further restrict answers by workflow, prompt, group, or role.

## API Authorization

The server API declares capability requirements in `docs/server-api.md`.

Rules:

- API tokens should be scoped to capabilities.
- Read endpoints must apply run/workflow-level access checks.
- Log, trace, and bundle endpoints must redact by default.
- `--no-redact` or equivalent API access requires a privileged capability.
- Cancel/replay operations must record actor and reason.
- Admin endpoints must not be exposed anonymously.

## CLI Security

Local CLI authority is runtime policy.

Recommended modes:

| Mode | Behavior |
|---|---|
| `local_dev` | Current OS user may validate, dry-run, and run local workflows with limited actions. |
| `server_client` | CLI authenticates to server API and uses server-side capabilities. |
| `ci` | Non-interactive, explicit token, strict redaction, no prompts. |

CLI commands must never print secrets by default.

## Audit Events

Security-sensitive operations must emit audit events.

Required audit event kinds:

```text
WorkflowRegistered
WorkflowRejected
PolicySnapshotBound
RunStartedByPrincipal
WebhookVerified
WebhookRejected
SecretResolved
SecretDenied
CapabilityGranted
CapabilityDenied
ActionScheduled
UnsafeActionScheduled
AskCreated
AskAnswered
AskDenied
RunCancelled
RunReplayed
BundleCreated
NoRedactAccessed
```

Audit event fields:

```json
{
  "event": "CapabilityDenied",
  "time": "2026-04-28T12:00:00Z",
  "run_id": "run_01",
  "workflow": "issue_triage",
  "step": "comment",
  "principal": "user_123",
  "capability": "github.issue.write",
  "reason": "not_allowed_by_policy"
}
```

Audit events must be durable and queryable by CLI/API.

## Error Codes

Recommended security error codes:

| Code | Meaning |
|---|---|
| `AUTH_REQUIRED` | Authentication required. |
| `AUTH_INVALID` | Authentication failed. |
| `CAPABILITY_DENIED` | Principal or workflow lacks required capability. |
| `ACTION_DENIED` | Action not allowed by policy. |
| `SECRET_NOT_DECLARED` | Workflow references undeclared secret. |
| `SECRET_UNAVAILABLE` | Required secret cannot be resolved. |
| `SECRET_ACCESS_DENIED` | Secret exists but policy denies access. |
| `SECRET_LEAK_BLOCKED` | Secret-tainted value attempted to reach forbidden sink. |
| `WEBHOOK_VERIFICATION_FAILED` | Webhook signature or verification failed. |
| `EGRESS_DENIED` | Network destination denied. |
| `FILE_ACCESS_DENIED` | File path denied. |
| `PROCESS_DENIED` | Process execution denied. |
| `SHELL_DENIED` | Shell execution denied. |
| `ASK_ANSWER_DENIED` | Principal cannot answer prompt. |
| `NO_REDACT_DENIED` | Principal cannot view unredacted data. |

## Minimal Policy File Shape

Runtime policy may be represented in any implementation format, but the logical shape should cover:

```yaml
version: twerk/policy/v1

defaults:
  redact: true
  anonymous_webhooks: false
  shell: deny
  process: deny

capabilities:
  workflows:
    issue_triage:
      allow:
        - network.http
        - github.issue.write
      deny:
        - shell.run

egress:
  allow:
    - https://api.github.com/*

files:
  read_roots: []
  write_roots: []

asks:
  self_approval: deny

inspection:
  no_redact_requires: admin.policy
```

Policy is intentionally separate from workflow YAML so normal authors do not see runtime knobs.

## Validator Integration

The validator must reject workflows when:

- An action requires a capability not allowed by policy.
- A workflow references an undeclared secret.
- A secret is declared but not bindable by the deployment principal.
- A webhook declares invalid verification or uniqueness sources.
- A shell/process action is used without explicit allow policy.
- Static URL/path destinations violate policy.
- `result` references secret-tainted data.
- `ask.prompt` references secret-tainted data.

The validator should warn when:

- A workflow uses broad network destinations.
- A workflow uses unsafe actions.
- A workflow declares unused secrets.
- A workflow uses dynamic URLs or paths that require runtime checks.
- A workflow has no webhook verification policy.

## Open Questions

1. Should policy be stored as YAML alongside workflows, or only as runtime configuration?
2. Should built-in provider actions imply fine-grained capabilities, or should each provider action declare them explicitly?
3. Should local development mode allow process execution by default, or require explicit project opt-in?
4. Should unredacted debug bundles exist at all, or should debug bundles always be redacted with encrypted internal raw payloads unavailable to users?
5. Should secret declassification be supported in v1, or forbidden until a later security model is implemented?
