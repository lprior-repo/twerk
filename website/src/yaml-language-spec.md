# YAML Language Spec

This document defines the YAML shapes that Twerk currently accepts at the parser boundary, using shipped examples and core Rust schema types as evidence.

## Scope

Twerk accepts two distinct YAML document families:

1. **Native job documents** — parsed as `twerk_core::job::Job`
2. **ASL-style state machines** — parsed as `twerk_core::asl::machine::StateMachine`

Do not treat them as the same grammar. They live side-by-side in `examples/`, but they are different schemas.

## Parser Contract

At the HTTP/parser boundary (`crates/twerk-web/src/api/yaml.rs`):

- Empty bodies are rejected.
- Bodies larger than `512 KiB` are rejected.
- NUL bytes are rejected.
- Duplicate YAML keys are rejected.
- Parser budgets are enforced with `max_depth = 64` and `max_nodes = 10_000`.
- YAML must be valid UTF-8.

## Native Job Document

Backed by `crates/twerk-core/src/job.rs` and `crates/twerk-core/src/task.rs`.

### Minimal shape

Example-backed top-level fields include:

- `name`
- `description`
- `inputs`
- `output`
- `tasks`

Examples:

- `examples/hello.yaml`
- `examples/hello-shell.yaml`
- `examples/bash-pipeline.yaml`
- `examples/split_and_stitch.yaml`

### Tasks

Common task fields evidenced in shipped examples:

- `name`
- `var`
- `image`
- `run`
- `entrypoint`
- `env`
- `files`
- `retry`
- `timeout`
- `pre`
- `post`
- `mounts`
- `parallel`
- `each`
- `subjob`

Examples:

- simple task: `examples/hello.yaml`
- retry: `examples/retry.yaml`, `examples/bash-retry.yaml`
- timeout: `examples/timeout.yaml`
- map-heavy task: `examples/split_and_stitch.yaml`

## Maps

### `inputs`

Top-level string map consumed by expressions like `{{ inputs.key }}`.

Evidence:

- `examples/split_and_stitch.yaml`

### `env`

Task-level string map. Appears on normal tasks and nested tasks.

Evidence:

- `examples/each.yaml`
- `examples/bash-each.yaml`
- `examples/split_and_stitch.yaml`

### `files`

Task-level string map from filename to inline file body.

Evidence:

- `examples/split_and_stitch.yaml`

### Not example-backed yet

The core schema supports additional top-level maps/collections, but shipped `examples/*.yaml` do **not** currently prove the user-authored syntax of all of them:

- `secrets`
- `tags`
- `webhooks`
- `schedule`
- `defaults`

They may exist in code and broader docs, but they are not all evidenced by shipped example YAML.

## Control Structures

### `each`

`each` contains:

- `list`
- optional `var`
- nested `task`

Example-backed iteration placeholder forms currently seen in examples include legacy underscore aliases:

- `item_index`
- `item_value`
- `myitem_index`
- `myitem_value`
- `num_index`
- `num_value`
- `item_value_start`
- `item_value_length`

Evidence:

- `examples/each.yaml`
- `examples/bash-each.yaml`
- `examples/split_and_stitch.yaml`

### `parallel`

`parallel` contains a nested `tasks` list.

Evidence:

- `examples/parallel.yaml`
- `examples/pokemon-benchmark.yaml`
- `examples/twerk-massive-parallel.yaml`
- `examples/subjob.yaml`
- `examples/bash-subjob.yaml`

### `subjob`

`subjob` embeds another native-job-like task list and may include `name` and `output`.

Evidence:

- `examples/subjob.yaml`
- `examples/bash-subjob.yaml`

## Retry and Timeout

Native task retry shape evidenced in examples:

```yaml
retry:
  limit: <integer>
```

Timeouts are evidenced as duration strings:

- `5s`
- `120s`

Evidence:

- `examples/retry.yaml`
- `examples/bash-retry.yaml`
- `examples/timeout.yaml`
- `examples/split_and_stitch.yaml`

## ASL-Style State Machines

Backed by `crates/twerk-core/src/asl/`.

Top-level ASL fields evidenced in shipped examples:

- `comment`
- `startAt`
- `states`

Evidence:

- `examples/asl-hello.yaml`
- `examples/asl-task-retry.yaml`

### ASL state forms currently evidenced by examples

- `type: pass`
- `type: task`
- `next`
- `end: true`
- task-state `retry` list with:
  - `errorEquals`
  - `intervalSeconds`
  - `maxAttempts`
  - `backoffRate`

## Important Gaps and Ambiguities

### `run` interpolation docs are inconsistent

`website/src/examples.md` says `run` is passed raw and is **not** evaluated. But shipped examples include `run` values containing `{{ ... }}` expressions. This spec does not claim runtime interpolation semantics beyond what the parser accepts: `run` is parsed as a string, and examples prove that strings containing template markers are accepted.

### Mixed example families

`examples/` contains both native job YAML and ASL YAML. Tooling and tests must parse them into the correct target type. Parsing every example as `Job` is not a valid contract.

### Example-backed vs code-backed

This spec is intentionally conservative. If a shape is supported in code but not evidenced by shipped examples, call that out explicitly instead of bluffing.

## Recommended Reading Order

1. `website/src/QUICKSTART_YAML.md` for a quick tour
2. this document for parser-backed shape constraints
3. `website/src/examples.md` for usage-oriented examples
