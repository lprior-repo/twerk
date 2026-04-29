# Twerk UI Layer

Status: draft

Audience: frontend builders, product designers, workflow authors, runtime implementers, and AI agents.

## Purpose

The Twerk UI is a Step Functions-style drag-and-drop workflow builder and run inspector for the Twerk YAML language.

The UI must make workflows feel simple:

```text
When this happens, take this input, run these steps, return this result.
```

The UI must not create a second hidden workflow language. It edits the same strict YAML model that the CLI validates and the runtime executes.

## Design Goals

- Non-technical users can build workflows by dragging cards.
- Technical users can edit YAML directly.
- AI agents can generate YAML and see the same graph as humans.
- The UI prevents invalid topology where possible.
- Every visual card maps to one language primitive.
- Action forms are generated from action registry contracts.
- Run inspection uses the same graph as workflow editing.
- Validation errors point to both YAML locations and visual cards.

## Non-Goals

- No freeform spaghetti canvas in v1.
- No UI-only execution semantics.
- No hidden node types that do not exist in YAML.
- No arbitrary cycles or backward edge creation.
- No visual behavior that cannot round-trip to YAML.
- No mandatory use of the UI for debugging; the CLI remains complete.

## Source Of Truth

The source of truth is always the workflow YAML.

```text
YAML -> parser -> workflow model -> validator -> compiled graph -> visual canvas
```

Edits flow back the same way:

```text
Canvas edit -> workflow model patch -> validation -> YAML update
```

The UI may store layout preferences in a sidecar file, but layout must never change workflow behavior.

```text
flow.yaml
flow.layout.json
```

The sidecar can store card positions, collapsed groups, zoom level, and viewport state. It must not store control flow, retries, error behavior, inputs, secrets, or action parameters.

## Visual Model

Use a structured Step Functions-style graph, not a freeform node canvas.

Top-level layout:

```text
[When this happens]
        |
[Inputs]
        |
[Steps]
        |
[Result / Finish]
```

Example:

```text
[Webhook: /github]
        |
[Map Inputs]
        |
[Get issue title]
        |
[Classify issue]
        |
[Choose route]
   | urgent
   v
[Page team] ---> [Finish]
   | otherwise
   v
[Create ticket] -> [Finish]
```

The canvas should auto-layout from the YAML structure. Manual positioning is optional and stored only in the layout sidecar.

## Card Types

Every language primitive maps to one visual card type.

| YAML | UI label | Card type |
|---|---|---|
| `when` | When this happens | Trigger card |
| `inputs` | Take this input | Input mapping card |
| `do` | Do something | Action card |
| `set` | Save a value | Data card |
| `choose` | Choose a path | Decision card |
| `for_each` | For each item | Loop container |
| `together` | Run together | Parallel container |
| `collect` | Collect pages | Pagination container |
| `reduce` | Add things up | Reducer container |
| `repeat` | Repeat until | Polling container |
| `wait` | Wait | Wait card |
| `ask` | Ask someone | Approval card |
| `finish` | Finish | Terminal card |
| `try_again` | Try again | Retry badge |
| `on_error` | If this fails | Error path |
| `if` | Only run if | Condition badge |

The UI copy should use friendly labels. The YAML view shows exact field names.

## Canvas Rules

Drag and drop should prevent invalid workflows.

| Drop target | Behavior |
|---|---|
| Between two steps | Insert a new YAML step. |
| Onto a step error handle | Create or edit `on_error`. |
| Onto a `choose` card | Add a branch. |
| Into a `for_each` card | Add loop body step. |
| Into a `together` card | Add branch step. |
| Into a `collect` card | Configure page action or item extraction. |
| Into a `reduce` card | Configure accumulator update. |
| Into a `repeat` card | Configure repeated action or check. |
| After a reachable `finish` card | Block as unreachable. |
| Backward edge | Block in v1. |
| Into another scope from outside | Block unless the model supports that scope transition. |

The UI should not let users draw graph edges that YAML validation will reject.

## Workflow Builder Layout

Recommended screen layout:

```text
Left: action palette
Center: graph canvas
Right: inspector panel
Bottom: validation and run output
Top: workflow name, save, validate, test, run, YAML toggle
```

The YAML editor should be available side-by-side or as a toggle. YAML edits update the graph after validation.

## Trigger Panel

The trigger panel edits `when`.

Friendly choices:

```text
Manual
Webhook
Schedule
Event
```

Webhook fields:

```text
Path
Method
Unique request key
```

Schedule fields:

```text
Cron
Timezone if supported by runtime policy
```

Event fields:

```text
Event name
Optional unique event key
```

The UI should avoid exposing queue depth, durability, and internal runtime knobs in normal mode.

## Input Mapper

The input mapper edits `inputs`.

It should show runtime source fields for the selected trigger.

Example webhook sources:

```text
request.body
request.header.NAME
request.query.NAME
request.path.NAME
request.method
request.path
```

Input editor fields:

```text
Name
Source
Kind
Required
Default
Deep shape
Allow extra fields
Description
```

The deep shape editor should support:

```text
object fields
list element type
optional fields
default values
extra: allow/reject
```

## Action Palette

The action palette is generated from the action registry.

Suggested categories:

```text
Start
Data
AI
HTTP
Files
GitHub
Slack
Email
People
Timing
Advanced
```

Shell must be in Advanced or Unsafe, not in the default recommended set.

Action metadata:

```yaml
name: github.issue.comment
title: Add GitHub comment
description: Adds a comment to an issue or PR
inputs_schema: {}
outputs_schema: {}
secrets:
  - github_token
retry_safe: requires_idempotency_key
side_effect: true
ui:
  icon: github
  category: GitHub
  color: gray
```

The UI should use action schemas to generate forms, validate fields, show output previews, and suggest next steps.

## Inspector Panel

Clicking a card opens the inspector.

Common fields for every step:

```text
Step name
Step ID
Only run if
What happens next
Try again
If this fails
Notes or description
```

For `do` cards:

```text
Action
Inputs
Secrets used
Output preview
Retry safety
Side effects
Timeout if exposed by action policy
```

For `set` cards:

```text
Output fields
Value expressions
Type preview
```

For `choose` cards:

```text
Branch list
Condition per branch
Otherwise branch
Branch result shape
Selected branch metadata
```

For `for_each` cards:

```text
List to repeat over
Item name
Max at once
Max starts per second
Loop body
Output shape
Failure behavior
```

For `together` cards:

```text
Branches
Branch names
Failure mode: fast, after all, collect
Branch outputs
Partial failure behavior
```

For `collect` cards:

```text
Starting cursor
Page action
Next cursor expression
Items expression
Stop condition
Page limit
Item limit
Time limit
Wait between pages
Partial output behavior
```

For `reduce` cards:

```text
Input list
Item name
Starting accumulator
Accumulator update fields
Final output preview
```

For `repeat` cards:

```text
Repeated action
Until condition
Attempt limit
Time limit
Wait between attempts
Final output preview
```

For `wait` cards:

```text
Duration
Until timestamp
Event name
Event condition
Timeout
```

For `ask` cards:

```text
Question
Choices
Who can answer
Timeout
Default or timeout behavior
Audit policy hint
```

For `finish` cards:

```text
Status
Error code if failure
Message
Result preview
```

## Data Picker

Every form field that accepts references should have a data picker.

Sections:

```text
Input
Variables
Secrets
Previous step results
Current item
Current error
Current attempt
```

The picker should insert valid references:

```text
$input.email
$vars.region
$secrets.github_token
$classify.label
$customer.email
$error.code
$attempt.body.status
```

Users should not need to memorize reference syntax. Advanced users can type references directly.

The picker must be scope-aware. It should not show `$customer.email` outside the relevant `for_each` body.

## Expression Builder

The expression builder is used for `if`, `choose`, `collect.stop`, and `repeat.until`.

It should expose a small safe vocabulary:

```text
equals
not equals
greater than
less than
contains
starts with
ends with
has key
exists
length
is empty
and
or
not
```

It compiles to the language expression syntax:

```yaml
if: contains($input.labels, "urgent")
```

The UI must not offer arbitrary JavaScript, Python, jq, or regex by default.

## Branch Builder

`choose` should render as a decision card with vertical or horizontal paths.

Example visual:

```text
[Choose route]
   | urgent
   v
[Page team]

   | otherwise
   v
[Create ticket]
```

The branch builder should require:

```text
Condition or otherwise
Branch body
Branch result
```

Downstream steps should reference the `choose` card output, not internal branch-only steps.

Good:

```text
$route.kind
```

Avoid forcing users to write fragile references like:

```text
$alert.id or $ticket.id
```

## Loop Builder

`for_each` should render as a container card.

Visual:

```text
[For each customer in input.customers]
   [Send email]
   [Record result]
```

The UI should show:

```text
Input list
Item name
Concurrency throttle
Rate throttle
Loop body
Output list preview
```

The UI should explain that output order follows input order even if execution is concurrent.

## Pagination Builder

`collect` should be a first-class UI pattern, not a hidden loop.

Visual:

```text
[Collect customers]
   Start cursor: null
   Page action: HTTP GET /customers
   Items: page.body.customers
   Next cursor: page.body.next_cursor
   Stop when: next cursor is empty
   Limits: 500 pages, 50000 items, 5m
```

The UI should make bounded pagination unavoidable:

```text
Page limit is required
Item limit is required
Time limit is required
```

This answers the product requirement for API pagination without allowing arbitrary graph cycles.

## Reduce Builder

`reduce` should be presented as "Add things up" or "Build a summary" for non-technical users.

Visual:

```text
[Build totals from customers]
   Start:
     count = 0
     revenue = 0

   For each customer:
     count = count + 1
     revenue = revenue + customer.spend
```

The builder should show accumulator fields and type-check each update.

It should make clear that reducer state is scoped to the reducer and does not mutate workflow globals.

## Repeat Builder

`repeat` should be presented as "Check until" or "Poll until".

Visual:

```text
[Check job until done]
   Action: HTTP GET /jobs/$job.id
   Stop when: attempt.body.status == done
   Try up to: 60 times
   Wait between: 5s
   Max time: 10m
```

The UI should require limits.

It should distinguish `repeat` from `try_again`:

| Feature | Meaning |
|---|---|
| `try_again` | Retry a failed step. |
| `repeat` | Run a successful check repeatedly until a condition is true. |

## Parallel Builder

`together` should render as named parallel lanes.

Visual:

```text
[Run together: enrich]
   Lane: profile -> [Lookup profile]
   Lane: orders  -> [List orders]
   Lane: risk    -> [Score risk]
```

The inspector should expose failure behavior:

```text
Stop fast
Wait for all then fail
Collect success and errors
```

Successful branch state should remain visible even if the parent fails.

## Error Path UI

Every step card should have a red error handle.

Dragging from that handle creates `on_error`.

Visual:

```text
[Call API]
   success -> [Continue]
   error   -> [Fallback]
```

The error path inspector should show:

```text
What errors are caught
Fallback output if any
Next step after recovery
Finish as failure if unrecovered
```

Inside an error handler, the data picker should expose `$error`.

For `together`, error UI should expose `$error.partial` so users can see successful and failed branches.

## Retry UI

`try_again` should appear as a badge on a card.

Badge examples:

```text
Try again x3
Retry timeouts only
Backoff up to 5s
```

The retry inspector should show:

```text
Total attempts
Retryable errors
Wait strategy
Idempotency warning
```

If an action is not retry-safe, the UI should warn or block retry configuration.

## Run Inspector

After a run starts, the builder graph becomes an execution inspector.

Use one graph for build and runtime inspection.

State colors:

| State | Visual treatment |
|---|---|
| pending | Grey |
| running | Blue pulse |
| succeeded | Green |
| failed | Red |
| skipped | Muted grey |
| waiting | Clock icon |
| asking | Person icon |
| retrying | Circular arrow |
| cancelled | Dark grey |

Clicking a completed step should show:

```text
Resolved input
Redacted secrets
Output
Attempts
Logs
Error object
Duration
Event history
```

For `collect`, show:

```text
Pages fetched
Items collected
Current cursor
Limits
Partial output on failure
```

For `repeat`, show:

```text
Attempt count
Last attempt output
Until condition status
Next wait
Limit remaining
```

For `together`, show:

```text
Branch status
Branch output
Branch error
Aggregate failure mode
```

## Timeline View

The timeline view uses the same data as `twerk trace`.

It should show:

```text
Queue wait
Input mapping
Step execution
Action latency
Retries
Waits
Asks
Fjall commits
Result evaluation
```

The timeline should help answer:

```text
Where did time go?
Which step retried?
Was this blocked on queue, action, wait, ask, or storage?
```

## Logs View

The logs view uses the same records as `twerk logs`.

Filters:

```text
Run
Step
Attempt
Action
Level
Time range
Error code
```

Secrets must remain redacted.

## Debug Bundle View

The UI should be able to open and export the same debug bundle as `twerk bundle`.

Bundle contents:

```text
Workflow YAML
Workflow digest
Validation diagnostics
Compiled graph
Action contracts
Input
Events
Step state
Logs
Trace
Errors
Suggested fixes
```

This gives human support, AI repair, and bug reports a single artifact.

## Validation UX

Validation should run continuously after edits.

Validation errors must link both ways:

```text
Canvas card -> YAML location
YAML location -> Canvas card
```

Error display:

```text
Code
Message
Card
YAML path
Line and column
Suggestion
Documentation link
```

Example:

```text
UNKNOWN_REFERENCE in Classify issue
steps[2].with.text, line 31
Unknown reference '$titel.text'. Did you mean '$title.text'?
```

Warnings should not block save unless runtime policy says so.

Errors should block deployment and execution.

## YAML Round Trip

The UI must support lossless semantic round-trip.

Required behavior:

- Opening YAML renders the same workflow graph.
- Editing the graph updates YAML.
- Editing YAML updates the graph.
- Comments may be preserved where practical but cannot carry semantics.
- Unknown fields are rejected, not hidden.
- Formatting may change, behavior must not.

The UI should expose a diff before applying large AI-generated changes.

## AI-Assisted Builder

AI features should operate on the same CLI and validation contracts.

Useful AI actions:

```text
Generate workflow from prompt
Explain this workflow
Fix validation errors
Suggest missing input mappings
Suggest retry policies
Suggest action replacement for shell.run
Generate examples
Summarize failed run
Create debug bundle
```

AI flow:

```text
Prompt -> YAML proposal -> validate -> graph preview -> user approval -> save
```

The AI should not directly mutate a deployed workflow without showing diff and validation results.

## Shell Minimization UI

Shell actions should be visually discouraged.

Rules:

- Hide shell from default palette.
- Place shell under Advanced or Unsafe.
- Prefer typed native actions.
- Suggest `http.get`, `file.write`, `json.pick`, or other native actions when users ask for shell.
- Show warning when shell uses secrets.
- Require runtime capability approval for shell in server mode.

Shell card warning:

```text
Shell can execute arbitrary host commands. Prefer typed actions when possible.
```

## Layout Sidecar

Optional sidecar shape:

```json
{
  "schema_version": "twerk.layout/v1",
  "workflow_digest": "sha256:...",
  "nodes": {
    "classify": {
      "x": 420,
      "y": 240,
      "collapsed": false
    }
  },
  "viewport": {
    "x": 0,
    "y": 0,
    "zoom": 1.0
  }
}
```

Rules:

- Sidecar is optional.
- Sidecar must not affect execution.
- If digest mismatches, UI may ignore or partially apply layout.
- Workflow semantics are derived only from YAML.

## Accessibility

The builder must be usable without drag and drop.

Requirements:

- Keyboard navigation between cards.
- Add step before/after using buttons.
- Move step up/down where legal.
- Screen-reader labels for card type, status, and validation errors.
- Color must not be the only state indicator.
- Run status must include text labels.

## MVP Sequence

Recommended implementation order:

1. Read-only YAML visualizer.
2. Validation panel with clickable errors.
3. Side-by-side YAML editor.
4. Action registry-driven forms.
5. Sequential drag/drop step creation.
6. `choose` branch builder.
7. `for_each`, `together`, `collect`, `reduce`, and `repeat` containers.
8. Run inspector with live status.
9. Logs, events, and trace views.
10. Debug bundle import/export.
11. AI workflow generator and fixer.

## Example YAML To UI Mapping

YAML:

```yaml
version: twerk/v1
name: issue_triage

when:
  webhook:
    path: /github
    method: POST

inputs:
  body:
    from: request.body
    is: object

steps:
  - id: title
    set:
      text: $input.body.issue.title

  - id: classify
    do: ai.classify
    with:
      text: $title.text

  - id: route
    choose:
      - if: $classify.label == "urgent"
        steps:
          - id: alert
            do: pager.alert
            with:
              message: $title.text
        result:
          kind: urgent
          id: $alert.id

      - otherwise: true
        steps:
          - id: ticket
            do: ticket.create
            with:
              title: $title.text
        result:
          kind: ticket
          id: $ticket.id

result:
  label: $classify.label
  route: $route.kind
```

Visual:

```text
[When webhook POST /github]
        |
[Input: body from request.body]
        |
[Save value: title]
        |
[Do: Classify text]
        |
[Choose route]
   | urgent          | otherwise
   v                 v
[Do: Page team]   [Do: Create ticket]
   |                 |
   +------ merge ----+
          |
       [Result]
```

## Product Position

The UI should feel like Step Functions for clarity, GitHub Actions for authoring, Argo for structured fanout, and n8n for approachability. It should not inherit the downsides of any of them.

Final decision:

```text
Structured graph builder.
YAML source of truth.
One card per primitive.
Registry-generated forms.
Scope-aware data picker.
Runtime graph doubles as run inspector.
No hidden workflow semantics.
```

## Open Questions

- Should the first UI support manual card positioning, or rely entirely on auto-layout?
- Should branch result shape be required in the UI for every `choose` branch?
- Should `collect`, `reduce`, and `repeat` be in the default palette or advanced palette?
- Should AI edits be applied directly to YAML or through workflow model patches?
- Should the UI support collaboration in v1, or leave concurrent editing out of scope?
