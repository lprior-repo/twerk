# Twerk

> The Rust rewrite of [Twerk](https://github.com/runabol/twerk) — a distributed task execution system

---

[![Rust](https://img.shields.io/badge/Rust-1.75+-pink.svg?style=for-the-badge)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-ff69b4.svg?style=for-the-badge)](LICENSE)

## Overview

Twerk is a port of the Twerk distributed task execution system from Go to Rust. The goal is to bring Twerk's functionality to Rust with proper type safety, async-first design, and zero-cost abstractions.

Twerk handles workflow execution across Docker, Podman, and Shell environments. Twerk aims to provide the same capabilities with Rust's compile-time safety guarantees.

## Features

- **Multi-Runtime Support** — Docker, Podman, Shell executors
- **Distributed Execution** — Scale tasks across multiple nodes
- **Async Everything** — Built on Tokio for high throughput
- **Type Safety** — Rust's compiler catches bugs before runtime
- **RabbitMQ Broker** — Production-grade message broker with connection pooling
- **Postgres Datastore** — Persistent storage with connection pooling
- **Graceful Shutdown** — Proper signal handling and cleanup

## Quick Start

```bash
git clone https://github.com/lprior-repo/twerk.git
cd twerk
cargo build --release
cargo run --bin twerk -- help
```

## Project Structure

```
twerk/
├── twerk/              # Core domain types
├── locker/            # Distributed locking
├── engine/            # Orchestration engine
├── broker/            # Message broker (RabbitMQ + in-memory)
├── datastore/         # Data storage (Postgres)
├── cli/               # Command line interface
├── health/            # Health checks
├── input/            # Input validation
├── coordinator/       # Job coordinator + API server
└── runtime/          # Runtime implementations (Docker, Podman, Shell)
```

## Design Principles

- **Type Safety** — Compiler-enforced correctness
- **Zero Panics** — No `.unwrap()` or `.expect()` in production
- **Parse at Boundaries** — Validate at input, trust internals
- **Async First** — All I/O is async via Tokio
- **Functional Core** — Data → Calc → Actions

## Port Status

The port covers all 173 Go source files across:

- Domain types (Task, Job, Node, User, Role, Mount)
- Broker implementations (RabbitMQ, In-Memory)
- Runtime executors (Docker, Podman, Shell)
- Engine orchestration and coordination
- Datastore and locker implementations
- Input validation and redaction
- Health checks and middleware
- CLI and configuration

## Contributing

1. Fork the repo
2. Create a feature branch
3. Make your changes
4. Run tests with `cargo test --workspace`
5. Submit a PR

## License

MIT
