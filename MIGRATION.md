# Tork to Twerk Migration Guide

This document details the architectural and behavioral migration from the original Go implementation (**Tork**) to the modern Rust implementation (**Twerk**).

## 1. Project Identity & Branding
- **Project Name**: Tork -> **Twerk**
- **Crate Prefix**: `twerk-*`
- **Environment Variables**: `TORK_` -> `TWERK_` (e.g., `TWERK_DATASTORE_TYPE`)
- **Default Workdir**: `/tork/workdir` -> `/twerk/workdir`

## 2. Architectural Structure (Hard-Boundary DDD)
Twerk uses a Rust Workspace with strict compilation-level isolation between layers.

| Rust Crate | Layer | Responsibilities |
| :--- | :--- | :--- |
| `twerk-common` | Common | Logging, configuration loading, sync primitives, short UUIDs. |
| `twerk-core` | Domain | Entities (Job, Task, Node), Repository traits, Expression evaluation. |
| `twerk-infrastructure` | Infrastructure | Postgres (SQLx), RabbitMQ (lapin), Runtimes (Docker/Podman), Reexec. |
| `twerk-app` | Application | Engine, Coordinator state machine, Worker consumer loop, Scheduler. |
| `twerk-web` | Presentation | Axum handlers, API middleware, HTTP responses. |
| `twerk-cli` | Presentation | CLI entry point, argument parsing, binary orchestration. |

## 3. Behavioral Parity & Fixed Gaps

### Datastore (Postgres)
- **Cascading Deletes**: Fixed a critical gap where job deletion in Rust didn't clean up tasks or logs. The schema now uses `ON DELETE CASCADE`.
- **Priority Updates**: Task priority is now correctly persisted during `update_task`.
- **Permission Fallbacks**: Resolved foreign key violations by aligning user/role fallback logic with the Go implementation.
- **Timestamping**: Migrated from Go's `timestamp` to Rust's `timestamptz` for absolute UTC precision.

### Broker (RabbitMQ)
- **Redelivery Handling**: Implemented Go's "poison pill" protection. Messages with the `redelivered` flag are moved to an `x-redeliveries` queue for auditing.
- **Queue Naming**: System queues are now prefixed with `x-` (e.g., `x-pending`, `x-progress`) to match the Tork standard.
- **Exchange Alignment**: Switched from custom `twerk.events` to the standard `amq.topic` exchange for wildcard routing parity.

### Runtimes (Docker & Podman)
- **Digital Twin Tests**: Integration tests now use `testcontainers-rs` to spin up "Digital Twin" environments for every run.
- **Lifecycle Sequence**: Strictly follows `Pre -> Main -> Post` task execution with identical exponential backoff for network cleanup.
- **Log Streaming**: Log streaming now initiates *before* health probes to match Go's startup visibility.

### Engine (Coordinator & Worker)
- **State Machine**: Fully ported the `PENDING` -> `SCHEDULED` -> `RUNNING` -> `COMPLETED` transitions.
- **Complex Scheduling**: Implemented logic for `parallel` tasks, `each` (iteration), and `subjob` (nesting).
- **Consumer Loops**: The Worker now correctly subscribes to work queues and reports heartbeats (CPU/Host stats) to the coordinator.

## 4. Configuration Mapping
Dependencies are centralized in the root `Cargo.toml` using **Workspace Inheritance**.

```toml
# Use in sub-crates
[dependencies]
twerk-common = { workspace = true }
tokio = { workspace = true }
```

## 5. Completed Items
1. **Web API Parity**: Error formats aligned (`{"message": "..."}`) and `Wait` parameter implemented for job creation.
2. **Specialized Middleware**: `onReadJob` and `onReadTask` hooks implemented for secret masking.
3. **Advanced RabbitMQ**: Connection pooling implemented (3-connection RR) for high-throughput workloads.
4. **Log Streaming**: Fixed to initiate before health probes for startup visibility parity.
5. **Scheduled Jobs API**: All 6 handlers implemented (create, list, get, pause, resume, delete).

## 6. Remaining Gaps

### Critical
- **Input Validation**: Job/task input validation not fully ported from Go
- **Webhook Middleware**: Core webhook library exists but middleware triggering not wired up
- **Expression Evaluation**: Missing `fromJSON()`, `split()`, `toJSON()` functions
- **HTTP Auth Middleware**: Exists but NOT applied to HTTP routes
- **Worker Runtime Health**: Always reports `UP` without checking runtime health
- **In-Memory Broker**: 4 methods are no-ops (heartbeat/publish methods)
- **Coordinator Queue Subscriptions**: Missing 6 queue handlers

### Medium
- Docker Runtime (not implemented)
- Network Create/Remove (not implemented)
- Sidecars Support (may be incomplete)
- Registry Auth from Config File (not implemented)
- Shell stderr redirect (broken)
- `docker.image.ttl` config not being read
- CLI reexec init not integrated
- Shell runtime reexec UID/GID not integrated
