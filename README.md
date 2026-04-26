# Twerk

[![Rust](https://img.shields.io/badge/Rust-1.75+-pink.svg?style=for-the-badge)](https://www.rust-lang.org)
[![License: Apache--2.0](https://img.shields.io/badge/License-Apache--2.0-ff69b4.svg?style=for-the-badge)](LICENSE)

**Docs** — [Quick Start](website/src/quick-start.md) · [REST API](website/src/rest-api.md) · [CLI Reference](website/src/cli.md) · [Configuration](website/src/configuration.md) · [Jobs](website/src/jobs.md) · [Architecture](website/src/architecture.md) · [Examples](examples/)

---

A Rust task runner and distributed execution system. Ported from [Tork](https://github.com/runabol/tork), written by AI.

**Not production infrastructure.** For serious orchestration, use [Restate](https://restate.dev/), [Temporal](https://temporal.io/), or [Inngest](https://www.inngest.com/). Great for smaller automations and local pipelines.

## Quick Start

```bash
git clone https://github.com/runabol/twerk.git
cd twerk
cargo build --release -p twerk-cli
./target/release/twerk server start standalone
```

Zero dependencies — in-memory broker, in-memory datastore, shell runtime:

```bash
curl http://localhost:8000/health
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @examples/hello-shell.yaml
```

## Features

- **Zero-Dependency Local Mode** — In-memory broker/datastore with shell runtime
- **Multi-Runtime Support** — Shell, Docker, and Podman executors
- **Distributed Execution** — Separate coordinator and worker processes backed by Postgres and RabbitMQ
- **HTTP API** — Job submission, status, logs, health, scheduled jobs, queues, and triggers
- **Async Rust Core** — Tokio-based with typed domain models and exhaustive state transitions

## Origin

Ported from [Tork](https://github.com/runabol/tork) (Go) to Rust with:

- Exhaustive state machine transitions — illegal states are not representable
- Newtyped domain primitives with construction-time validation
- Argon2id-based encryption at rest for sensitive values

Written by AI. Architecture, testing, and quality enforcement were human-directed.

## Project Structure

```
crates/twerk-core           # Domain types, validation, state machines
crates/twerk-common         # Shared config, logging, utilities
crates/twerk-infrastructure # Brokers, datastores, runtimes
crates/twerk-app            # Engine, coordinator, worker
crates/twerk-web            # HTTP API and OpenAPI
crates/twerk-cli            # CLI binary
website/                    # mdBook documentation source
examples/                   # 22 example job definitions
configs/                    # Sample configuration files
```

## License

Apache-2.0
