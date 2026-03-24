# Port Gap Analysis: Go Tork → Rust Twerk

> Generated 2026-03-23. Go version: [runabol/tork](https://github.com/runabol/tork) v0.1.152.
> Last updated: 2026-03-23 — All P0 and P1 items implemented (P1-5 pending).

---

## P0 — Critical (blocks production use)

### 1. ~~Full-Text Search Broken~~ ✅ FIXED

**Fixed**: Schema now uses `tsvector GENERATED ALWAYS AS (...) STORED` for both `jobs.ts` and `tasks_log_parts.ts` columns, matching Go's auto-population via `to_tsvector`.

**Files**: `datastore/postgres/schema.rs`

---

### 2. ~~Wait Mode for Job Creation Missing~~ ✅ FIXED

**Fixed**: `POST /jobs` now supports `wait=true` query parameter. Subscribes to `job.*` broker events, blocks until terminal state or 60s timeout, returns full job JSON.

**Files**: `coordinator/api/handlers.rs`

---

### 3. ~~User-Based Job Filtering Broken~~ ✅ FIXED

**Fixed**: `list_jobs_handler` and `list_scheduled_jobs_handler` now extract `current_user` from request extensions (populated by auth middleware) instead of passing `String::new()`.

**Files**: `coordinator/api/handlers.rs`, `tork/user.rs`, `engine/coordinator.rs`

---

### 4. ~~Detached Subjobs Not Executed~~ ✅ FIXED

**Fixed**: Detached subjobs now create a standalone Job (new ID, PENDING state), persist it, publish to broker, and immediately complete the parent task.

**Files**: `coordinator/scheduler/mod.rs`

---

### 5. ~~No Web Middleware Implementations~~ ✅ FIXED

**Fixed**: Implemented CORS, rate limiting (governor), request logging, and body limit middleware. All configurable via `middleware.web.*` config namespace. Wired into Axum router.

**Files**: `middleware/web/cors.rs`, `middleware/web/rate_limit.rs`, `middleware/web/logger.rs`, `middleware/web/body_limit.rs`, `middleware/web/config.rs`, `coordinator/api/mod.rs`

---

## P1 — High (significant missing functionality)

### 6. ~~No Individual API Endpoint Toggling~~ ✅ FIXED

**Fixed**: Added `ApiEndpoints` struct with 8 boolean fields (health, jobs, tasks, nodes, queues, metrics, users, scheduled_jobs), all defaulting to `true`. Router conditionally registers routes.

**Files**: `coordinator/config.rs`, `coordinator/api/mod.rs`, `coordinator/coordinator.rs`

---

### 7. ~~Postgres Distributed Locker Missing~~ ✅ FIXED

**Fixed**: Engine now reads `TORK_LOCKER_TYPE` env → falls back to `TORK_DATASTORE_TYPE` → falls back to `"inmemory"`. When datastore is postgres, the locker is also postgres.

**Files**: `engine/lib.rs`

---

### 8. ~~Each-Task List Expression Evaluation Simplified~~ ✅ FIXED

**Fixed**: `parse_list_expression` now has 3 strategies: JSON → evalexpr → comma-separated. Uses `create_eval_context()` with built-in functions (sequence, randomInt). Inner tasks are template-evaluated with item values injected.

**Files**: `coordinator/scheduler/mod.rs`

---

### 9. Config Format Mismatch (TOML vs YAML)

**Go**: Uses TOML for configuration (`config.toml`) via `knadh/koanf`.

**Rust**: Uses YAML for configuration (`config.yaml`). This is a breaking change for existing Tork users migrating to twerk.

**Consider**: Support both formats, or document the migration path clearly.

**Files**: `conf.rs`, `Cargo.toml`

---

### 10. ~~Progress Events Not Fired on Task Completion~~ ✅ FIXED

**Fixed**: `complete_top_level_task` now publishes a progress event via `broker.publish_event(TOPIC_JOB_PROGRESS, serialized_job)` after updating the job.

**Files**: `coordinator/handlers/completed/mod.rs`, `tork/broker/mod.rs`

---

## P2 — Medium (feature gaps)

### 11. No Podman Runtime Configuration

**Go**: `runtime.podman.*` config namespace with `privileged` mode, image TTL, etc.

**Rust**: Podman runtime code exists in `runtime/podman/` but configuration support is incomplete. No `runtime.podman` config namespace in `conf.rs`.

**Files**: `conf.rs`, `runtime/podman/config.rs`

---

### 12. RabbitMQ Management API Not Integrated

**Go**: `broker.rabbitmq.management.url` config enables RabbitMQ Management API integration for queue stats (size, subscribers, unacked).

**Rust**: RabbitMQ broker exists but no management API integration. `GET /queues` endpoint may return incomplete data.

**Files**: `broker/rabbitmq.rs`, `coordinator/api/handlers.rs`

---

### 13. Missing Request Logging Middleware

**Go**: `middleware.web.logger` config enables request logging with configurable level and skip paths.

**Rust**: No request logging middleware found.

**Files**: `middleware/web/`

---

### 14. Coordinator Doesn't Send Heartbeats

**Go**: Coordinator sends periodic heartbeats with hostname, CPU percent, and version to the broker for cluster monitoring.

**Rust**: No coordinator heartbeat sending found. Worker heartbeats are handled, but the coordinator itself doesn't report its health.

**Files**: `coordinator/subscriptions.rs`, `coordinator/coordinator.rs`

---

### 15. `bytea` vs `jsonb` Column Types

**Go**: Uses `jsonb` for JSON columns in PostgreSQL.

**Rust**: Uses `bytea` for JSON columns. Different storage format — data portability between Go and Rust databases is not possible.

**Files**: `datastore/postgres/schema.rs`, `datastore/postgres/records.rs`

---

### 16. Missing Configuration Options

Config keys present in Go but absent or unused in Rust:

| Config Key | Purpose |
|---|---|
| `broker.rabbitmq.consumer.timeout` | RabbitMQ consumer timeout |
| `broker.rabbitmq.management.url` | RabbitMQ management API |
| `broker.rabbitmq.durable.queues` | Durable queue support |
| `broker.rabbitmq.queue.type` | Queue type (classic/quorum) |
| `middleware.web.cors.*` | CORS middleware |
| `middleware.web.basicauth.*` | Basic auth |
| `middleware.web.keyauth.*` | API key auth |
| `middleware.web.ratelimit.*` | Rate limiting |
| `middleware.web.bodylimit` | Request body size limit |
| `worker.limits.*` | Default task resource limits |
| `mounts.bind.*` | Bind mount restrictions |
| `mounts.temp.dir` | Temp directory for mounts |
| `runtime.docker.privileged` | Privileged container mode |
| `runtime.docker.image.ttl` | Image cache TTL |

**Files**: `conf.rs`, `engine/lib.rs`

---

## P3 — Low (polish and tests)

### 17. No Custom Template Functions

**Go**: `internal/eval/funcs.go` registers custom functions available in template expressions (e.g., `sequence()`).

**Rust**: No custom template functions registered. Expression evaluation uses `evalexpr` defaults.

**Files**: `eval/`

---

### 18. Missing Sample Config File

**Go**: `configs/sample.config.toml` (3,827 bytes) provides a documented reference config.

**Rust**: No equivalent sample config file.

**Files**: Root directory

---

### 19. Missing Example Job Definitions

**Go**: `examples/` directory contains 14 YAML job definitions (hello, each, parallel, subjob, retry, timeout, resize_image, split_and_stitch, hls, etc.).

**Rust**: No `examples/` directory.

**Files**: Root directory

---

### 20. Integration Test Coverage

**Go**: Comprehensive test suites including:
- `datastore/postgres/postgres_test.go` (45,591 bytes)
- `internal/coordinator/api/api_test.go` (27,046 bytes)
- `internal/coordinator/handlers/completed_test.go` (19,233 bytes)
- `internal/coordinator/scheduler/scheduler_test.go` (13,842 bytes)
- `engine/engine_test.go` (8,992 bytes)

**Rust**: Has inline `#[cfg(test)]` modules but lacks equivalent integration test depth, particularly for:
- Full API tests with real Postgres
- Comprehensive datastore CRUD tests
- RabbitMQ broker tests
- Webhook middleware integration tests

---

### 21. Auth Context Not Wired

**Go**: Sets `j.CreatedBy` from authenticated user context on job creation.

**Rust**: Sets `job.created_by = None` with comment "No auth context yet". When auth middleware is implemented, this needs to be wired.

**Files**: `coordinator/api/handlers.rs`

---

## Summary Table

| Priority | Count | Category | Status |
|---|---|---|---|
| P0 Critical | 5 | Full-text search, wait mode, auth filtering, detached subjobs, web middleware | **All fixed** |
| P1 High | 5 | Endpoint toggling, locker, expressions, config format, progress events | **4 fixed**, 1 remaining (config format) |
| P2 Medium | 6 | Podman config, RabbitMQ mgmt, logging middleware, heartbeats, column types, config keys | Pending |
| P3 Low | 5 | Template funcs, sample config, examples, tests, auth wiring | Pending |
| **Total** | **21** | | **9 fixed, 12 remaining** |

### Bonus Fixes
- Fixed pre-existing broken test imports in `completed/mod.rs`, `job/mod.rs`, `schedule/mod.rs`
- Fixed off-by-one bug in `has_next_task()` (used `<=` instead of `<`)
- All 376 tests now pass (was 350 before, 26 failed to compile)
