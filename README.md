# Twerk

> A distributed task runner built in Rust. Run jobs across Docker, Podman, or shell — locally or distributed.

[![Rust](https://img.shields.io/badge/Rust-1.75+-pink.svg?style=for-the-badge)](https://www.rust-lang.org)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-ff69b4.svg?style=for-the-badge)](LICENSE)
[![Test Suite](https://img.shields.io/badge/Tests-4791%20passed-brightgreen?style=for-the-badge)]()

**Documentation:** [📖 Full Docs](https://runabol.github.io/twerk/) | [Quick Start](#quick-start) | [Architecture](#architecture) | [REST API](#rest-api)

---

## What is Twerk?

Twerk is a **distributed task execution system** that lets you define jobs with multiple tasks, each running in isolated containers. Think "background job processing for infrastructure teams" — no Kubernetes required.

**Perfect for:**
- 🔄 **Scheduled jobs** — Cron-based task execution with pause/resume
- 📦 **CI/CD pipelines** — Run build, test, and deploy steps
- 🔧 **DevOps automation** — Database backups, log rotation, health checks
- 📊 **Data processing** — ETL jobs, batch processing, parallel workflows
- 🐳 **Containerized tasks** — Docker/Podman isolation without the orchestration overhead

## Features

| Feature | Description |
|---------|-------------|
| 🚀 **Zero-setup mode** | Single binary, no Postgres/RabbitMQ required |
| 🐳 **Multi-runtime** | Docker, Podman, or shell execution |
| 📈 **Horizontally scalable** | Add workers to increase throughput |
| 🔒 **Task isolation** | Containers with resource limits |
| ⏱️ **Retry with backoff** | Configurable retry on failure |
| 📅 **Scheduled jobs** | Cron syntax with pause/resume |
| 🔐 **Secrets management** | Auto-redaction of sensitive values |
| 📡 **HTTP API** | Full API for job, task, queue, and node management |
| 🦀 **Built in Rust** | Tokio async, zero panic in production |

## Quick Start

### 1. Download or Build

**Download binary:**
```bash
curl -L https://github.com/runabol/twerk/releases/latest/download/twerk-linux-x86_64.tar.gz | tar xz
```

**Or build from source:**
```bash
git clone https://github.com/runabol/twerk.git
cd twerk
cargo build --release -p twerk-cli
```

### 2. Run

```bash
./twerk run standalone
```

Starts on `http://localhost:8000` with zero dependencies (in-memory broker + shell runtime).

### 3. Submit a Job

```bash
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @- <<'EOF'
name: hello-world
tasks:
  - name: say hello
    image: alpine:latest
    run: echo "Hello from Twerk!"
EOF
```

### 4. Check Status

```bash
# Health check
curl http://localhost:8000/health

# List jobs
curl http://localhost:8000/jobs

# View logs
curl http://localhost:8000/jobs/<job-id>/log
```

---

## Architecture

```
Client → Coordinator → Broker → Worker → Runtime (Docker/Podman/Shell)
                ↓
            Datastore (PostgreSQL or In-Memory)
```

### Components

| Component | Role |
|-----------|------|
| **Coordinator** | Receives jobs, schedules tasks, manages state |
| **Worker** | Executes tasks via configured runtime |
| **Broker** | Routes tasks (RabbitMQ or In-Memory) |
| **Datastore** | Persists state (PostgreSQL or In-Memory) |

### Modes

| Mode | Coordinator | Worker | Use Case |
|------|-------------|--------|----------|
| `standalone` | ✅ | ✅ | Development, small workloads |
| `coordinator` | ✅ | ❌ | Production API server |
| `worker` | ❌ | ✅ | Scale out task execution |

---

## Job Definition

Jobs are defined in YAML:

```yaml
name: my-job
description: A real workflow
tags: [production, backup]

inputs:
  database: mydb
  retention: 7

tasks:
  - name: backup
    image: postgres:15
    run: pg_dump -a $DATABASE > /backups/dump.sql
    env:
      DATABASE: '{{ inputs.database }}'

  - name: cleanup
    image: alpine:latest
    run: find /backups -mtime +{{ inputs.retention }} -delete
```

### Advanced Task Features

```yaml
tasks:
  # Retry on failure
  - name: unstable-task
    retry:
      limit: 3
    image: alpine:latest
    run: ./might-fail.sh

  # Run in parallel
  - name: parallel-parent
    parallel:
      tasks:
        - name: task-a
          image: alpine:latest
          run: echo A
        - name: task-b
          image: alpine:latest
          run: echo B

  # Loop over items
  - name: process-items
    each:
      list: '[1, 2, 3, 4, 5]'
      concurrency: 2
      task:
        image: alpine:latest
        run: echo "Processing item {{ item.value }}"

  # Conditional execution
  - name: deploy
    if: "{{ job.state == 'SCHEDULED' }}"
    image: alpine:latest
    run: ./deploy.sh

  # Resource limits
  - name: limited-task
    limits:
      cpus: "0.5"
      memory: "256m"
    image: alpine:latest
    run: echo hello
```

---

## REST API

Base URL: `http://localhost:8000`

### Jobs

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/jobs` | Submit a job |
| `POST` | `/jobs?wait=true` | Submit and block until completion |
| `GET` | `/jobs` | List jobs (paginated) |
| `GET` | `/jobs/{id}` | Get job details |
| `GET` | `/jobs/{id}/log` | Fetch job logs |
| `PUT` | `/jobs/{id}/cancel` | Cancel a job |
| `PUT` | `/jobs/{id}/restart` | Restart a job |
| `DELETE` | `/jobs/{id}` | Delete a job |

### Tasks

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/tasks/{id}` | Get task details |
| `GET` | `/tasks/{id}/log` | Fetch task logs |

### Scheduled Jobs

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/scheduled-jobs` | Create scheduled job |
| `GET` | `/scheduled-jobs` | List scheduled jobs |
| `GET` | `/scheduled-jobs/{id}` | Get scheduled job |
| `PUT` | `/scheduled-jobs/{id}/pause` | Pause schedule |
| `PUT` | `/scheduled-jobs/{id}/resume` | Resume schedule |
| `DELETE` | `/scheduled-jobs/{id}` | Delete scheduled job |

### Queues

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/queues` | List queues |
| `GET` | `/queues/{name}` | Get queue details |
| `DELETE` | `/queues/{name}` | Delete a queue |

### System

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/nodes` | List nodes |
| `GET` | `/metrics` | Fetch metrics |
| `GET` | `/openapi.json` | OpenAPI spec |

Full API documentation: [📡 REST API Docs](website/src/rest-api.md)

---

## Configuration

### Environment Variables

```bash
TWERK_BROKER_TYPE=rabbitmq          # inmemory, rabbitmq
TWERK_DATASTORE_TYPE=postgres       # inmemory, postgres
TWERK_RUNTIME_TYPE=docker            # docker, podman, shell
TWERK_LOGGING_LEVEL=debug
```

### TOML Config

```toml
[broker]
type = "rabbitmq"

[broker.rabbitmq]
url = "amqp://guest:guest@localhost:5672/"

[datastore]
type = "postgres"

[datastore.postgres]
dsn = "host=localhost user=twerk password=twerk dbname=twerk port=5432 sslmode=disable"

[runtime]
type = "docker"
```

Config file search order: `./config.local.toml` → `./config.toml` → `~/twerk/config.toml` → `/etc/twerk/config.toml`

Full configuration reference: [⚙️ Configuration Docs](website/src/configuration.md)

---

## Distributed Mode

```bash
# Terminal 1: Start coordinator
TWERK_DATASTORE_TYPE=postgres \
TWERK_DATASTORE_POSTGRES_DSN="host=localhost user=twerk password=twerk dbname=twerk port=5432" \
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
./twerk run coordinator

# Terminal 2: Start worker(s)
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
TWERK_RUNTIME_TYPE=docker \
./twerk run worker

# Terminal 3: Scale workers
TWERK_BROKER_TYPE=rabbitmq \
TWERK_BROKER_RABBITMQ_URL="amqp://guest:guest@localhost:5672/" \
TWERK_RUNTIME_TYPE=docker \
./twerk run worker
```

---

## Documentation

| Topic | Link |
|-------|------|
| Full Documentation | [📖 docs/](website/src/) |
| Installation | [installation.md](website/src/installation.md) |
| Quick Start | [quick-start.md](website/src/quick-start.md) |
| Architecture | [architecture.md](website/src/architecture.md) |
| CLI Reference | [cli.md](website/src/cli.md) |
| Job Reference | [jobs.md](website/src/jobs.md) |
| Task Reference | [tasks.md](website/src/tasks.md) |
| Runtimes | [runtimes.md](website/src/runtimes.md) |
| Configuration | [configuration.md](website/src/configuration.md) |
| REST API | [rest-api.md](website/src/rest-api.md) |
| Examples | [examples.md](website/src/examples.md) |
| YAML Spec | [yaml-language-spec.md](website/src/yaml-language-spec.md) |

---

## Project Structure

```
twerk/
├── crates/
│   ├── twerk-common/       # Shared config, logging, utilities
│   ├── twerk-core/         # Domain types, validation, expressions
│   ├── twerk-infrastructure/  # Brokers, datastores, runtimes
│   ├── twerk-app/          # Engine, coordinator, worker
│   ├── twerk-web/          # HTTP API, OpenAPI spec
│   └── twerk-cli/         # CLI binary
├── website/src/            # Documentation (mdBook)
├── examples/               # Example job definitions
├── configs/                # Sample configurations
└── .github/workflows/     # CI/CD
```

---

## Contributing

1. Build: `cargo build --release -p twerk-cli`
2. Run tests: `cargo test --workspace`
3. Lint: `cargo clippy --all-targets --all-features -- -D warnings`
4. Format: `cargo fmt --check`

See [AGENTS.md](AGENTS.md) for development workflow.

---

## License

Apache-2.0
