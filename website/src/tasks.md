# Tasks

Tasks are the unit of execution in Twerk.

## Minimal Task

```yaml
- name: my task
  image: alpine:latest
  run: echo hello
```

## Complete Task Reference

```yaml
# ─── Identification ───────────────────────────────────────────────────────────
name: my task                        # Task name
description: Optional description    # Task description
tags: [tag1, tag2]                  # Metadata tags

# ─── Container ───────────────────────────────────────────────────────────────
image: ubuntu:mantic                 # Container image
cmd: ["/bin/sh", "-c"]              # Override entrypoint
entrypoint: ["/bin/sh", "-c"]       # Same as cmd
run: |
  echo hello
  echo world

# ─── Registry ────────────────────────────────────────────────────────────────
registry:                            # Private registry credentials
  username: user
  password: secret

# ─── Environment ─────────────────────────────────────────────────────────────
env:                                 # Environment variables
  KEY: value
  TEMPLATE: '{{ inputs.key }}'        # Expression support

files:                               # Files to create in working dir
  script.py: |
    print("hello")

# ─── Output ──────────────────────────────────────────────────────────────────
var: output_key                      # Store output under this key
                                     # Use {{ tasks.output_key }} to reference

# ─── Conditions ──────────────────────────────────────────────────────────────
if: "{{ inputs.run == 'true' }}"     # Conditional execution

# ─── Routing ─────────────────────────────────────────────────────────────────
queue: default                       # Target queue
priority: 5                          # 0-9, higher = more priority

# ─── Execution Control ───────────────────────────────────────────────────────
timeout: 5m                          # Max execution time
retry:
  limit: 3                           # Max retries
  attempts: 0                        # Current attempt count (internal)

# ─── Resources ────────────────────────────────────────────────────────────────
limits:
  cpus: "0.5"                        # CPU limit
  memory: "256m"                     # Memory limit

gpus: all                           # GPU access (Docker only)

workdir: /app                        # Working directory

# ─── Networking ──────────────────────────────────────────────────────────────
networks:                            # Container networks
  - my-network

# ─── Storage ─────────────────────────────────────────────────────────────────
mounts:
  - type: volume                      # or: bind, tmpfs
    target: /data                    # Mount point
    source: /host/path                # For bind mounts

# ─── Pre/Post Tasks ─────────────────────────────────────────────────────────
pre:                                 # Run before main task
  - name: setup
    image: alpine:latest
    run: echo setup

post:                                # Run after main task
  - name: cleanup
    image: alpine:latest
    run: echo cleanup

# ─── Parallel Execution ──────────────────────────────────────────────────────
parallel:
  tasks:
    - name: task a
      image: alpine:latest
      run: echo A
    - name: task b
      image: alpine:latest
      run: echo B
  completions: 2                      # Wait for N tasks to complete

# ─── Each (Loop) ─────────────────────────────────────────────────────────────
each:
  var: item_output                    # Output variable name
  list: '{{ sequence(1, 5) }}'        # Items to iterate
  concurrency: 2                      # Max parallel executions
  size: 5                             # Total items (internal)
  index: 0                            # Current index (internal)
  completions: 0                      # Completed count (internal)
  task:                               # Task template
    name: process item
    image: alpine:latest
    env:
      VALUE: '{{ item.value }}'
    run: echo $VALUE

# ─── Sub-Job ─────────────────────────────────────────────────────────────────
subjob:
  name: my sub job                    # Sub-job name
  description: Optional               # Sub-job description
  tasks:                             # Sub-job tasks
    - name: sub task
      image: alpine:latest
      run: echo sub
  inputs:                            # Sub-job inputs
    key: value
  secrets:                           # Sub-job secrets
    key: value
  autoDelete:                        # Sub-job auto-delete
    after: 1h
  detached: false                    # Wait for completion if false
  webhooks:                          # Sub-job webhooks
    - url: https://example.com/hook
      event: job.StateChange

# ─── Health Check ─────────────────────────────────────────────────────────────
probe:
  path: /health                      # Health check path
  port: 8080                         # Health check port
  timeout: 5s                        # Health check timeout

# ─── Sidecars ────────────────────────────────────────────────────────────────
sidecars:                            # Sidecar containers
  - name: proxy
    image: envoyproxy/envoy:latest
    run: echo proxy
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

## Built-in Functions

Available in expressions:

- `len(array)` — Array length
- `first(array)` — First element
- `last(array)` — Last element
- `contains(array, item)` — Check membership
- `sequence(start, end)` — Generate number range
- `fromJSON(string)` — Parse JSON string
- `toJSON(value)` — Convert to JSON string

## Next Steps

- [Runtimes](runtimes.md) — Docker, Podman, Shell
- [Configuration](configuration.md) — Runtime configuration
