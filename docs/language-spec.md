# Twerk Workflow Language v1

Status: draft

Audience: workflow authors, UI builders, runtime implementers, validators, and AI agents.

## Purpose

Twerk Workflow Language v1 is a strict YAML language for durable AI-native workflows. It is designed around one sentence:

```text
When this happens, take this input, run these steps, return this result.
```

The language must be readable by non-technical people, generated reliably by AI, validated before execution, visualized as a graph, and executed by a high-throughput single-binary runtime.

## Design Goals

- YAML is the source of truth.
- The language is small, closed, and strict.
- Workflow data is visible, inspectable, and typed.
- Runtime state is durable and replay-safe.
- External side effects are explicit and at-least-once.
- Control flow is graph-like for humans and structured for the runtime.
- AI agents can validate, explain, dry-run, repair, and inspect workflows without guessing.
- Shell execution is an escape hatch, not the center of the platform.

## Non-Goals

- No arbitrary scripting language in workflow YAML.
- No arbitrary backward graph cycles in v1.
- No global mutable variables.
- No unbounded loops, unbounded pagination, or unbounded retries.
- No hidden UI-only workflow semantics.
- No exactly-once guarantee for external side effects.
- No Docker, Kubernetes, RabbitMQ, or Postgres requirement in the language.

## Core Mental Model

Twerk workflows use these concepts:

| Concept | Meaning |
|---|---|
| `when` | What starts the workflow. |
| `inputs` | Runtime data mapped into stable names. |
| `vars` | Static non-secret constants. |
| `secrets` | Named secret requirements, never literal secret values. |
| `steps` | Ordered workflow actions and control primitives. |
| `result` | Final workflow output mapping. |

Each step has an `id`. After a step succeeds, its output is available through `$step_id.field`. Step outputs are immutable.

## YAML Profile

Twerk uses a restricted YAML profile.

Allowed:

- Strings
- Numbers
- Booleans
- Null
- Lists
- Objects
- Comments

Rejected:

- Duplicate keys
- Anchors
- Aliases
- Merge keys
- Custom tags
- Binary scalars
- Parser-specific YAML 1.1 booleans such as `yes`, `no`, `on`, and `off`

Unknown top-level fields and unknown step fields are validation errors.

## Top-Level Fields

Required top-level fields:

```yaml
version:
name:
when:
steps:
```

Optional top-level fields:

```yaml
inputs:
vars:
secrets:
result:
examples:
```

Canonical version:

```yaml
version: twerk/v1
```

Minimal workflow:

```yaml
version: twerk/v1
name: hello_world

when:
  manual: {}

steps:
  - id: greeting
    set:
      text: Hello from Twerk

result:
  message: $greeting.text
```

## Step Fields

Allowed step fields:

```yaml
id:
name:
if:
do:
with:
set:
choose:
for_each:
together:
collect:
reduce:
repeat:
wait:
ask:
try_again:
on_error:
then:
finish:
```

Every step must have exactly one primitive:

```text
do, set, choose, for_each, together, collect, reduce, repeat, wait, ask, finish
```

Metadata and control fields are not primitives:

```text
id, name, if, with, try_again, on_error, then
```

## Names And IDs

Workflow names, step IDs, branch names, and loop variables must use this pattern:

```text
^[a-z][a-z0-9_]{0,63}$
```

Reserved names:

```text
input, inputs, vars, secrets, steps, result, when, item, error,
true, false, null,
do, set, choose, for_each, together, collect, reduce, repeat,
wait, ask, try_again, on_error, then, finish
```

In v1, all step IDs must be globally unique within a workflow, including nested `choose` branches, `for_each` bodies, `together` branches, `collect` page actions, `reduce` bodies, and `repeat` bodies.

Rationale:

- CLI inspection can address any step by one ID.
- UI graph rendering does not need scoped path disambiguation.
- AI repair can patch exact steps without resolving lexical scopes.
- Event journal records can use stable step IDs without nested aliases.

Loop variables, branch names, and local roots must not collide with any global step ID or reserved name.

## Triggers

`when` declares exactly one trigger in v1.

Supported v1 triggers:

```yaml
when:
  manual: {}
```

```yaml
when:
  webhook:
    path: /github
    method: POST
    unique: request.header.X-GitHub-Delivery
```

```yaml
when:
  schedule:
    cron: "*/5 * * * *"
```

```yaml
when:
  event:
    name: customer.created
```

Runtime-specific data is not directly visible inside steps. It must be mapped into `inputs`.

Webhook uniqueness rules:

- `webhook.unique` is optional but strongly recommended for external webhook triggers.
- If present, it is evaluated from the raw request before input mapping.
- Invalid `unique` source syntax rejects the workflow at validation time.
- Missing `unique` value at runtime rejects the request with `400 INPUT_MAPPING_FAILED`.
- A duplicate `unique` value returns the existing run ID and does not enqueue a new run.
- The default duplicate response is `200 OK` with `{ "duplicate": true, "run_id": "..." }`.
- The uniqueness retention window is runtime policy and must be visible through `twerk doctor` or server config inspection.

Schedule rules:

- Cron uses five fields in v1: minute, hour, day-of-month, month, day-of-week.
- Seconds fields are invalid in v1.
- Timezone defaults to the runtime server timezone unless configured by runtime policy.
- Missed schedules after downtime do not catch up by default.
- Overlapping scheduled runs are allowed only if runtime workflow concurrency policy permits them.
- Schedule deduplication key is `workflow_digest + scheduled_time`.

## Inputs

Inputs are the workflow front door. They map runtime data into stable names and validate shape before the first step runs.

Short form:

```yaml
inputs:
  email: text
  amount: number
```

Long form:

```yaml
inputs:
  email:
    from: request.body.email
    is: text
  amount:
    from: request.body.amount
    is: number
    default: 0
```

Allowed runtime source roots in `inputs.from`:

| Trigger | Allowed source roots |
|---|---|
| webhook | `request.body`, `request.header.NAME`, `request.query.NAME`, `request.path.NAME`, `request.method`, `request.path` |
| schedule | `schedule.time`, `schedule.cron` |
| event | `event.name`, `event.id`, `event.body` |
| manual | `manual.user`, `manual.input.NAME` |

No silent type coercion is allowed. String `"123"` does not satisfy `number`.

## Deep Type System

Simple kinds:

```text
text, number, boolean, object, list, any
```

Lists should declare element type unless intentionally opaque:

```yaml
inputs:
  customers:
    from: request.body.customers
    is: list
    of:
      is: object
      fields:
        id: text
        email: text
        spend: number
```

Objects can declare fields:

```yaml
inputs:
  customer:
    from: request.body.customer
    is: object
    fields:
      id: text
      email: text
      tags:
        is: list
        of: text
      address:
        is: object
        optional: true
        fields:
          city: text
          country: text
    extra: allow
```

Rules:

- `list` without `of` is invalid unless written as `list<any>` or `of: any`.
- `object` without `fields` is an opaque object.
- `extra: allow` permits unknown object fields.
- `extra: reject` rejects unknown object fields.
- `optional: true` allows a missing field.
- `nullable: true` allows an explicit `null` value.
- `default` must match the declared type.
- `null` is valid only where the schema allows it.

This handles real API schema drift without abandoning static validation.

Canonical normalized schema form:

```json
{
  "is": "list",
  "nullable": false,
  "optional": false,
  "of": {
    "is": "object",
    "nullable": false,
    "optional": false,
    "extra": "reject",
    "fields": {
      "id": {
        "is": "text",
        "nullable": false,
        "optional": false
      }
    }
  }
}
```

Validator and UI implementations should normalize all shorthand forms into this shape before type checking.

Nullability rules:

- Missing and `null` are different states.
- `optional: true` allows a field to be absent.
- `nullable: true` allows a field to be present with value `null`.
- A field may be both optional and nullable.
- `default` applies to missing values unless the field is present.
- A present `null` uses `default` only if runtime policy explicitly enables null-defaulting; v1 default behavior is to preserve `null` and validate it against `nullable`.

## Vars

`vars` are immutable non-secret constants.

```yaml
vars:
  model: fast
  region: us-east-1
  max_score: 100
```

Vars cannot reference runtime data, step outputs, or secrets.

## Secrets

`secrets` declare secret requirements. The workflow file never stores literal secret values.

```yaml
secrets:
  github_token: GITHUB_TOKEN
  slack_webhook: SLACK_WEBHOOK
```

Rules:

- Undeclared secret references are validation errors.
- Missing required secrets fail before observable use.
- Secrets are redacted from logs, traces, errors, examples, and result previews.
- Interpolating a secret taints the whole value.
- Secret-tainted values are blocked from `result` by default.

## References

Allowed references:

```text
$input.x
$vars.x
$secrets.x
$step_id.x
$loop_name.x
$error.x
```

Examples:

```yaml
with:
  email: $input.email
  token: $secrets.github_token
  label: $classify.label
```

Rules:

- Missing paths are errors.
- Future step references are invalid.
- Skipped step outputs are unavailable.
- Full scalar references preserve native type.
- References embedded in text become strings.
- Literal `$` is escaped as `$$`.
- References are not evaluated inside YAML keys.

## Expressions

Expressions are deterministic, bounded, side-effect-free, and statically analyzable.

Allowed operators:

```text
==, !=, >, >=, <, <=, and, or, not
```

Allowed numeric arithmetic for deterministic data shaping and reducers:

```text
+, -, *, /
```

Arithmetic rules:

- Operands must be finite numbers.
- Division by zero is a runtime error.
- Results must be finite numbers.
- Arithmetic is intended for `set` and `reduce.set`, not for arbitrary scripting.
- Implementations must reject non-finite values such as `NaN`, `Infinity`, and `-Infinity`.

Allowed predicate helpers:

```text
contains(value, needle)
starts_with(text, prefix)
ends_with(text, suffix)
has(object, key)
exists(path)
length(value)
empty(value)
```

Examples:

```yaml
if: contains($input.labels, "urgent")
if: starts_with($input.path, "/api/")
if: has($input.body, "issue")
if: exists($input.body.issue.title)
if: length($input.customers) > 0
```

Allowed reducer helpers:

```text
append(list, value)
append_if(list, value, condition)
merge(object, object)
sum(list, field)
count(list)
unique(list)
```

Formal expression grammar:

```text
expr        = or_expr
or_expr     = and_expr *( "or" and_expr )
and_expr    = not_expr *( "and" not_expr )
not_expr    = [ "not" ] compare_expr
compare_expr = add_expr [ ( "==" | "!=" | ">" | ">=" | "<" | "<=" ) add_expr ]
add_expr    = mul_expr *( ( "+" | "-" ) mul_expr )
mul_expr    = unary_expr *( ( "*" | "/" ) unary_expr )
unary_expr  = primary
primary     = reference | literal | function_call | "(" expr ")"
literal     = string | number | boolean | null
function_call = identifier "(" [ expr *( "," expr ) ] ")"
```

Operator precedence from highest to lowest:

```text
parentheses, function calls, *, /, +, -, comparisons, not, and, or
```

Literal rules:

- Strings use double quotes in expressions.
- String escapes are JSON-style: `\"`, `\\`, `\n`, `\r`, `\t`, and `\uXXXX`.
- Numbers use JSON number syntax and must be finite.
- Booleans are `true` and `false`.
- Null is `null`.
- Object and list literals are not part of v1 expressions.

Missing path rules:

- Normal reference evaluation fails when a path is missing.
- `exists(path)` is special: its argument is parsed as a path probe and must not evaluate the path before the helper runs.
- `has(object, key)` returns false if the object exists and the key is absent.
- `and` and `or` short-circuit left to right.
- Missing references in a short-circuited branch are not evaluated.

Arithmetic scope:

- Arithmetic is allowed in `set` and `reduce.set`.
- Arithmetic is allowed in conditions only when both operands are already numeric and no data-shaping result is produced.
- Arithmetic must not allocate unbounded data or invoke helper functions with side effects.

Forbidden:

- JavaScript
- Python
- jq
- Regex in v1 unless a bounded RE2-style implementation is explicitly adopted
- Network calls
- Time and random functions
- User-defined functions
- Loops inside expressions

## Control Flow

Steps run top to bottom by default.

`then` can jump only forward to an existing step in the same scope.

Rules:

- Backward jumps are forbidden in v1.
- Arbitrary graph cycles are forbidden in v1.
- Jumping into or out of nested loop, branch, parallel, or error-handler scopes is invalid.
- Unreachable steps are validation errors.
- A reachable `finish` terminates the run.

Structured repetition is provided by `for_each`, `collect`, `reduce`, and `repeat`, not by arbitrary cycles.

`if` semantics:

- `if` is evaluated before the step primitive runs and before `try_again` is considered.
- If `if` evaluates true, the step runs normally.
- If `if` evaluates false, the step state becomes `skipped`.
- Skipped steps produce no output.
- `on_error` does not run for skipped steps.
- A skipped step with `then` continues to the `then` target.
- A skipped step without `then` continues to the next step in YAML order.
- Later steps may reference a conditionally skipped step only when control-flow validation proves the reference is reachable exclusively through a path where the step ran, or when a fallback is explicitly provided by a prior recovery step.

`then` semantics:

- `then` runs only after successful completion or skip of the current step.
- `then` overrides the natural next step.
- `then` may target a `finish` step.
- `then` may not target a previous step.
- `then` may not target a nested step outside the current scope.
- `finish` steps must not define `then`.
- `on_error.then` may target any forward normal step in the same scope.
- `choose` branches may use `then` only within their own branch scope.

## State Isolation

| Scope | State rule |
|---|---|
| Workflow | Immutable inputs, vars, and secrets. |
| Step | Immutable output after success. |
| Choose branch | Isolated branch scope. |
| Together branch | Isolated branch scope. |
| For each item | Isolated item scope. |
| Collect | Scoped durable cursor and accumulator. |
| Reduce | Scoped durable accumulator. |
| Repeat | Scoped durable attempt state. |
| Error handler | Scoped `$error` object. |

No primitive writes to global mutable state.

## `do`

`do` runs a registered action.

```yaml
steps:
  - id: create_ticket
    do: ticket.create
    with:
      title: $input.title
      body: $input.body
```

Rules:

- `do` must reference a registered action.
- `with` must validate against the action input contract.
- Unknown `with` fields are errors unless the action schema allows them.
- Action output must be an object. Scalar output is wrapped as `{ value: scalar }`.
- Side effects are at-least-once.
- Retry-safe actions must declare idempotency behavior.

Shell is just an action, not a special language feature:

```yaml
do: shell.run
```

Runtimes should disable or restrict shell by default in server mode.

## `set`

`set` creates the current step output without I/O.

```yaml
steps:
  - id: title
    set:
      text: $input.body.issue.title
```

After success:

```text
$title.text
```

Rules:

- `set` cannot mutate `input`, `vars`, secrets, previous outputs, or runtime metadata.
- `set` is deterministic.
- Retrying `set` should warn or error because there is no transient side effect.

## `choose`

`choose` selects exactly one branch.

```yaml
steps:
  - id: route
    choose:
      - if: $input.priority == "urgent"
        steps:
          - id: alert
            do: pager.alert
            with:
              message: $input.message
        result:
          kind: urgent
          id: $alert.id

      - otherwise: true
        steps:
          - id: ticket
            do: ticket.create
            with:
              body: $input.message
        result:
          kind: ticket
          id: $ticket.id
```

Downstream steps use the selected branch result:

```yaml
result:
  routed_as: $route.kind
  id: $route.id
```

Rules:

- Branches are evaluated top to bottom.
- The first true branch wins.
- `otherwise` is the default branch.
- Multiple `otherwise` branches are invalid.
- No match without `otherwise` is a runtime error.
- Unselected branch outputs are inaccessible.
- `choose` output is the selected branch result.
- `$route.choice` may expose selected branch metadata for debugging.
- Branch result shapes must match in v1 unless using the v1 tagged-result convention.

Tagged-result convention:

- Every branch result must contain the same discriminator field.
- The default discriminator field is `kind`.
- The discriminator value must be a branch-unique text literal.
- Shared downstream fields must have compatible types in every branch that defines them.
- Fields that exist in only some branches must be treated as unavailable unless guarded by a discriminator check.

Example valid tagged result:

```yaml
result:
  kind: urgent
  id: $alert.id
```

## `for_each`

`for_each` repeats work over a finite list.

```yaml
steps:
  - id: notify_all
    for_each:
      in: $input.customers
      as: customer
      at_once: 10
      per_second: 50
      do: email.send
      with:
        to: $customer.email
        subject: Hello
```

Expanded form:

```yaml
steps:
  - id: notify_all
    for_each:
      in: $input.customers
      as: customer
      at_once: 10
      steps:
        - id: send_email
          do: email.send
          with:
            to: $customer.email
```

Rules:

- `in` must resolve to a list.
- Empty list succeeds with `[]`.
- Output order must match input order.
- Runtime may execute iterations concurrently.
- `at_once` limits in-flight iterations.
- `per_second` limits iteration start rate.
- Runtime global policy can lower workflow limits.
- Failure defaults to fail-fast.
- Nested iteration step IDs are not referenced outside unless explicitly exported.

## `together`

`together` runs named branches concurrently.

```yaml
steps:
  - id: enrich
    together:
      fail: after_all
      branches:
        profile:
          do: profile.lookup
          with:
            email: $input.email

        orders:
          do: order.list
          with:
            customer_id: $input.customer_id
```

Output on success:

```text
$enrich.profile.name
$enrich.orders.count
```

Failure modes:

| Mode | Meaning |
|---|---|
| `fast` | Cancel unfinished branches after first failure and fail parent. |
| `after_all` | Let all branches finish, then fail parent if any branch failed. |
| `collect` | Parent succeeds and returns success/error per branch. |

Rules:

- Branch names must be unique and ID-like.
- Branch outputs are keyed by branch name.
- Output shape is deterministic by YAML order, not completion order.
- Successful branch outputs are recorded even when the parent fails.
- Parent output is unavailable if parent fails, except through `$error.partial` inside `on_error`.
- Successful side effects are not automatically rolled back.
- Compensation must be explicit.

Example partial recovery:

```yaml
steps:
  - id: enrich
    together:
      fail: after_all
      branches:
        profile:
          do: profile.lookup
          with:
            email: $input.email
        orders:
          do: order.list
          with:
            customer_id: $input.customer_id
    on_error:
      set:
        ok: false
        profile: $error.partial.profile.output
        orders_error: $error.partial.orders.error
```

## `collect`

`collect` handles bounded cursor pagination and unknown-length collection.

```yaml
steps:
  - id: customers
    collect:
      cursor:
        start: null
        next: $page.body.next_cursor

      page:
        do: http.get
        with:
          url: https://api.example.com/customers
          query:
            cursor: $cursor
            limit: 100

      items: $page.body.customers
      stop: $page.body.next_cursor == null

      limit:
        pages: 500
        items: 50000
        time: 5m
        wait_between: 100ms
```

Output:

```yaml
result:
  customers: $customers.items
  page_count: $customers.pages
  item_count: $customers.count
```

Rules:

- Unbounded pagination is forbidden.
- `collect` must declare limits.
- Cursor state is durable.
- Each page attempt is recorded.
- Items are appended to a durable scoped accumulator.
- Output appears only after successful completion.
- Partial output is available only inside `on_error` through `$error.partial`.
- Page, item, time, and wait limits are enforced by runtime.

Limit failure example:

```json
{
  "code": "COLLECT_LIMIT_REACHED",
  "step": "customers",
  "details": {
    "pages": 500,
    "items": 50000,
    "last_cursor": "..."
  }
}
```

## `reduce`

`reduce` accumulates data over a finite list without global mutation.

```yaml
steps:
  - id: totals
    reduce:
      in: $customers.items
      as: customer

      start:
        count: 0
        revenue: 0
        vip_customers: []

      set:
        count: $total.count + 1
        revenue: $total.revenue + $customer.spend
        vip_customers: append_if($total.vip_customers, $customer, $customer.spend > 1000)
```

Output:

```yaml
result:
  count: $totals.count
  revenue: $totals.revenue
  vip: $totals.vip_customers
```

Rules:

- `$total` exists only inside `reduce`.
- `$total` is immutable per iteration.
- Each iteration produces a new accumulator version.
- Accumulator state is durably checkpointed.
- Output appears only when `reduce` succeeds.
- Partial accumulator is available only to `on_error`.

## `repeat`

`repeat` handles bounded polling and retry-like durable checking.

```yaml
steps:
  - id: job
    do: api.start_job
    with:
      payload: $input.payload

  - id: wait_for_job
    repeat:
      do: http.get
      with:
        url: "https://api.example.com/jobs/$job.id"

      until: $attempt.body.status == "done"

      limit:
        times: 60
        time: 10m
        wait_between: 5s
```

Local roots inside `repeat`:

```text
$attempt
$attempts
```

Rules:

- `repeat` must declare limits.
- Each attempt is durable.
- Completed prior steps are not rerun after restart.
- Output is the final successful attempt.
- Failure exposes partial attempt history in `$error`.
- `repeat` is for polling and bounded checks, not arbitrary loops.

## `wait`

`wait` pauses workflow execution durably.

```yaml
steps:
  - id: cool_down
    wait: 10m
```

```yaml
steps:
  - id: wait_until
    wait:
      until: "2026-04-28T12:00:00Z"
```

```yaml
steps:
  - id: wait_for_event
    wait:
      for: payment.completed
      where: $wait_event.body.customer_id == $input.customer_id
      timeout: 1h
```

Rules:

- Waits survive restart.
- Negative durations are invalid.
- Past timestamps resume immediately.
- Event waits must declare timeout.
- Max wait is runtime policy.
- Event waits expose `$wait_event` only inside the `wait.where` expression and the resumed wait output.

Duration grammar:

```text
duration = positive_integer unit
unit = "ms" | "s" | "m" | "h" | "d"
```

Examples:

```text
100ms, 5s, 10m, 1h, 24h, 7d
```

Compound durations such as `1h30m` are invalid in v1. Use the smallest needed unit instead, for example `90m`.

## `ask`

`ask` creates a durable human input point.

```yaml
steps:
  - id: approval
    ask:
      question: Approve production deploy?
      choices:
        - approve
        - reject
      timeout: 24h
```

Output:

```text
$approval.answer
$approval.answered_by
$approval.answered_at
```

Rules:

- Prompt is durable.
- Response is untrusted input.
- Response must be validated.
- Audit trail is required.
- Self-approval is runtime policy.
- Secrets in prompts are forbidden by default.

Approval model:

- `ask.to` may identify a user, group, role, or runtime-defined approval queue.
- Runtime must authenticate the responder.
- Runtime must record responder identity, answer, timestamp, and optional comment.
- Runtime may allow delegation only if policy explicitly permits it.
- Timeout may either fail with `ASK_TIMEOUT` or use an explicit `default` answer.
- CLI responses use `twerk ask answer <run_id> <step_id> --answer <value>`.
- Server responses use the ask answer endpoint defined in `docs/server-api.md`.

## `finish`

`finish` terminates the workflow.

```yaml
steps:
  - id: done
    finish: success
```

Expanded form:

```yaml
steps:
  - id: failed
    finish:
      status: failure
      error: deploy_failed
```

Allowed statuses:

```text
success, failure, cancelled
```

Multiple finish steps are allowed as alternative terminals. One run cannot reach two terminal steps.

## `try_again`

`try_again` retries the current step primitive only after runtime failures.

```yaml
steps:
  - id: call_api
    do: http.post
    with:
      url: https://api.example.com/events
      body: $input.body
    try_again:
      times: 3
      when:
        - Http.Timeout
        - Http.RateLimited
      wait:
        type: exponential
        initial: 100ms
        max: 5s
        jitter: full
```

Rules:

- `times` is total attempts including the first.
- `times: 1` means no retry.
- Retry counters are durable.
- `on_error` runs only after retries are exhausted.
- Non-retry-safe actions require idempotency keys or runtime rejection.

## `on_error`

`on_error` handles runtime step failures after retries are exhausted.

V1 grammar supports exactly one of:

```text
then, set, finish
```

Handler `steps` are not part of v1. Use `on_error.then` to jump to normal workflow steps when multi-step recovery is needed.

Short form:

```yaml
on_error: failed
```

Equivalent form:

```yaml
on_error:
  then: failed
```

Fallback output:

```yaml
on_error:
  set:
    found: false
    name: null
```

Terminal failure:

```yaml
on_error:
  finish:
    status: failure
    error: lookup_failed
```

Rules:

- Static validation errors are not catchable.
- Cancellation is not catchable by default.
- `$error` is available only inside the handler.
- Handler failure records both original and handler error.
- Recursive handlers are forbidden.
- If `on_error.set` is used, it becomes the failed step replacement output.
- `on_error` must not contain more than one of `then`, `set`, or `finish`.

## `result`

`result` is the final workflow output mapping.

```yaml
result:
  label: $classify.label
  confidence: $classify.confidence
```

Rules:

- Omitted result defaults to `{}`.
- Missing references fail completion.
- Skipped references fail completion.
- Secret references and secret-tainted values fail by default.
- Result size is limited by runtime policy.

## Examples

Examples are executable fixtures.

```yaml
examples:
  - name: bug_report
    input:
      body:
        issue:
          title: Crash on login
    expect:
      result:
        label: bug
```

Rules:

- Real secrets are forbidden.
- Fake example secrets are still masked.
- Examples should be runnable by `twerk test` and `twerk dry-run`.

## Runtime Guarantees

- Accepted means durable.
- Queue full returns `QUEUE_FULL`.
- Step execution is at-least-once.
- External side effects are not exactly-once.
- Each run binds to an immutable workflow snapshot.
- Completed steps do not rerun during normal replay.
- Waits, asks, retries, loops, and pagination are durable.
- Loop outputs are ordered by input order.
- Parallel outputs are ordered by YAML declaration order.
- Secrets are redacted and tainted.

## Run State Machine

Allowed run states:

```text
accepted, queued, running, waiting, asking, succeeded, failed, cancelled
```

Terminal states:

```text
succeeded, failed, cancelled
```

No terminal state transitions back to running.

## Step State Machine

Allowed step states:

```text
pending, skipped, running, waiting, asking, retrying, succeeded, failed, cancelled
```

Rules:

- Completed steps must not rerun during normal replay.
- Skipped steps produce no output.
- Failed step output is unavailable unless replaced by `on_error.set`.

## Cancellation Semantics

Cancellation is a terminal control request, not an error handler path.

Rules:

- Any non-terminal run may receive a cancellation request.
- Waiting and asking runs transition to `cancelled` after the cancellation event is durably recorded.
- Pending steps become `cancelled`.
- Running actions receive a best-effort cancellation signal if the action supports it.
- External side effects already completed are not undone by cancellation.
- `on_error` does not run for cancellation by default.
- `together` cancellation sends cancellation to unfinished branches and records completed branch outputs as retained history, not parent output.
- `for_each`, `collect`, `reduce`, and `repeat` persist partial progress for inspection but do not expose successful output after cancellation.
- Cancellation completes when all locally cancellable work is stopped or marked abandoned by runtime policy.
- Cancelled runs cannot be resumed; they may be replayed as a new run.

## Validation Rules

Validation must reject:

- Duplicate YAML keys
- Unknown top-level fields
- Unknown step fields
- Missing required top-level fields
- Invalid `version`
- Invalid IDs
- Duplicate IDs
- Multiple step primitives
- Missing step primitive
- Unknown references
- Undeclared secrets
- Direct runtime references outside `inputs.from`
- Invalid `then` targets
- Control-flow cycles
- Unreachable steps
- Invalid `choose`
- Invalid `for_each`
- Invalid `together`
- Invalid `collect`
- Invalid `reduce`
- Invalid `repeat`
- Invalid `wait`
- Invalid retry policy
- Secret result leaks
- Type mismatches
- Payload limit violations

Validation should warn about:

- Unused inputs
- Unused vars
- Unused secrets
- Step outputs that are never consumed
- Retry policies on deterministic `set` steps
- Large fanout
- Large result payloads

## Error Object

Runtime errors use a stable object shape:

```json
{
  "code": "ACTION_FAILED",
  "message": "Action failed",
  "step": "classify",
  "retryable": true,
  "details": {}
}
```

Error objects must redact secrets.

## Validation Error Codes

Recommended validation codes:

```text
DUPLICATE_KEY
UNKNOWN_TOP_LEVEL_FIELD
UNKNOWN_STEP_FIELD
MISSING_REQUIRED_FIELD
INVALID_VERSION
INVALID_ID
DUPLICATE_ID
MULTIPLE_STEP_PRIMITIVES
MISSING_STEP_PRIMITIVE
UNKNOWN_REFERENCE
SECRET_NOT_DECLARED
DIRECT_RUNTIME_REFERENCE
INVALID_THEN_TARGET
CONTROL_FLOW_CYCLE
UNREACHABLE_STEP
INVALID_CHOOSE
INVALID_FOR_EACH
INVALID_TOGETHER
INVALID_COLLECT
INVALID_REDUCE
INVALID_REPEAT
INVALID_WAIT
INVALID_RETRY
SECRET_RESULT_LEAK
TYPE_MISMATCH
PAYLOAD_TOO_LARGE
```

## Runtime Error Codes

Recommended runtime codes:

```text
INPUT_MAPPING_FAILED
INPUT_TYPE_MISMATCH
SECRET_UNAVAILABLE
REFERENCE_MISSING
STEP_SKIPPED_REFERENCE
ACTION_FAILED
RETRY_EXHAUSTED
WAIT_TIMEOUT
ASK_TIMEOUT
FOR_EACH_ITEM_FAILED
TOGETHER_BRANCH_FAILED
COLLECT_LIMIT_REACHED
COLLECT_PAGE_FAILED
REDUCE_ITEM_FAILED
REPEAT_LIMIT_REACHED
RESULT_REFERENCE_MISSING
PAYLOAD_TOO_LARGE
QUEUE_FULL
```

## Safety Limits

Suggested defaults:

| Item | Limit |
|---|---:|
| YAML size | 1 MiB |
| Steps | 1000 |
| Nesting depth | 8 |
| Input after mapping | 1 MiB |
| Step output | 256 KiB |
| Result | 256 KiB |
| Parallel branches | 100 |
| Loop items | 10000 |
| Collect pages | 500 |
| Collect items | 50000 |
| Retry attempts | 10 |

Runtime policy may be stricter.

Limit enforcement rules:

- Static limits are checked during validation when possible.
- Runtime limits are checked before persistence and after action output production.
- Oversized input mapping fails the run before first step execution.
- Oversized step output fails that step with `PAYLOAD_TOO_LARGE` before exposing the output to downstream steps.
- Oversized result fails workflow completion with `PAYLOAD_TOO_LARGE`.
- Redaction does not reduce the measured size for enforcement; limits apply to original values.
- Action outputs are buffered by default and must fit the configured step output limit.
- Streaming action outputs require a separate artifact/blob contract and are not normal step outputs in v1.
- Binary values are not valid YAML values and are not valid normal action outputs; actions that produce files must return file metadata or artifact handles.

## Example: Webhook Triage

```yaml
version: twerk/v1
name: issue_triage

when:
  webhook:
    path: /github
    method: POST
    unique: request.header.X-GitHub-Delivery

inputs:
  body:
    from: request.body
    is: object
  delivery_id:
    from: request.header.X-GitHub-Delivery
    is: text

secrets:
  github_token: GITHUB_TOKEN

steps:
  - id: title
    set:
      text: $input.body.issue.title

  - id: classify
    do: ai.classify
    with:
      text: $title.text
    try_again:
      times: 3
      wait:
        type: exponential
        initial: 100ms
        max: 5s

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
          - id: comment
            do: github.issue.comment
            with:
              token: $secrets.github_token
              issue: $input.body.issue.number
              body: "Classified as $classify.label"
        result:
          kind: comment
          id: $comment.id

result:
  label: $classify.label
  route: $route.kind
  id: $route.id
```

## Open Questions

- Should numeric arithmetic stay limited to `set` and `reduce.set`, or should later versions allow it in all condition expressions?
- Should `collect` support streaming output to downstream steps, or only final output?
- Should `for_each` support `collect`-style partial outputs in `on_error`?
- Should regex be deferred entirely or allowed through a bounded engine?
- Should the default tagged-result discriminator always be `kind`, or should workflows declare a custom discriminator field?
