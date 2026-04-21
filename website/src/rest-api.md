# REST API

Base URL: `http://localhost:8000`

## Health

```bash
GET /health
```

```json
{ "status": "UP", "version": "0.1.0" }
```

## Jobs

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/jobs` | Submit a job |
| `POST` | `/jobs?wait=true` | Submit and block until completion |
| `GET` | `/jobs` | List jobs |
| `GET` | `/jobs/{id}` | Get job details |
| `GET` | `/jobs/{id}/log` | Fetch job logs |
| `POST` | `/jobs/{id}/cancel` | Cancel a job |
| `PUT` | `/jobs/{id}/cancel` | Cancel a job |
| `PUT` | `/jobs/{id}/restart` | Restart a job |

Example request body:

```yaml
name: hello shell
tasks:
  - name: hello
    run: echo "hello from twerk"
```

## Tasks

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/tasks/{id}` | Get task details |
| `GET` | `/tasks/{id}/log` | Fetch task logs |

## Scheduled Jobs

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/scheduled-jobs` | Create a scheduled job |
| `GET` | `/scheduled-jobs` | List scheduled jobs |
| `GET` | `/scheduled-jobs/{id}` | Get a scheduled job |
| `PUT` | `/scheduled-jobs/{id}/pause` | Pause a scheduled job |
| `PUT` | `/scheduled-jobs/{id}/resume` | Resume a scheduled job |
| `DELETE` | `/scheduled-jobs/{id}` | Delete a scheduled job |

## Queues

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/queues` | List queues |
| `GET` | `/queues/{name}` | Get queue details |
| `DELETE` | `/queues/{name}` | Delete a queue |

## System

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/nodes` | List nodes |
| `GET` | `/metrics` | Fetch metrics |
| `POST` | `/users` | Create a user |

## Triggers

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/triggers` | List triggers |
| `POST` | `/api/v1/triggers` | Create a trigger |
| `GET` | `/api/v1/triggers/{id}` | Get a trigger |
| `PUT` | `/api/v1/triggers/{id}` | Update a trigger |
| `DELETE` | `/api/v1/triggers/{id}` | Delete a trigger |

## OpenAPI

```bash
GET /openapi.json
```

## CLI Integration

```bash
# Submit and wait for completion
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @examples/hello-shell.yaml

# Inspect the resulting job and logs
curl http://localhost:8000/jobs/$JOB_ID
curl http://localhost:8000/jobs/$JOB_ID/log
```

## Next Steps

- [Web UI](web-ui.md) — Visual interface
- [Examples](examples.md) — Complete workflows
