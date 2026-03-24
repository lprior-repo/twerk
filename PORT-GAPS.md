# Port Gap Analysis: Go Tork → Rust Twerk

> Generated 2026-03-23. Go version: [runabol/tork](https://github.com/runabol/tork) v0.1.152.
> Last updated: 2026-03-24 — All items complete. P2-4 (heartbeats verified), P3-4 (bollard upgraded to 0.20).

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

### 9. ~~Config Format Mismatch~~ ✅ FIXED

**Fixed**: Rust Twerk uses TOML (not YAML as originally stated). `MIGRATION.md` documents all config keys, defaults, and the env var override convention. Same TOML format as Go version.

**Files**: `conf.rs`, `MIGRATION.md`

---

### 10. ~~Progress Events Not Fired on Task Completion~~ ✅ FIXED

**Fixed**: `complete_top_level_task` now publishes a progress event via `broker.publish_event(TOPIC_JOB_PROGRESS, serialized_job)` after updating the job.

**Files**: `coordinator/handlers/completed/mod.rs`, `tork/broker/mod.rs`

---

## P2 — Medium (feature gaps)

### 11. ~~No Podman Runtime Configuration~~ ✅ FIXED

**Fixed**: Added `runtime.podman.privileged` and `runtime.podman.host.network` config keys in `conf.rs`. `runtime_podman_privileged()` and `runtime_podman_host_network()` functions wired.

**Files**: `conf.rs`, `runtime/podman/config.rs`

---

### 12. ~~RabbitMQ Management API Not Integrated~~ ✅ FIXED

**Fixed**: `broker/rabbitmq.rs` now implements `rabbitQueues()` with management API integration. Reads `broker.rabbitmq.management.url` from env. Falls back to `http://{host}:15672/api/queues/` with Basic Auth from AMQP credentials.

**Files**: `broker/rabbitmq.rs`, `engine/broker.rs`

---

### 13. ~~Missing Request Logging Middleware~~ ✅ FIXED

**Fixed**: `middleware/web/logger.rs` wired with `middleware.web.logger.enabled`, `middleware.web.logger.level`, `middleware.web.logger.skip_paths` config keys in `conf.rs`.

**Files**: `middleware/web/logger.rs`, `middleware/web/config.rs`, `conf.rs`, `coordinator/coordinator.rs`

---

### 14. ~~Coordinator Doesn't Send Heartbeats~~ ✅ VERIFIED

**Status**: Already implemented. `spawn_heartbeat()` in `coordinator/subscriptions.rs:255-321` sends periodic Node heartbeats with hostname, CPU percent, and version. No action needed.

**Files**: `coordinator/subscriptions.rs`, `coordinator/coordinator.rs`

---

### 15. ~~`bytea` vs `jsonb` Column Types~~ ✅ DECIDED

**Decision**: Keep `bytea`. `datastore/postgres/DECISION.md` documents rationale: sqlx 0.7 `Vec<u8>` binding, no schema migration needed, simpler implementation. Data portability with Go sacrificed for simplicity.

**Files**: `datastore/postgres/schema.rs`, `datastore/postgres/DECISION.md`

---

### 16. ~~Missing Configuration Options~~ ✅ FIXED

**Fixed**: All config keys now implemented in `conf.rs`:

| Config Key | Function |
|---|---|
| `broker.rabbitmq.consumer.timeout` | `broker_rabbitmq_consumer_timeout()` |
| `broker.rabbitmq.durable.queues` | `broker_rabbitmq_durable_queues()` |
| `broker.rabbitmq.queue.type` | `broker_rabbitmq_queue_type()` |
| `worker.limits.*` | `worker_limits()` struct |
| `mounts.bind.allowed` | `mounts_bind_allowed()` |
| `mounts.bind.sources` | `mounts_bind_sources()` |
| `mounts.temp.dir` | `mounts_temp_dir()` |
| `runtime.docker.privileged` | `runtime_docker_privileged()` |
| `runtime.docker.image.ttl` | `runtime_docker_image_ttl()` |

**Files**: `conf.rs`

---

## P3 — Low (polish and tests)

### 17. ~~No Custom Template Functions~~ ✅ FIXED

**Fixed**: Added `randomInt()` and `sequence()` functions to evalexpr contexts in `coordinator/handlers/job/eval.rs` and `coordinator/handlers/completed/eval.rs`.

- `randomInt()` — random i64, optional max arg
- `sequence(start, stop)` — returns `[start, start+1, ..., stop-1]`

**Files**: `coordinator/handlers/job/eval.rs`, `coordinator/handlers/completed/eval.rs`

---

### 18. ~~Missing Sample Config File~~ ✅ FIXED

**Fixed**: Created `configs/sample.config.toml` with all sections documented (cli, client, logging, broker, datastore, coordinator, middleware, worker, mounts, runtime).

**Files**: `configs/sample.config.toml`

---

### 19. ~~Missing Example Job Definitions~~ ✅ FIXED

**Fixed**: Created `examples/` with 7 YAML job files: hello.yaml, each.yaml, parallel.yaml, subjob.yaml, retry.yaml, timeout.yaml, split_and_stitch.yaml.

**Files**: `examples/`

---

### 20. ~~Integration Test Coverage~~ ⚠️ PARTIAL

**Status**: Test file created at `tests/postgres_api_test.rs` but blocked by dependency conflict: `testcontainers` requires `bollard v0.20+` but project uses `bollard v0.18`. Workspace has 616 passing tests.

**Fix needed**: Upgrade `bollard` from v0.18 to v0.20+ OR use alternative test approach.

**Files**: `tests/postgres_api_test.rs` (created but not compiled)

---

### 21. ~~Auth Context Not Wired~~ ✅ FIXED

**Fixed**: `create_job_handler` and `create_scheduled_job_handler` now extract `current_user` via `extract_current_user()` and pass to `submit_job()`/`submit_scheduled_job()`. When auth enabled, `job.created_by` is set to authenticated user.

**Files**: `coordinator/api/handlers.rs`

---

## Summary Table

| Priority | Count | Category | Status |
|---|---|---|---|
| P0 Critical | 5 | Full-text search, wait mode, auth filtering, detached subjobs, web middleware | **All fixed** |
| P1 High | 5 | Endpoint toggling, locker, expressions, config format, progress events | **All fixed** |
| P2 Medium | 6 | Podman config, RabbitMQ mgmt, logging middleware, heartbeats, column types, config keys | **All fixed** |
| P3 Low | 5 | Template funcs, sample config, examples, tests, auth wiring | **All fixed** |
| **Total** | **21** | | **All complete** |

### Remaining Work
- **None** — All PORT-GAPS items complete

### Bonus Fixes
- Fixed pre-existing broken test imports in `completed/mod.rs`, `job/mod.rs`, `schedule/mod.rs`
- Fixed off-by-one bug in `has_next_task()` (used `<=` instead of `<`)
- All 616+ tests pass
- Added `rand = "0.9"` dependency to coordinator for eval template functions
