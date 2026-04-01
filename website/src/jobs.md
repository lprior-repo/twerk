# Jobs

A **job** is a collection of tasks executed in order.

## Simple Example

```yaml
name: my job
tasks:
  - name: first task
    image: ubuntu:mantic
    run: echo "Hello from task 1"
  - name: second task
    image: alpine:latest
    run: echo "Hello from task 2"
```

## Job Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Job name |
| `description` | string | Optional description |
| `tags` | list | Metadata tags |
| `inputs` | map | Key-value pairs accessible via `{{ inputs.key }}` |
| `secrets` | map | Sensitive values, auto-redacted in responses |
| `defaults` | object | Default values for all tasks |
| `tasks` | list | Ordered list of tasks |
| `webhooks` | list | Notifications on state changes |
| `permissions` | list | Access control |
| `autoDelete` | object | Auto-cleanup after completion |
| `schedule` | object | Cron-based scheduling |

## Inputs

Pass data to jobs:

```yaml
name: video processor
inputs:
  source_url: https://example.com/video.mov
  output_format: mp4
tasks:
  - name: download video
    image: alpine:latest
    env:
      SOURCE: '{{ inputs.source_url }}'
    run: wget $SOURCE -O /tmp/video.mov
```

## Secrets

Store sensitive values securely:

```yaml
name: api job
secrets:
  api_key: 1111-1111-1111-1111
tasks:
  - name: call api
    image: alpine:latest
    env:
      API_KEY: '{{ secrets.api_key }}'
    run: curl -H "Authorization: Bearer $API_KEY" https://api.example.com
```

Secrets are auto-redacted in API responses and logs.

## Defaults

Set defaults for all tasks:

```yaml
name: my job
defaults:
  retry:
    limit: 3
  limits:
    cpus: 1
    memory: 500m
  timeout: 10m
  queue: default
  priority: 5
tasks:
  - name: task 1
    image: alpine:latest
    run: echo hello
  - name: task 2
    image: alpine:latest
    run: echo world
```

## Scheduled Jobs

Run jobs on a cron schedule:

```yaml
name: daily backup
schedule:
  cron: "0 2 * * *"  # 2 AM daily
tasks:
  - name: backup database
    image: postgres:latest
    run: pg_dump all > /backups/db.sql
```

Submit to scheduler:

```bash
curl -X POST http://localhost:8000/scheduled-jobs \
  -H "Content-type: text/yaml" \
  --data-binary @scheduled.yaml
```

**Scheduled Job Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `cron` | string | Cron expression |
| `state` | string | ACTIVE or PAUSED |
| `inputs` | map | Job inputs |
| `tasks` | list | Job tasks |
| `defaults` | object | Task defaults |
| `webhooks` | list | Notifications |
| `autoDelete` | object | Auto-cleanup |

**Scheduled Job API:**

- `GET /scheduled-jobs` — List all
- `GET /scheduled-jobs/{id}` — Get one
- `PUT /scheduled-jobs/{id}/pause` — Pause
- `PUT /scheduled-jobs/{id}/resume` — Resume
- `DELETE /scheduled-jobs/{id}` — Delete

## Webhooks

Get notified on job events:

```yaml
name: my job
webhooks:
  - url: https://myapp.com/webhook
    event: job.StateChange
    if: "{{ job.state == 'COMPLETED' }}"
tasks:
  - name: my task
    image: alpine:latest
    run: echo hello
```

**Webhook Events:**
- `job.StateChange` — Any job state transition
- `task.StateChange` — Any task state transition

## Auto Delete

Automatically clean up completed jobs:

```yaml
name: temp job
autoDelete:
  after: 24h  # Delete 24 hours after completion
tasks:
  - name: process
    image: alpine:latest
    run: echo processing
```

## Job States

| State | Description |
|-------|-------------|
| `PENDING` | Created, not yet scheduled |
| `SCHEDULED` | Tasks queued for execution |
| `RUNNING` | At least one task executing |
| `COMPLETED` | All tasks finished successfully |
| `FAILED` | Task failed without retry |
| `CANCELLED` | Manually cancelled |
| `RESTART` | Being restarted |

## Next Steps

- [Tasks](tasks.md) — Deep dive into task configuration
- [REST API](rest-api.md) — API reference
