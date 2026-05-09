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
./twerk server-start standalone

# Submit a job
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @- <<'EOF'
name: hello-world
tasks:
  - name: say hello
    run: echo "Hello from Twerk!"
EOF
```

---

## What is Twerk?

Twerk is a **distributed task execution system** for personal automations. Define jobs with multiple tasks running as local shell commands or isolated containers.

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

## Reality-Checked Performance

There is **no published full-workflow throughput benchmark yet**. The previous million-tasks/sec number came from `twerk-bench`, a synthetic Crossbeam channel microbenchmark. It proves the channel pipeline can move fake tasks quickly; it does **not** measure the Twerk HTTP API, scheduler, datastore, broker, worker runtime, Docker, logs, retries, or YAML execution.

### Real Local Proofs

These are single-run smoke proofs from `target/release/twerk server-start standalone` on the local machine, not capacity benchmarks:

| Use case | API path | Result |
|----------|----------|--------|
| One-step shell job | `POST /jobs?wait=true` with `examples/hello-shell.yaml` | `COMPLETED` in 7 ms; log contained `hello from twerk` |
| Two-step shell workflow | `POST /jobs?wait=true` with inline YAML | `COMPLETED` in 9 ms; logs contained both steps |
| Parallel shell fan-out | `POST /jobs` plus polling | `COMPLETED`; logs contained all fan-out tasks |
| Docker Alpine task | `TWERK_RUNTIME_TYPE=docker` plus `POST /jobs?wait=true` | `COMPLETED` in 2.5 s with task `exitCode: 0` |

---

## Features

| Feature | Description |
|---------|-------------|
| 🚀 **Zero-setup mode** | Single binary, no Postgres/RabbitMQ required |
| 🐳 **Multi-runtime** | Docker, Podman, or shell execution |
| 📈 **Parallel tasks** | Run tasks concurrently with `parallel` blocks |
| 🔄 **Each loops** | Iterate over lists with concurrency control |
| ⏱️ **Retry** | Syntax exists, but standalone runtime retry needs more QA before relying on it |
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

## REST API

See [`docs/openapi.yaml`](docs/openapi.yaml) or `GET /openapi.json` for the full contract. Common endpoints:

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
./twerk server-start coordinator

# Worker(s)
TWERK_BROKER_TYPE=rabbitmq \
TWERK_RUNTIME_TYPE=docker \
./twerk server-start worker
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

# Docker task, when started with TWERK_RUNTIME_TYPE=docker
name: docker-work
tasks:
  - name: alpine-task
    image: alpine:latest
    run: echo "hello from a container"
```

---

## Inspiration

Twerk is a Rust port of [Tork](https://github.com/runabol/tork) (Go). Tork is the production-grade version if you need something battle-tested for enterprise workloads.

---

## License

Apache-2.0
