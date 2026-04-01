# Web UI

Twerk Web provides a visual interface for managing jobs.

## Running Tork Web

```bash
docker run -it --rm \
  --name twerk-web \
  -p 3000:3000 \
  -e BACKEND_URL=http://localhost:8000 \
  runabol/tork-web
```

Access at `http://localhost:3000`

## Features

- **Job List** — View all jobs with state, timing, and search
- **Job Details** — Inspect task execution, logs, and outputs
- **Submit Jobs** — Upload YAML job definitions
- **Cancel/Restart** — Control running jobs
- **Scheduled Jobs** — Manage cron jobs
- **Nodes** — Monitor coordinators and workers
- **Queues** — View queue sizes and subscribers
- **Users** — Manage access

## Screenshots

### Jobs List
View all jobs with filtering by state and full-text search.

### Job Detail
See task execution order, timing, logs, and output.

### Submit Job
Upload YAML or write job definition directly.

### Nodes
Monitor active coordinators and workers with heartbeats.

## Next Steps

- [REST API](rest-api.md) — API-first management
- [Examples](examples.md) — Complete workflows
