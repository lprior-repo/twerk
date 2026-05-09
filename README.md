**⚠️ Transparency Statement: Twerk is NOT production-ready.** It's a personal automation tool that I've stress-tested extensively through adversarial reviews and hundreds of hours of testing. It's designed for individual developers automating their own workflows — not enterprise deployments.

# Twerk

> **An agentic workflow engine for personal automations and workflows.** Run jobs across Docker, Podman, or shell — locally or distributed.

🤖 **Agent-first codebase** — Twerk is built with an agent-first development model. AI agents handle implementation, testing, and validation, with human oversight for architecture and design decisions.

---

[Features](#features) • [Quick Start](#quick-start) • [Installation](#installation) • [Architecture](#architecture) • [Jobs](#jobs) • [Tasks](#tasks) • [Configuration](#configuration) • [REST API](#rest-api) • [Inspiration](#inspiration)

Twerk is a lightweight, distributed workflow engine for personal automations. Define jobs as YAML and run tasks in Docker containers, Podman, or plain shell — on a single machine or across multiple workers.

## Features

- **REST API** – Submit jobs, query status, cancel/restart
- **Zero-setup mode** – Single binary with in-memory broker and shell runtime; no Postgres or RabbitMQ required
- **Multi-runtime** – Docker, Podman, or shell execution
- **Parallel tasks** – Run tasks concurrently with `parallel` blocks
- **Each loops** – Iterate over lists with concurrency control
- **Retry** – Automatic retry with configurable limits; verified in standalone mode
- **Scheduled jobs** – Cron syntax with pause/resume
- **Secrets** – Auto-redacted environment variables
- **Stand-alone and distributed** – Run all-in-one or split into Coordinator + Workers
- **Task isolation** – Each task runs in its own container (Docker/Podman) or shell process
- **Rust + Tokio** – Async-first, type-safe, zero panic in production paths

---

## Quick Start

### Requirements

1. A recent [Rust toolchain](https://rustup.rs/) (or download a pre-built binary from [Releases](https://github.com/runabol/twerk/releases)).
2. (Optional) [Docker](https://www.docker.com/get-started) if you want containerized tasks.

### Hello World

Run Twerk in **standalone** mode (zero dependencies):

```bash
cargo run --bin twerk -- server-start standalone
```

Or with a pre-built binary:

```bash
./twerk server-start standalone
```

Create `hello.yaml`:

```yaml
---
name: hello job
tasks:
  - name: say hello
    run: |
      echo -n hello world
  - name: say goodbye
    run: |
      echo -n bye world
```

Submit the job:

```bash
JOB_ID=$(curl -s -X POST --data-binary @hello.yaml \
  -H "Content-type: text/yaml" http://localhost:8000/jobs | jq -r .id)
```

Check status:

```bash
curl -s http://localhost:8000/jobs/$JOB_ID
```

```json
{
  "id": "ed0dba93-d262-492b-8cf2-6e6c1c4f1c98",
  "state": "COMPLETED",
  ...
}
```

### Running in distributed mode

In distributed mode, the **Coordinator** schedules work and **Workers** execute tasks. A message broker (e.g. RabbitMQ) moves tasks between them.

Start PostgreSQL:

```bash
docker run -d \
  --name twerk-postgres \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=twerk \
  -e POSTGRES_USER=twerk \
  -e POSTGRES_DB=twerk \
  postgres:16-alpine
```

Start RabbitMQ:

```bash
docker run -d \
  -p 5672:5672 -p 15672:15672 \
  --name=twerk-rabbitmq \
  rabbitmq:3-management-alpine
```

Run the coordinator:

```bash
TWERK_DATASTORE_TYPE=postgres \
TWERK_BROKER_TYPE=rabbitmq \
./twerk server-start coordinator
```

Run one or more workers:

```bash
TWERK_BROKER_TYPE=rabbitmq \
TWERK_RUNTIME_TYPE=docker \
./twerk server-start worker
```

Submit the same job as before; the coordinator and workers will process it.

---

## Installation

### From source

```bash
git clone https://github.com/runabol/twerk.git
cd twerk
cargo build --release
```

The binary will be at `target/release/twerk`.

### Pre-built binaries

Download for your platform from the [Releases](https://github.com/runabol/twerk/releases) page.

---

## Architecture

A workflow is a **job**: a series of **tasks** (steps) run in order. Jobs are defined in YAML:

```yaml
---
name: hello job
tasks:
  - name: say hello
    run: echo -n hello world
  - name: say goodbye
    run: echo -n bye world
```

Components:

- **Coordinator** – Tracks jobs, dispatches work to workers, handles retries and failures. Stateless; does not run tasks.
- **Worker** – Runs tasks via a runtime (Docker, Podman, or Shell).
- **Broker** – Routes messages between Coordinator and Workers (in-memory or RabbitMQ).
- **Datastore** – Persists job and task state (in-memory or PostgreSQL).
- **Runtime** – Execution environment for tasks (Docker, Podman, Shell).

```
Client → Coordinator → Broker → Worker → Runtime (Docker/Podman/Shell)
                ↓
            Datastore
```

| Mode | Coordinator | Worker | Use Case |
|------|-------------|--------|----------|
| `standalone` | ✅ | ✅ | Personal automations |
| `coordinator` | ✅ | ❌ | Multi-machine setup |
| `worker` | ❌ | ✅ | Scale out workers |

---

## Jobs

A **job** is a list of tasks executed in order.

### Simple example

```yaml
name: hello job
tasks:
  - name: say hello
    run: |
      echo -n hello world
  - name: say goodbye
    run: |
      echo -n bye world
```

Submit:

```bash
curl -s -X POST --data-binary @job.yaml \
  -H "Content-type: text/yaml" \
  http://localhost:8000/jobs
```

### Inputs

```yaml
name: mov to mp4
inputs:
  source: https://example.com/path/to/video.mov
tasks:
  - name: convert the video to mp4
    image: jrottenberg/ffmpeg:3.4-alpine
    env:
      SOURCE_URL: '{{ inputs.source }}'
    run: |
      ffmpeg -i $SOURCE_URL /tmp/output.mp4
```

### Secrets

Use the `secrets` block for sensitive values (redacted in API responses):

```yaml
name: my job
secrets:
  api_key: 1111-1111-1111-1111
tasks:
  - name: my task
    image: alpine:latest
    run: curl -X POST -H "API_KEY: $API_KEY" http://example.com
    env:
      API_KEY: '{{ secrets.api_key }}'
```

### Defaults

Set defaults for all tasks:

```yaml
name: my job
defaults:
  retry:
    limit: 2
  limits:
    cpus: 1
    memory: 500m
  timeout: 10m
  queue: default
  priority: 3
tasks:
  - name: my task
    image: alpine:latest
    run: echo hello world
```

### Scheduled jobs

Use cron syntax:

```yaml
name: scheduled job test
schedule:
  cron: "0/5 * * * *"   # every 5 minutes
tasks:
  - name: my first task
    image: alpine:3.18.3
    run: echo -n hello world
```

Submit to the scheduler:

```bash
curl -s -X POST --data-binary @job.yaml \
  -H "Content-type: text/yaml" \
  http://localhost:8000/scheduled-jobs | jq .
```

---

## Tasks

A **task** is the unit of execution. With the Docker runtime, each task runs in a container. The `image` property sets the Docker image; `run` is the script to execute.

### Basic task

```yaml
- name: say hello
  image: alpine:latest
  run: |
    echo -n hello world
```

### Output and variables

Write to `$TWERK_OUTPUT` and set `var` to store the result in the job context for later tasks:

```yaml
tasks:
  - name: populate a variable
    var: task1
    image: alpine:latest
    run: echo -n "world" > "$TWERK_OUTPUT"
  - name: say hello
    image: alpine:latest
    env:
      NAME: '{{ tasks.task1 }}'
    run: echo -n hello $NAME
```

### Parallel Task

```yaml
- name: a parallel task
  parallel:
    tasks:
      - image: alpine:latest
        run: sleep 2
      - image: alpine:latest
        run: sleep 1
      - image: alpine:latest
        run: sleep 3
```

### Each Task

Run a task for each item in a list (with optional `concurrency`):

```yaml
- name: sample each task
  each:
    list: '[1, 2, 3, 4, 5]'
    concurrency: 3
    task:
      image: alpine:latest
      env:
        ITEM: '{{ item.value }}'
        INDEX: '{{ item.index }}'
      run: echo -n HELLO $ITEM at $INDEX
```

### Retry

```yaml
retry:
  limit: 5
  initialDelay: 5s
  scalingFactor: 2
```

---

## Configuration

Twerk can be configured with a `config.toml` file or environment variables. Config file locations (in order): current directory, `~/twerk/config.toml`, `/etc/twerk/config.toml`.

Environment variables: `TWERK_` + property path with dots replaced by underscores (e.g. `TWERK_LOGGING_LEVEL=warn`).

### Example config.toml

```toml
[broker]
type = "inmemory"   # inmemory | rabbitmq

[broker.rabbitmq]
url = "amqp://guest:guest@localhost:5672/"
consumer.timeout = "30m"

[datastore]
type = "inmemory"   # inmemory | postgres

[datastore.postgres]
dsn = "postgres://twerk:twerk@localhost:5432/twerk"

[coordinator]
address = "localhost:8000"

[worker]
queues.default = 1

[runtime]
type = "shell"   # shell | docker | podman

[runtime.shell]
cmd = ["bash", "-c"]
```

---

## REST API

Base URL: `http://localhost:8000` (or your coordinator address).

### Health check

```bash
GET /health
```

```json
{ "status": "UP" }
```

### List jobs

```bash
GET /jobs?page=1&size=10
```

Query params: `page`, `size` (1–20).

### Get job

```bash
GET /jobs/<JOB_ID>
```

### Submit a job

```bash
POST /jobs
Content-Type: text/yaml
```

Body: job YAML. Or `Content-Type: application/json` with JSON job definition.

### Cancel job

```bash
PUT /jobs/<JOB_ID>/cancel
```

### Restart job

```bash
PUT /jobs/<JOB_ID>/restart
```

### List queues

```bash
GET /queues
```

Returns broker queues with size and subscriber counts.

---

## Status & Testing

| Category | Status |
|----------|--------|
| Test Suite | ✅ 4,791 tests passing |
| Clippy | ✅ Zero warnings |
| Format | ✅ Clean |
| OpenAPI Contract | ✅ 19 endpoints verified |
| Docker Cleanup | ✅ No container leaks |
| Adversarial Review | ✅ 100s of hours of stress testing |

**This is NOT production software.** I make no guarantees about stability, security, or fitness for any purpose. Use at your own risk.

---

## Inspiration

Twerk is a Rust port of [Tork](https://github.com/runabol/tork) (Go) by Arik Cohen. Both share the same conceptual architecture — coordinator, worker, broker, datastore — and YAML job format, but target different use cases.

| | Twerk | Tork |
|---|---|---|
| **Language** | Rust (Tokio) | Go |
| **Maturity** | Personal automation / learning project | Production-grade, 800+ stars, 144 releases |
| **Web UI** | API only | Full web UI included |
| **Expression language** | Winnow parser (in progress) | expr library |
| **Middleware** | Basic | Extensive (auth, CORS, rate limit, webhooks) |
| **Pre/Post tasks** | No | Yes |
| **Subjobs** | No | Yes |
| **Task priority** | No | 0–9 |
| **Full-text search** | No | Yes |
| **Best for** | Personal scripts, learning Rust async | Enterprise workflows, CI/CD, data pipelines |

If you need something battle-tested for production workloads, use **Tork**. If you want a Rust async learning project or personal automation tool, **Twerk** is the spiritual sibling.

---

## License

Apache-2.0
