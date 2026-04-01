# Architecture

## Components

### Coordinator

Tracks jobs, dispatches work to workers, handles retries and failures. Stateless and leaderless; does not run tasks.

### Worker

Runs tasks via a runtime (Docker, Podman, or Shell).

### Broker

Routes messages between Coordinator and Workers:
- **RabbitMQ** — Production-grade message broker
- **In-Memory** — For testing and single-node deployments

### Datastore

Persists job and task state:
- **PostgreSQL** — Production database
- **In-Memory** — For testing

### Runtime

Execution environment for tasks:
- **Docker** — Default, best isolation
- **Podman** — Daemonless Docker alternative
- **Shell** — Runs on host

## Request Flow

```
Client → Coordinator → Broker → Worker → Runtime (Docker/Podman/Shell)
                ↓
            Datastore
```

1. Client submits job via REST API
2. Coordinator stores job in Datastore
3. Coordinator publishes tasks to Broker
4. Worker receives tasks from Broker
5. Worker executes tasks in containers
6. Worker reports results back via Broker
7. Coordinator updates job state in Datastore

## Modes

| Mode | Coordinator | Worker |
|------|-------------|--------|
| `standalone` | ✓ | ✓ |
| `coordinator` | ✓ | ✗ |
| `worker` | ✗ | ✓ |
