# Twerk

> A Rust task runner and distributed execution system. AI-built, human-governed.

Twerk is a port of [Tork](https://github.com/runabol/tork) — a Go task orchestrator — rewritten in Rust with improvements to the type system, state machine correctness, and runtime safety.

The code was written entirely by AI. Architecture, integration strategy, and quality enforcement were human-directed — hundreds of hours of adversarial testing across dozens of sessions went into shaking out real bugs from superficially correct code.

**This is not production infrastructure.** For serious orchestration needs, use [Restate](https://restate.dev/), [Temporal](https://temporal.io/), or [Inngest](https://www.inngest.com/). Twerk is best suited for smaller automations, local task pipelines, and as a reference for what an AI-augmented Rust development workflow looks like.

---

[![Rust](https://img.shields.io/badge/Rust-1.75+-pink.svg?style=for-the-badge)](https://www.rust-lang.org)
[![License: Apache--2.0](https://img.shields.io/badge/License-Apache--2.0-ff69b4.svg?style=for-the-badge)](LICENSE)

## Quick Start

```bash
git clone https://github.com/runabol/twerk.git
cd twerk
cargo build --release -p twerk-cli
./target/release/twerk server start standalone
```

The repo-root `config.toml` runs with zero dependencies — in-memory broker, in-memory datastore, shell runtime:

```bash
curl http://localhost:8000/health
```

Submit a job and wait for completion:

```bash
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

## Docs

- [Quick Start](website/src/quick-start.md) — Get running in 30 seconds
- [CLI Reference](website/src/cli.md) — `twerk server start`, `twerk health`, `twerk migration`
- [REST API](website/src/rest-api.md) — Full endpoint reference
- [Configuration](website/src/configuration.md) — TOML and environment variables
- [Jobs](website/src/jobs.md) — YAML job definitions and state reference
- [Architecture](website/src/architecture.md) — Coordinator, worker, broker, datastore
- [Examples](examples/) — 22 ready-to-run job definitions

## Origin

Twerk started as [Tork](https://github.com/runabol/tork), a Go task orchestrator. The Rust port preserved the core API surface (jobs, tasks, scheduled jobs, queues, triggers, webhooks) while adding:

- Exhaustive state machine transitions — illegal states are not representable
- Newtyped domain primitives with construction-time validation
- Direct event publishing to typed channels for reliable job completion signaling
- Argon2id-based encryption at rest for sensitive values

The entire codebase was written by Claude (Anthropic) across hundreds of sessions. A human architect directed the work: defining requirements, reviewing output, and running sustained adversarial QA (red-team testing, mutation testing, truth serum audits, contract parity checks) to catch the gap between "compiles and passes tests" and "actually correct under stress."

## Project Structure

```
twerk/
├── crates/twerk-core           # Domain types, validation, state machines
├── crates/twerk-common         # Shared config, logging, utilities
├── crates/twerk-infrastructure # Brokers, datastores, runtimes
├── crates/twerk-app            # Engine, coordinator, worker
├── crates/twerk-web            # HTTP API and OpenAPI
├── crates/twerk-cli            # CLI binary
├── website/                    # mdBook documentation source
├── examples/                   # Example job definitions
└── configs/                    # Sample configuration files
```

## Contributing

1. Build with `cargo build -p twerk-cli`
2. Run checks with `cargo test` and `cargo clippy`
3. Verify the standalone docs flow before landing user-facing changes

## License

Apache-2.0
