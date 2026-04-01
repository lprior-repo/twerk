# Tasks

Tasks are the unit of execution in Twerk.

## Minimal Task

```yaml
- name: my task
  image: alpine:latest
  run: echo hello
```

## What Fields Support Expressions?

Expressions using `{{ }}` syntax are supported in these fields:
- `name` — Task name
- `image` — Container image
- `var` — Output variable name
- `queue` — Target queue
- `if` — Conditional execution
- `env` values — Environment variables
- `files` keys/values — Files to create

**Note:** The `run` field is NOT evaluated — it's passed as raw shell script.

## Complete Task Reference

```yaml
# ─── Identification ───────────────────────────────────────────────────────────
name: my task                        # Supports {{ }} expressions
description: Optional description    # Plain text only

# ─── Container ───────────────────────────────────────────────────────────────
image: ubuntu:mantic                 # Supports {{ }} expressions
cmd: ["/bin/sh", "-c"]             # Override entrypoint
entrypoint: ["/bin/sh", "-c"]       # Same as cmd
run: |
  echo hello                        # RAW shell script - NO expression evaluation

# ─── Environment ──────────────────────────────────────────────────────────────
env:                                 # Values support {{ }} expressions
  KEY: value
  TEMPLATE: '{{ inputs.key }}'      # ✓ Works

files:                               # Keys and values support {{ }}
  config.json: '{"key": "{{ inputs.value }}"}'  # ✓ Works

# ─── Output ─────────────────────────────────────────────────────────────────
var: output_key                      # Supports {{ }} - store task output
                                     # Access via {{ tasks.output_key }}

# ─── Conditions ──────────────────────────────────────────────────────────────
if: "{{ job.state == 'SCHEDULED' }}"  # ✓ Works in if field

# ─── Routing ────────────────────────────────────────────────────────────────
queue: default                       # Supports {{ }} expressions
priority: 5

# ─── Execution Control ───────────────────────────────────────────────────────
timeout: 5m
retry:
  limit: 3

# ─── Resources ────────────────────────────────────────────────────────────────
limits:
  cpus: "0.5"
  memory: "256m"

gpus: all
workdir: /app

# ─── Mounts ──────────────────────────────────────────────────────────────────
mounts:
  - type: volume
    target: /data

# ─── Pre/Post Tasks ─────────────────────────────────────────────────────────
pre:
  - name: setup
    image: alpine:latest
    run: echo setup

post:
  - name: cleanup
    image: alpine:latest
    run: echo cleanup

# ─── Parallel ────────────────────────────────────────────────────────────────
parallel:
  tasks:
    - name: a
      image: alpine:latest
      run: echo A
    - name: b
      image: alpine:latest
      run: echo B

# ─── Each (Loop) ────────────────────────────────────────────────────────────
each:
  list: '{{ fromJSON(inputs.items) }}'  # Expression for list
  concurrency: 2
  task:
    image: alpine:latest
    env:
      VALUE: '{{ item.value }}'        # ✓ Works in each tasks
      INDEX: '{{ item.index }}'
    run: echo $VALUE
```

## Supported Expression Syntax

### Input/Secret References

```yaml
env:
  VALUE: '{{ inputs.my_input }}'      # Job input
  SECRET: '{{ secrets.my_secret }}'   # Job secret (auto-redacted)
```

### Each Loop Variables

```yaml
each:
  task:
    env:
      VALUE: '{{ item.value }}'        # Current item value
      INDEX: '{{ item.index }}'       # Current index (0-based)
```

### Built-in Functions

```yaml
env:
  JSON: '{{ fromJSON(inputs.json_string) }}'
  SEQ: '{{ sequence(1, 5) }}'         # [1, 2, 3, 4]
  LEN: '{{ len(tasks.results) }}'
  SPLIT: '{{ split("a,b,c", ",") }}'  # ["a", "b", "c"]
```

### Conditional with `if`

```yaml
if: "{{ job.state == 'SCHEDULED' }}"  # Job must be scheduled
if: "{{ job.state != 'FAILED' }}"      # Job not failed
```

## Task States

| State | Description |
|-------|-------------|
| `CREATED` | Task created |
| `PENDING` | Queued for execution |
| `SCHEDULED` | Assigned to worker |
| `RUNNING` | Executing |
| `COMPLETED` | Finished successfully |
| `FAILED` | Failed |
| `CANCELLED` | Cancelled |
| `STOPPED` | Stopped |
| `SKIPPED` | Skipped (conditional) |

## Next Steps

- [Runtimes](runtimes.md) — Docker, Podman, Shell
- [Configuration](configuration.md) — Full configuration reference
