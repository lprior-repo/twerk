# Twerk Quickstart

Status: draft

Audience: workflow authors, operators, UI builders, and AI agents.

## What Twerk Is

Twerk is a strict YAML workflow runtime.

The authoring model is:

```text
When this happens, take this input, run these steps, return this result.
```

The runtime model is:

```text
validate -> compile -> accept durably -> run -> inspect everything
```

This quickstart describes the intended v1 user experience and command contract.

## Create A Workflow

Create `hello.yaml`:

```yaml
version: twerk/v1
name: hello_world

when:
  manual: {}

inputs:
  name: text

steps:
  - id: greeting
    set:
      message: "Hello $input.name"

result:
  message: $greeting.message

examples:
  - name: ada
    input:
      name: Ada
    expect:
      result:
        message: Hello Ada
```

Validate it:

```bash
twerk validate hello.yaml
```

Machine-readable validation:

```bash
twerk validate hello.yaml --json
```

Expected success shape:

```json
{
  "ok": true,
  "workflow": "hello_world",
  "diagnostics": [],
  "steps": 1
}
```

## Explain The Workflow

Explain turns YAML into a plain-English and machine-readable execution plan.

```bash
twerk explain hello.yaml
```

Expected human output:

```text
Workflow: hello_world
Trigger: manual
Inputs: name:text
Steps:
  greeting: set message from input name
Result:
  message <- greeting.message
```

Machine output:

```bash
twerk explain hello.yaml --json
```

## Dry Run With Input

Dry-run validates the workflow, maps input, resolves expressions, and shows the steps that would run without side effects.

```bash
twerk dry-run hello.yaml --input '{"name":"Ada"}'
```

Expected output:

```text
DRY RUN hello_world
input.name = Ada
step greeting -> {"message":"Hello Ada"}
result -> {"message":"Hello Ada"}
```

Dry-run an example:

```bash
twerk dry-run hello.yaml --example ada
```

## Test Examples

Examples are executable fixtures.

```bash
twerk test hello.yaml
```

Expected output:

```text
PASS hello_world example=ada
```

## Run Locally

Run a manual workflow:

```bash
twerk run hello.yaml --input '{"name":"Ada"}'
```

Machine-readable event stream:

```bash
twerk run hello.yaml --input '{"name":"Ada"}' --jsonl
```

Example JSONL:

```jsonl
{"event":"RunAccepted","run_id":"run_01","workflow":"hello_world"}
{"event":"StepStarted","run_id":"run_01","step":"greeting"}
{"event":"StepSucceeded","run_id":"run_01","step":"greeting","output":{"message":"Hello Ada"}}
{"event":"RunSucceeded","run_id":"run_01","result":{"message":"Hello Ada"}}
```

## Inspect A Run

Every accepted run is inspectable.

```bash
twerk inspect run_01
```

Useful views:

```bash
twerk events run_01 --jsonl
twerk logs run_01 --jsonl
twerk trace run_01 --json
twerk graph hello.yaml --json
```

## Add An Action

Actions are registered capabilities, not arbitrary scripts.

```yaml
version: twerk/v1
name: issue_comment

when:
  manual: {}

inputs:
  repo: text
  issue: number
  body: text

secrets:
  github_token:
    required: true

steps:
  - id: comment
    do: github.issue.comment
    with:
      token: $secrets.github_token
      repo: $input.repo
      issue: $input.issue
      body: $input.body

result:
  url: $comment.url
```

List available actions:

```bash
twerk actions list
```

Inspect one action contract:

```bash
twerk actions show github.issue.comment --json
```

## Run A Webhook Workflow

Create `github_triage.yaml`:

```yaml
version: twerk/v1
name: github_triage

when:
  webhook:
    method: POST
    path: /hooks/github
    unique: request.header.X-GitHub-Delivery

inputs:
  delivery_id:
    from: request.header.X-GitHub-Delivery
    is: text
  title:
    from: request.body.issue.title
    is: text
  body:
    from: request.body.issue.body
    is: text
    default: ""

steps:
  - id: classify
    do: ai.classify
    with:
      text: "$input.title\n\n$input.body"

  - id: route
    choose:
      - if: $classify.label == "bug"
        steps:
          - id: bug_result
            set:
              kind: bug
              priority: high
        result:
          kind: bug
          priority: $bug_result.priority

      - otherwise: true
        steps:
          - id: normal_result
            set:
              kind: normal
              priority: normal
        result:
          kind: normal
          priority: $normal_result.priority

result:
  delivery_id: $input.delivery_id
  label: $classify.label
  priority: $route.priority
```

Start server mode:

```bash
twerk serve --workflow github_triage.yaml
```

Submit a webhook:

```bash
curl -X POST http://localhost:8080/hooks/github \
  -H 'Content-Type: application/json' \
  -H 'X-GitHub-Delivery: delivery-123' \
  -d '{"issue":{"title":"Bug: login fails","body":"Users cannot log in"}}'
```

If accepted, the response means the run is already durable.

## Use The UI

The UI is a structured builder over the same YAML.

Recommended flow:

1. Open workflow YAML.
2. See cards for `when`, `inputs`, `steps`, and `result`.
3. Drag an action from the registry into the step list.
4. Configure `with` fields using the data picker.
5. Validate from the side panel.
6. Run an example or dry-run.
7. Inspect runtime events on the same graph.

No UI-only workflow semantics are allowed. Layout hints live in a sidecar only.

## Debug With A Bundle

When an AI agent or human needs full context:

```bash
twerk bundle run_01 --output run_01.twerk-bundle.zip
```

The bundle should contain redacted:

- Workflow snapshot.
- Compiled graph.
- Input mapping.
- Events.
- Logs.
- Traces.
- Step inputs and outputs.
- Errors.
- Runtime version and policy summary.

## Shell Is Not The Default

Prefer typed actions:

```yaml
do: http.post
```

Instead of:

```yaml
do: shell.run
```

If shell is enabled, it is an advanced unsafe action controlled by runtime policy.

## Next Reading

- `docs/language-spec.md` for language semantics.
- `docs/action-registry.md` for action contracts.
- `docs/cli-observability.md` for CLI and JSON output.
- `docs/ui-layer.md` for the visual builder.
- `docs/runtime-architecture.md` for runtime behavior.
- `docs/storage-model.md` for Fjall persistence.
- `docs/security-capabilities.md` for policy and capabilities.
- `docs/validator-contract.md` for validation rules.
