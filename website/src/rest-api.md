# REST API

Base URL: `http://localhost:8000`

## Health

### Health Check

```bash
GET /health
```

```json
{ "status": "UP" }
```

## Jobs

### Submit Job

```bash
POST /jobs
Content-Type: text/yaml
```

```yaml
name: my job
tasks:
  - name: hello
    image: alpine:latest
    run: echo hello
```

### List Jobs

```bash
GET /jobs?page=1&size=10&q=searchterm
```

```json
{
  "items": [
    {
      "id": "abc123",
      "name": "my job",
      "state": "COMPLETED",
      "createdAt": "2024-01-01T00:00:00Z"
    }
  ],
  "totalPages": 5
}
```

### Get Job

```bash
GET /jobs/{id}
```

### Get Job Log

```bash
GET /jobs/{id}/log
```

### Cancel Job

```bash
PUT /jobs/{id}/cancel
```

### Restart Job

```bash
PUT /jobs/{id}/restart
```

## Tasks

### Get Task

```bash
GET /tasks/{id}
```

### Get Task Log

```bash
GET /tasks/{id}/log
```

## Scheduled Jobs

### Submit Scheduled Job

```bash
POST /scheduled-jobs
Content-Type: text/yaml
```

```yaml
name: daily job
schedule:
  cron: "0 2 * * *"
tasks:
  - name: backup
    image: alpine:latest
    run: echo backing up
```

### List Scheduled Jobs

```bash
GET /scheduled-jobs
```

### Get Scheduled Job

```bash
GET /scheduled-jobs/{id}
```

### Pause Scheduled Job

```bash
PUT /scheduled-jobs/{id}/pause
```

### Resume Scheduled Job

```bash
PUT /scheduled-jobs/{id}/resume
```

### Delete Scheduled Job

```bash
DELETE /scheduled-jobs/{id}
```

## Queues

### List Queues

```bash
GET /queues
```

```json
[
  {
    "name": "default",
    "size": 5,
    "subscribers": 2,
    "unacked": 1
  }
]
```

### Get Queue

```bash
GET /queues/{name}
```

### Delete Queue

```bash
DELETE /queues/{name}
```

## Nodes

### List Nodes

```bash
GET /nodes
```

```json
[
  {
    "id": "node-123",
    "name": "Coordinator",
    "status": "UP",
    "startedAt": "2024-01-01T00:00:00Z",
    "lastHeartbeatAt": "2024-01-01T12:00:00Z"
  }
]
```

## Metrics

### Get Metrics

```bash
GET /metrics
```

## Users

### Create User

```bash
POST /users
Content-Type: application/json

{
  "username": "admin",
  "password": "secret"
}
```

## CLI Integration

```bash
# Submit job
curl -X POST http://localhost:8000/jobs \
  -H "Content-Type: text/yaml" \
  --data-binary @job.yaml

# Check status
curl http://localhost:8000/jobs/$JOB_ID | jq .

# Cancel
curl -X PUT http://localhost:8000/jobs/$JOB_ID/cancel
```

## Next Steps

- [Web UI](web-ui.md) — Visual interface
- [Examples](examples.md) — Complete workflows
