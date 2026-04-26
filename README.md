# Twerk

> A Rust task runner and distributed execution system.

---

[![Rust](https://img.shields.io/badge/Rust-1.75+-pink.svg?style=for-the-badge)](https://www.rust-lang.org)
[![License: Apache--2.0](https://img.shields.io/badge/License-Apache--2.0-ff69b4.svg?style=for-the-badge)](LICENSE)

## Overview

Twerk runs jobs across shell, Docker, and Podman runtimes. It can run as a single local process for development or as a distributed coordinator and worker system backed by Postgres and RabbitMQ.

## Features

- **Zero-Dependency Local Mode** — In-memory broker/datastore with shell runtime for fast onboarding
- **Multi-Runtime Support** — Shell, Docker, and Podman executors
- **Distributed Execution** — Separate coordinator and worker processes when you need them
- **HTTP API** — Job submission, status, logs, health, scheduled jobs, queues, and triggers
- **Async Rust Core** — Tokio-based services with typed domain models

## Quick Start

```bash
git clone https://github.com/runabol/twerk.git
cd twerk
cargo build --release -p twerk-cli
./target/release/twerk server start standalone
```

The repo-root `config.toml` is set up for the primary local journey:

- `broker.type = "inmemory"`
- `datastore.type = "inmemory"`
- `runtime.type = "shell"`

Health check:

```bash
curl http://localhost:8000/health
```

Submit the example job and wait for completion:

```bash
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @examples/hello-shell.yaml
```

## Docs

- [Quick Start](website/src/quick-start.md) — Primary standalone journey
- [CLI Reference](website/src/cli.md) — All available commands
- [REST API](website/src/rest-api.md) — Current HTTP endpoints
- [Configuration](website/src/configuration.md) — Config file and environment variable reference
- [Jobs](website/src/jobs.md) — Job definitions and YAML reference
- `examples/hello-shell.yaml` — Zero-dependency sample job

## Project Structure

```
twerk/
├── crates/twerk-common         # Shared config, logging, utilities
├── crates/twerk-core           # Domain types and validation
├── crates/twerk-infrastructure # Brokers, datastores, runtimes
├── crates/twerk-app            # Engine, coordinator, worker
├── crates/twerk-web            # HTTP API and OpenAPI
├── crates/twerk-cli            # CLI crate that ships the `twerk` binary
├── website/                    # mdBook source
├── examples/                   # Example job definitions
└── configs/                    # Sample configuration files
```

## Contributing

1. Build with `cargo build -p twerk-cli`
2. Run checks with `cargo test` and `cargo clippy`
3. Verify the standalone docs flow before landing user-facing changes
4. For non-trivial work, follow the staged workflow in `AGENTS.md` and `.claude/CRISPY.md`

## License

Apache-2.0
