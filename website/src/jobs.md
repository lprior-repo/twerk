# Jobs

A **job** is a collection of tasks executed in order.

## Minimal Example

```yaml
name: my job
tasks:
  - name: hello
    image: alpine:latest
    run: echo hello
```

## Complete Job Reference

```yaml
# ─── Identification ───────────────────────────────────────────────────────────
name: my job                           # Job name
description: Optional description       # Job description
tags: [tag1, tag2]                    # Metadata tags

# ─── Input & Secrets ─────────────────────────────────────────────────────────
inputs:                               # Non-sensitive inputs
  key: value
secrets:                              # Sensitive values (auto-redacted)
  api_key: secret123

# ─── Task Defaults ────────────────────────────────────────────────────────────
defaults:                             # Applied to all tasks
  retry:
    limit: 3                         # Max retry attempts
  limits:
    cpus: "1"                       # CPU limit
    memory: "512m"                  # Memory limit
  timeout: 10m                      # Task timeout
  queue: default                      # Queue name
  priority: 5                        # 0-9, higher = more priority

# ─── Tasks ───────────────────────────────────────────────────────────────────
tasks:
  - name: first task
    image: alpine:latest
    run: echo hello

# ─── Scheduling ───────────────────────────────────────────────────────────────
schedule:
  cron: "0 2 * * *"                 # Cron expression

# ─── Notifications ───────────────────────────────────────────────────────────
webhooks:
  - url: https://example.com/hook
    event: job.StateChange           # or task.StateChange
    if: "{{ job.state == 'COMPLETED' }}"  # Conditional

# ─── Access Control ───────────────────────────────────────────────────────────
permissions:
  - role: admin                      # Or: user: username
  - user: someuser

# ─── Cleanup ──────────────────────────────────────────────────────────────────
autoDelete:
  after: 24h                         # Delete after completion
```

## Job States

| State | Description |
|-------|-------------|
| `PENDING` | Created, not yet scheduled |
| `SCHEDULED` | Tasks queued |
| `RUNNING` | Executing |
| `COMPLETED` | All tasks finished |
| `FAILED` | Task failed |
| `CANCELLED` | Manually cancelled |

## Cron Syntax

```
┌───────────── minute (0-59)
│ ┌───────────── hour (0-23)
│ │ ┌───────────── day of month (1-31)
│ │ │ ┌───────────── month (1-12)
│ │ │ │ ┌───────────── day of week (0-6)
│ │ │ │ │
* * * * *
```

Examples:
- `0 * * * *` — Every hour
- `0 2 * * *` — Daily at 2 AM
- `0/5 * * * *` — Every 5 minutes
- `0 0 * * 0` — Weekly on Sunday

## Next Steps

- [Tasks](tasks.md) — Task configuration
- [REST API](rest-api.md) — Submit via API
