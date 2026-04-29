# Twerk Validator Contract

Status: draft

Audience: validator implementers, CLI implementers, UI builders, runtime implementers, action authors, and AI agents.

## Purpose

The validator is the compiler front door for Twerk. It turns friendly YAML into a strict, typed, deterministic workflow model or a complete list of actionable errors.

The validator must be good enough for AI agents to repair workflows without guessing.

```text
YAML bytes -> parse tree -> normalized document -> typed workflow model -> compiled graph -> diagnostics
```

## Core Guarantees

- Validation is deterministic.
- Unknown fields are errors.
- Duplicate keys are errors.
- YAML profile violations are errors.
- References are checked before execution when possible.
- Action `with` objects are checked against action schemas.
- Security policy violations are reported before registration when possible.
- Diagnostics include source locations and repair suggestions where practical.
- Machine-readable output is stable across releases within a major version.

## Validator Inputs

Validation accepts:

| Input | Required | Description |
|---|---:|---|
| Workflow YAML bytes | Yes | Raw document. |
| Workflow path/name | No | Used for source locations and diagnostics. |
| Action registry snapshot | Yes for full validation | Action manifests and schemas. |
| Runtime policy snapshot | No for syntax validation, yes for registration validation | Security and capability rules. |
| Secret binding metadata | No | Names and availability, not values. |
| Target runtime limits | No | Step count, payload size, nesting, etc. |
| Validation mode | Yes | `syntax`, `portable`, `registry`, `policy`, `registration`, `example`. |

Validation modes:

| Mode | Purpose |
|---|---|
| `syntax` | Parse YAML profile, closed schema, local shape. No action registry needed. |
| `portable` | Validate language-only semantics. No host-specific actions beyond name shape. |
| `registry` | Validate actions and `with`/output schemas against registry. |
| `policy` | Validate capabilities, secrets, network/file/process policy. |
| `registration` | Full validation before workflow registration. |
| `example` | Validate and execute examples in dry-run/test context. |

## Validation Phases

The validator should run phases in a stable order so diagnostics are predictable.

1. Decode bytes as UTF-8.
2. Parse restricted YAML.
3. Reject duplicate keys and forbidden YAML features.
4. Validate top-level closed schema.
5. Validate step closed schema.
6. Normalize shorthand forms.
7. Validate names, IDs, and reserved words.
8. Validate trigger shape and runtime source mapping.
9. Normalize and validate type schemas.
10. Validate `vars` constants and defaults.
11. Validate secret declarations.
12. Parse references and expressions.
13. Validate scopes and reference existence.
14. Build control-flow graph.
15. Validate graph reachability, cycles, terminals, and `then` targets.
16. Validate primitive-specific semantics.
17. Validate action existence and `with` schemas.
18. Infer and validate step output schemas.
19. Validate `result` mapping and secret taint.
20. Validate examples.
21. Validate runtime limits and policy.
22. Emit diagnostics and normalized model.

Implementations may short-circuit after parse failure, but should collect multiple independent errors after parsing succeeds.

## Restricted YAML Validation

Must reject:

- Duplicate mapping keys at any depth.
- Anchors.
- Aliases.
- Merge keys.
- Custom tags.
- Binary scalars.
- Non-UTF-8 input.
- Parser-specific YAML 1.1 booleans such as unquoted `yes`, `no`, `on`, `off` if parsed ambiguously.
- Non-finite numbers.

Allowed YAML values are JSON-compatible:

- object
- list
- string
- finite number
- boolean
- null

Comments are ignored and never carry semantics.

## Closed Schema Validation

Allowed top-level fields:

```text
version
name
when
inputs
vars
secrets
steps
result
examples
```

Allowed step fields:

```text
id
name
if
with
set
choose
for_each
together
collect
reduce
repeat
wait
ask
try_again
on_error
then
finish
```

Unknown fields are errors.

Extension fields are not part of v1. If extensions are later added, they must be version-gated and ignored only when explicitly declared safe.

## Normalization

The validator produces a canonical normalized model.

Examples:

Short input form:

```yaml
inputs:
  email: text
```

Normalizes to:

```json
{
  "email": {
    "is": "text",
    "optional": false,
    "nullable": false
  }
}
```

String action form remains explicit:

```yaml
do: github.issue.comment
```

Normalizes to:

```json
{
  "primitive": "do",
  "action": "github.issue.comment"
}
```

Normalization must not change behavior.

## Type Schema Normalization

Canonical type schema shape:

```json
{
  "is": "object",
  "optional": false,
  "nullable": false,
  "fields": {
    "email": {
      "is": "text",
      "optional": false,
      "nullable": false
    }
  },
  "extra": false
}
```

Supported `is` values:

```text
text
number
boolean
object
list
any
```

Rules:

- `optional: true` allows absent fields.
- `nullable: true` allows present `null` values.
- `default` applies to missing values, not present null, unless runtime policy later defines null-defaulting.
- `list` may define `of`.
- `object` may define `fields` and `extra`.
- `extra: false` rejects undeclared fields.
- `extra: true` allows undeclared fields of any type.
- `extra` may be a schema object for typed additional fields.

## Name And ID Validation

Step IDs, loop variable names, branch names, input names, var names, and secret aliases must match:

```text
^[a-z][a-z0-9_]{0,63}$
```

Reserved names:

```text
input
inputs
vars
secrets
steps
result
when
item
branch
error
wait_event
true
false
null
do
set
choose
for_each
together
collect
reduce
repeat
wait
ask
finish
if
then
on_error
try_again
```

Rules:

- All step IDs are globally unique in v1, including nested step IDs.
- Loop variables cannot collide with step IDs or reserved names.
- Branch names cannot collide inside the same `together` step.
- Action names use dotted action registry syntax and are not step IDs.

## One Primitive Per Step

Each step must contain exactly one primitive:

```text
do
set
choose
for_each
together
collect
reduce
repeat
wait
ask
finish
```

Metadata/control fields do not count as primitives:

```text
id
name
if
with
try_again
on_error
then
```

Invalid:

```yaml
- id: bad
  do: email.send
  set:
    ok: true
```

Diagnostic code: `MULTIPLE_STEP_PRIMITIVES`.

## Reference Parsing

References use the language grammar from `docs/language-spec.md`.

The validator must distinguish:

- Whole-scalar references that preserve native type.
- Embedded interpolation that produces text.
- Expression contexts such as `if`, `choose.if`, `wait.where`, `repeat.until`, and `reduce.set`.

Allowed roots:

```text
$input
$vars
$secrets
$error
$wait_event
$step_id
$loop_variable
```

Rules:

- `$input.x` must reference declared input `x`.
- `$vars.x` must reference declared var `x`.
- `$secrets.x` must reference declared secret alias `x`.
- `$step_id.x` must reference a globally declared step ID and an output field known from schema when possible.
- `$loop_variable.x` is only valid inside that loop scope.
- `$error.x` is only valid inside `on_error` contexts.
- `$wait_event.x` is only valid inside event wait filters and resumed wait output handling.
- Direct runtime roots such as `$request` and `$event` are invalid outside `inputs.from`.

## Expression Validation

Expression grammar is intentionally small.

The validator must parse and type-check:

- Boolean operators: `and`, `or`, `not`.
- Comparisons: `==`, `!=`, `>`, `>=`, `<`, `<=`.
- Numeric arithmetic: `+`, `-`, `*`, `/` in allowed contexts.
- Parentheses.
- String, number, boolean, and null literals.
- Approved helper functions: `exists`, `has`, `contains`, `starts_with`, `ends_with`, `length`, `empty`, `append`, `append_if`, `merge`, `sum`, `count`, `unique` where allowed.

No arbitrary functions, object literals, list literals, regex, JS, Python, jq, or shell evaluation.

Missing path rules:

- Normal missing references are errors.
- `exists(path)` may probe missing paths without error.
- `has(object, key)` returns false if key is absent.
- `and` and `or` short-circuit; the skipped branch is not evaluated.

## Control-Flow Graph Validation

The validator builds a graph from:

- Step order.
- `then` targets.
- `choose` branches.
- `for_each` bodies.
- `together` branches.
- `collect` page/item flows.
- `reduce` bodies.
- `repeat` bodies.
- `on_error.then` targets.
- `finish` terminals.

Rules:

- Default flow is the next step in list order.
- `then` overrides default flow after success or skip.
- `then` may target only a forward step in the same scope.
- Backward jumps are forbidden in v1.
- Jumps into or out of nested scopes are forbidden.
- `finish` is terminal and cannot define `then`.
- Steps after unconditional finish are unreachable.
- Unreachable steps are errors unless a future version defines alternate entry points.
- Arbitrary cycles are forbidden.
- Structured repetition is allowed only through `for_each`, `collect`, `reduce`, and `repeat`.

## Skipped Step Safety

A step skipped by `if` produces no output.

The validator should reject references to possibly skipped outputs unless:

- Control flow proves the producer ran.
- The reference is in a branch guarded by the same condition.
- The reference has an explicit fallback supported by the expression language.
- The step has `on_error.set` or other replacement output covering the failure path.

If static proof is not feasible, the validator must emit a warning or error according to validation mode. Registration mode should treat unsafe skipped references as errors.

## Primitive Validation

### `do`

Checks:

- Action exists in registry for `registry` and stronger modes.
- `with` validates against action input schema.
- Unknown `with` fields are rejected unless action schema allows extra fields.
- Action output schema is known or explicitly open.
- Retry policy is compatible with action retry safety.
- Required capabilities are allowed by policy.
- Required secrets are declared and bindable.

### `set`

Checks:

- Contains an object mapping.
- Performs no I/O.
- Does not mutate `input`, `vars`, `secrets`, previous step outputs, or outer scopes.
- Output schema is inferred from mapping when possible.

### `choose`

Checks:

- Branch conditions are boolean expressions.
- At most one `otherwise` branch.
- Branch outputs have matching shape, or use the tagged-result convention.
- Branch-local steps obey global ID uniqueness.
- Missing default branch behavior is explicit according to language rules.

### `for_each`

Checks:

- Input resolves to list type when statically known.
- `as` loop variable is valid and unique.
- `at_once` and `per_second` are within runtime limits when present.
- Body steps do not escape loop scope.
- Output is deterministic and ordered by input item order.

### `together`

Checks:

- Branch names are unique and valid.
- Branch count is within limits.
- Failure mode is valid.
- Throttle values are within limits.
- Branch outputs are namespaced by branch name.

### `collect`

Checks:

- Page action exists and declares cursor behavior.
- Cursor path is valid.
- Item path resolves to a list.
- Hard limits exist for pages, items, bytes, and runtime.
- Accumulator shape is valid.
- No unbounded pagination is possible.

### `reduce`

Checks:

- Input resolves to list type.
- Initial accumulator matches accumulator schema.
- Reducer expressions only use current item, accumulator, and allowed prior values.
- Output matches declared accumulator/result shape.

### `repeat`

Checks:

- `until` is a boolean expression.
- Hard limits exist for attempts and total runtime.
- Wait interval is valid.
- Body is restart-safe and bounded.

### `wait`

Checks:

- Duration grammar is valid.
- Absolute timestamps are RFC3339 with timezone.
- Event wait filters use only allowed `$wait_event` and workflow values.
- Timeout behavior is explicit for event waits.

### `ask`

Checks:

- Prompt exists.
- Prompt is not secret-tainted.
- Recipient/role/queue is valid where statically knowable.
- Response schema or choices are valid.
- Timeout behavior is explicit.

### `finish`

Checks:

- Status is `success`, `failure`, or `cancelled`.
- No `then` exists on finish step.
- Result/finish payload does not leak secrets.

## Action Registry Validation

In registry mode and stronger, the validator must load action manifests from a stable registry snapshot.

For each `do` action:

- Validate action name.
- Resolve exact action version or selected default.
- Validate `with` input schema.
- Record output schema.
- Record side-effect class.
- Record retry safety.
- Record capabilities.
- Record timeout constraints.
- Record UI metadata for builder diagnostics.

If an action is unavailable, emit `UNKNOWN_ACTION`.

## Security Policy Validation

In policy mode and stronger, the validator checks:

- Workflow trigger is allowed.
- Webhook verification policy is valid.
- Action capabilities are allowed.
- Secret declarations are bindable.
- Shell/process actions are explicitly allowed.
- Static file paths and URLs obey policy.
- Ask targets are allowed when statically known.
- `result`, logs, ask prompts, and webhook replies do not receive secret-tainted values by default.

Policy diagnostics should include capability names and source locations.

## Output Schema Inference

Each step must have an output schema.

Sources:

- Action manifest output schema.
- `set` mapping inferred schema.
- `choose` branch output merge/tagged schema.
- `for_each` list of body result schemas.
- `together` object keyed by branch names.
- `collect` accumulator/page/item schema.
- `reduce` accumulator schema.
- `repeat` last successful body output or declared result schema.
- `wait` wait result schema.
- `ask` answer schema.
- `finish` terminal payload schema.

Unknown output schemas are allowed only when the producing action declares open output. References into unknown open outputs should be warnings in authoring mode and may be errors in registration mode if strict typing is required.

## Diagnostics

Diagnostic shape:

```json
{
  "severity": "error",
  "code": "UNKNOWN_REFERENCE",
  "message": "Unknown input '$input.emali'. Did you mean '$input.email'?",
  "source": {
    "path": "flows/issue_triage.yaml",
    "line": 42,
    "column": 13,
    "pointer": "/steps/2/with/to"
  },
  "details": {
    "reference": "$input.emali",
    "suggestion": "$input.email"
  }
}
```

Severities:

| Severity | Meaning |
|---|---|
| `error` | Workflow cannot be accepted. |
| `warning` | Workflow can run but may be unsafe, brittle, or non-portable. |
| `info` | Helpful explanation. |

Diagnostics must not include secret values.

## Diagnostic Codes

Parse/schema:

```text
INVALID_UTF8
INVALID_YAML
DUPLICATE_KEY
FORBIDDEN_YAML_FEATURE
UNKNOWN_TOP_LEVEL_FIELD
UNKNOWN_STEP_FIELD
MISSING_REQUIRED_FIELD
INVALID_VERSION
```

Names/references:

```text
INVALID_ID
DUPLICATE_ID
RESERVED_NAME
UNKNOWN_REFERENCE
AMBIGUOUS_REFERENCE
DIRECT_RUNTIME_REFERENCE
OUT_OF_SCOPE_REFERENCE
POSSIBLY_SKIPPED_REFERENCE
```

Types/expressions:

```text
TYPE_MISMATCH
INVALID_TYPE_SCHEMA
INVALID_NULLABILITY
INVALID_EXPRESSION
INVALID_FUNCTION
MISSING_PATH
DIVISION_BY_ZERO_STATIC
```

Control flow:

```text
MULTIPLE_STEP_PRIMITIVES
MISSING_STEP_PRIMITIVE
INVALID_THEN_TARGET
CONTROL_FLOW_CYCLE
UNREACHABLE_STEP
INVALID_FINISH
MULTIPLE_TERMINALS_REACHABLE
```

Primitives:

```text
INVALID_CHOOSE
INVALID_FOR_EACH
INVALID_TOGETHER
INVALID_COLLECT
INVALID_REDUCE
INVALID_REPEAT
INVALID_WAIT
INVALID_ASK
INVALID_RETRY
INVALID_ON_ERROR
```

Registry/security:

```text
UNKNOWN_ACTION
ACTION_INPUT_INVALID
ACTION_OUTPUT_UNKNOWN
ACTION_RETRY_UNSAFE
CAPABILITY_DENIED
ACTION_DENIED
SECRET_NOT_DECLARED
SECRET_UNAVAILABLE
SECRET_LEAK_BLOCKED
WEBHOOK_VERIFICATION_INVALID
EGRESS_DENIED
FILE_ACCESS_DENIED
PROCESS_DENIED
SHELL_DENIED
```

Limits/examples:

```text
PAYLOAD_LIMIT_EXCEEDED
STEP_LIMIT_EXCEEDED
NESTING_LIMIT_EXCEEDED
EXAMPLE_INVALID
EXAMPLE_EXPECTATION_FAILED
```

## Machine Output

`twerk validate --json` should return:

```json
{
  "ok": false,
  "mode": "registration",
  "workflow": "issue_triage",
  "digest": "sha256:...",
  "diagnostics": [],
  "summary": {
    "errors": 2,
    "warnings": 1,
    "infos": 0
  }
}
```

If valid, include normalized and compiled summaries when requested:

```json
{
  "ok": true,
  "workflow": "issue_triage",
  "digest": "sha256:...",
  "compiled_ir_digest": "sha256:...",
  "steps": 12,
  "actions": ["http.post", "github.issue.comment"],
  "capabilities": ["network.http", "github.issue.write"],
  "secrets": ["github_token"]
}
```

## UI Integration

The validator should support partial validation for UI editing.

Required UI features:

- Validate one field without losing whole-document diagnostics.
- Return source pointers for cards and inspector fields.
- Return action schema for selected `do` action.
- Return data picker suggestions from currently available references.
- Return warnings for references to future or skipped steps.
- Return quick-fix suggestions when safe.

Quick-fix examples:

- Replace misspelled `$input.emali` with `$input.email`.
- Add missing secret declaration.
- Move step before first reference.
- Replace `shell.run` with suggested typed action.
- Add `on_error` for unsafe external action.

## AI Repair Integration

Diagnostics must be explicit enough for an AI agent to repair YAML.

AI-facing output should include:

- Error code.
- Source pointer.
- Exact invalid value.
- Expected shape.
- Known alternatives.
- Minimal patch suggestion when safe.
- Links to relevant docs section identifiers where available.

The validator should avoid vague messages such as “invalid workflow.”

## Deterministic Compiled Model

After validation, the compiler emits a deterministic model with:

- Workflow metadata.
- Normalized input schemas.
- Normalized vars.
- Secret aliases.
- Step table with stable indexes.
- Control-flow edges.
- Action bindings.
- Output schemas.
- Capability requirements.
- Source location map.
- Layout hints if a layout sidecar exists.

The compiled model must hash deterministically.

The runtime binds each run to the compiled model digest.

## Open Questions

1. Should registration mode always reject open action output schemas, or allow them with warnings?
2. Should skipped-reference safety be a strict static error in all modes, or only in registration mode?
3. Should quick-fix patches be emitted as JSON Patch, unified diff, or a Twerk-specific edit format?
4. Should validation support `x-*` extension fields in v1, or keep the schema completely closed until v2?
5. Should the validator expose a long-lived language server protocol for the UI, or only CLI/API validation calls?
