# Twerk

> **An agentic workflow engine for personal automations and workflows.** Run jobs across Docker, Podman, or shell — locally or distributed.

**⚠️ NOT production-ready.** Personal automation tool. Use at your own risk.

---

## 📚 Documentation

| Docs | API | CLI | Config | Examples |
|------|-----|-----|--------|----------|
| [Full Docs](website/src/) | [REST API](website/src/rest-api.md) | [CLI](website/src/cli.md) | [Config](website/src/configuration.md) | [Examples](website/src/examples.md) |
| [Quick Start](website/src/quick-start.md) | [YAML Spec](website/src/yaml-language-spec.md) | [Jobs](website/src/jobs.md) | [Tasks](website/src/tasks.md) | [Runtimes](website/src/runtimes.md) |
| [Architecture](website/src/architecture.md) | [Comprehensive Guide](website/src/COMPREHENSIVE_GUIDE.md) | | | |

---

## ⚡ Quick Start

```bash
# Download binary
curl -L https://github.com/runabol/twerk/releases/latest/download/twerk-linux-x86_64.tar.gz | tar xz

# Run (zero deps - in-memory + shell)
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

## ✅ Status

| Tests | Clippy | Format | OpenAPI | Docker |
|-------|--------|--------|---------|--------|
| 4,791 passed | ✅ Zero warnings | ✅ Clean | ✅ 19 endpoints | ✅ No leaks |

100s of hours adversarial testing. **This is NOT production software.**

---

## 🚀 Benchmarks

```
ID Creation:  1,000,000+ IDs/second  (~100ns per ID)
Stress Test:  10,000 parallel tasks - coordinator accepts without blocking
Workflow:     Pokemon API benchmark - 18 tasks, triple parallelism, all completed
```

---

## What is Twerk?

Distributed task execution for personal automations:
- 🔄 Scheduled workflows with cron
- 🔧 Scripts, backups, file processing
- 🐳 Docker/Podman isolation
- 📈 Parallel + loop tasks
- ⏱️ Retry with backoff
- 📡 HTTP API (19 endpoints)
- 🦀 Rust + Tokio

---

## Architecture

```
Client → Coordinator → Broker → Worker → Runtime (Docker/Podman/Shell)
                ↓
            Datastore
```

| Mode | Coordinator | Worker | Best For |
|------|-------------|--------|----------|
| `standalone` | ✅ | ✅ | Personal automations |
| `coordinator` | ✅ | ❌ | Multi-machine |
| `worker` | ❌ | ✅ | Scale out |

---

## Configuration

```bash
TWERK_BROKER_TYPE=rabbitmq     # or inmemory
TWERK_DATASTORE_TYPE=postgres  # or inmemory
TWERK_RUNTIME_TYPE=docker      # or podman, shell
```

TOML: `./config.toml`, `~/twerk/config.toml`, `/etc/twerk/config.toml`

---

## Distributed Mode

```bash
# Coordinator
TWERK_BROKER_TYPE=rabbitmq TWERK_DATASTORE_TYPE=postgres ./twerk run coordinator

# Worker
TWERK_BROKER_TYPE=rabbitmq TWERK_RUNTIME_TYPE=docker ./twerk run worker
```

---

## Job Example

```yaml
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
```

More: [Examples](website/src/examples.md)

---

## Inspiration

Rust port of [Tork](https://github.com/runabol/tork) (Go). Tork is production-grade if you need enterprise-ready.

---

## License

Apache-2.0
