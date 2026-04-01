# Introduction

**Twerk** is a distributed task execution system written in Rust — a port of [Tork](https://github.com/runabol/tork) from Go. It lets you define jobs consisting of multiple tasks, each running inside its own container.

## Why Twerk?

- **Horizontally scalable** — Add workers to handle more tasks
- **Task isolation** — Tasks run in containers with resource limits
- **Multi-runtime** — Docker, Podman, or Shell execution
- **Retry with backoff** — Configurable retry on failure
- **Scheduled jobs** — Cron-based scheduling with pause/resume
- **Secrets management** — Auto-redaction of sensitive values
- **REST API** — Full API for job, task, queue, node, and user management
- **Health checks** — Built-in liveness and readiness probes

## Architecture

```
Client → Coordinator → Broker → Worker → Runtime (Docker/Podman/Shell)
                ↓
            Datastore (PostgreSQL)
```

- **Coordinator** — Receives jobs, schedules tasks, manages state
- **Worker** — Executes tasks via the configured runtime
- **Broker** — Routes tasks between Coordinator and Workers (RabbitMQ or In-Memory)
- **Datastore** — Persists all job, task, and node state (PostgreSQL)

## Modes

| Mode | Description |
|------|-------------|
| `standalone` | All-in-one: Coordinator + Worker in a single process |
| `coordinator` | API server that schedules work (requires workers) |
| `worker` | Executes tasks by pulling from broker |

## Next Steps

- [Installation](installation.md) — Get Twerk running
- [Quick Start](quick-start.md) — Run your first job
- [CLI Reference](cli.md) — All available commands
