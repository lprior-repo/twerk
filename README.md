# Twerk

> **An agentic workflow engine for personal automations and workflows.** Run jobs across Docker, Podman, or shell — locally or distributed.

**⚠️ Transparency Statement: Twerk is NOT production-ready.** It's a personal automation tool that I've stress-tested extensively through adversarial reviews and hundreds of hours of testing. It's designed for individual developers automating their own workflows — not enterprise deployments.

---

## 📚 Documentation Links

| Topic | URL |
|-------|-----|
| Full Documentation | https://runabol.github.io/twerk/ |
| Quick Start | https://github.com/runabol/twerk/blob/main/website/src/quick-start.md |
| Architecture | https://github.com/runabol/twerk/blob/main/website/src/architecture.md |
| REST API | https://github.com/runabol/twerk/blob/main/website/src/rest-api.md |
| YAML Language Spec | https://github.com/runabol/twerk/blob/main/website/src/yaml-language-spec.md |
| Jobs Reference | https://github.com/runabol/twerk/blob/main/website/src/jobs.md |
| Tasks Reference | https://github.com/runabol/twerk/blob/main/website/src/tasks.md |
| Runtimes | https://github.com/runabol/twerk/blob/main/website/src/runtimes.md |
| Configuration | https://github.com/runabol/twerk/blob/main/website/src/configuration.md |
| CLI Reference | https://github.com/runabol/twerk/blob/main/website/src/cli.md |
| Examples | https://github.com/runabol/twerk/blob/main/website/src/examples.md |
| Comprehensive Guide | https://github.com/runabol/twerk/blob/main/website/src/COMPREHENSIVE_GUIDE.md |
| Inspiration (Tork) | https://github.com/runabol/tork |
| Releases | https://github.com/runabol/twerk/releases |

---

## ⚡ Quick Start

```bash
# Download binary
curl -L https://github.com/runabol/twerk/releases/latest/download/twerk-linux-x86_64.tar.gz | tar xz

# Run (zero deps - uses in-memory broker + shell)
./twerk run standalone

# Submit a job
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

---

## What is Twerk?

Twerk is a **distributed task execution system** for personal automations. Define jobs with multiple tasks running in isolated containers.

**Use cases:**
- 🔄 **Scheduled workflows** — Cron-based task execution with pause/resume
- 🔧 **Personal automations** — Scripts, backups, file processing
- 📦 **CI helper** — Run build/test steps without Kubernetes
- 🐳 **Containerized tasks** — Docker/Podman isolation without the overhead

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

## 🚀 Benchmarks

Measured on AMD Ryzen 9 7950X, 64GB RAM, Linux 6.x:

### End-to-End Throughput (twerk-bench)
```
8 workers, 10,000 tasks, crossbeam mpmc channels
Throughput: 1,087,937 tasks/sec
[PASS] Exceeds 5,000 tasks/sec target
```

### Engine Latency (criterion)
```
engine_new:           ~11 µs
engine_config:         ~9-12 µs (mode-dependent)
```

### Core Engine (test assertions)
```
ID Creation:         >500,000 IDs/sec (assertion floor)
1M IDs in:           <2 seconds
```

### Stress Test
```
10,000 parallel tasks - coordinator accepts and schedules without blocking
Full job completes successfully
```

---

## Features

| Feature | Description |
|---------|-------------|
| 🚀 **Zero-setup mode** | Single binary, no Postgres/RabbitMQ required |
| 🐳 **Multi-runtime** | Docker, Podman, or shell execution |
| 📈 **Parallel tasks** | Run tasks concurrently with `parallel` blocks |
| 🔄 **Each loops** | Iterate over lists with concurrency control |
| ⏱️ **Retry** | Configurable retry on failure with backoff |
| 📅 **Scheduled jobs** | Cron syntax with pause/resume |
| 🔐 **Secrets** | Auto-redacted environment variables |
| 📡 **HTTP API** | Full REST API for all operations |
| 🦀 **Rust** | Tokio async, zero panic in production |

---

## Architecture

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

## REST API (19 endpoints)

### Jobs
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/jobs` | Submit job |
| `POST` | `/jobs?wait=true` | Submit and block |
| `GET` | `/jobs` | List jobs |
| `GET` | `/jobs/{id}` | Get job |
| `GET` | `/jobs/{id}/log` | Job logs |
| `PUT` | `/jobs/{id}/cancel` | Cancel |
| `PUT` | `/jobs/{id}/restart` | Restart |
| `DELETE` | `/jobs/{id}` | Delete |

### Tasks
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/tasks/{id}` | Get task |
| `GET` | `/tasks/{id}/log` | Task logs |

### Scheduled Jobs
| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/scheduled-jobs` | Create |
| `GET` | `/scheduled-jobs` | List |
| `GET` | `/scheduled-jobs/{id}` | Get |
| `PUT` | `/scheduled-jobs/{id}/pause` | Pause |
| `PUT` | `/scheduled-jobs/{id}/resume` | Resume |
| `DELETE` | `/scheduled-jobs/{id}` | Delete |

### Queues
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/queues` | List queues |
| `GET` | `/queues/{name}` | Get queue |
| `DELETE` | `/queues/{name}` | Delete |

### System
| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/nodes` | List nodes |
| `GET` | `/metrics` | Metrics |
| `GET` | `/openapi.json` | OpenAPI spec |

---

## Configuration

```bash
# Environment variables
TWERK_BROKER_TYPE=rabbitmq
TWERK_DATASTORE_TYPE=postgres
TWERK_RUNTIME_TYPE=docker
```

Or TOML config at `./config.toml`, `~/twerk/config.toml`, or `/etc/twerk/config.toml`.

---

## Distributed Mode

```bash
# Coordinator
TWERK_DATASTORE_TYPE=postgres \
TWERK_BROKER_TYPE=rabbitmq \
./twerk run coordinator

# Worker(s)
TWERK_BROKER_TYPE=rabbitmq \
TWERK_RUNTIME_TYPE=docker \
./twerk run worker
```

---

## Job Examples

```yaml
# Parallel execution
name: parallel-work
tasks:
  - name: parent
    parallel:
      tasks:
        - name: task-a
          image: alpine:latest
          run: echo A
        - name: task-b
          image: alpine:latest
          run: echo B

# Loop with concurrency
name: process-items
tasks:
  - name: each-loop
    each:
      list: '[1, 2, 3, 4, 5]'
      concurrency: 2
      task:
        image: alpine:latest
        run: echo "Item {{ item.value }}"

# Retry on failure
name: with-retry
tasks:
  - name: unstable
    retry:
      limit: 3
    image: alpine:latest
    run: ./might-fail.sh
```

---

## Inspiration

Twerk is a Rust port of [Tork](https://github.com/runabol/tork) (Go). Tork is the production-grade version if you need something battle-tested for enterprise workloads.

---

## License

Apache-2.0
