# Codebase Map: twerk-cli

Generated for bead `twerk-0gr` -- bug-fix bead covering test infra, trigger tests, and handler error-body drops.

---

## 1. Directory Structure

```
crates/twerk-cli/
  build.rs                  # Injects GIT_COMMIT_HASH via git rev-parse
  Cargo.toml                # Binary "twerk", depends on twerk-common/core/app/infrastructure/web
  src/
    lib.rs                  # Crate root: deny(unwrap/expect/panic), forbid(unsafe), re-exports
    main.rs                 # Binary entry: tokio::main -> run().await -> process::exit
    cli.rs                  # Top-level orchestrator: arg parsing, banner, dispatch, JSON payloads
    commands.rs             # clap Parser/Subcommand enums (Cli, Commands, RunMode, TriggerCommand, ...)
    error.rs                # CliError enum (15 variants) + ErrorKind (Validation|Runtime)
    run.rs                  # Engine bootstrap: binds API server, starts Engine, awaits shutdown
    health.rs               # GET /health, returns HealthResponse
    migrate.rs              # Postgres schema migration via twerk_infrastructure
    banner.rs               # ASCII art banner + BannerMode enum
    handlers/
      mod.rs                # Re-exports all handler sub-modules
      trigger.rs            # trigger_list/get/create/update/delete -- HTTP client fns
      queue.rs              # queue_list/get/delete
      task.rs               # task_get/task_log
      node.rs               # node_list/node_get
      metrics.rs            # metrics_get
      user.rs               # user_create
  tests/
    bdd_behavior_report.rs           # Flat claim-sheet tests + adversarial module (301 lines)
    bdd_behavioral_contract_test.rs  # Nested BDD Given-When-Then modules (378 lines)
    trigger_contract_regression_test.rs  # Live HTTP server tests for trigger handlers (210 lines)
    e2e_cli_test.rs                  # Subprocess E2E tests for help/version/JSON (238 lines)
```

---

## 2. Source Files: Public API Surface

### `lib.rs` (27 lines)
- Crate-level lints: `deny(unwrap_used, expect_used, panic)`, `allow(pedantic)`, `warn(nursery)`, `forbid(unsafe_code)`
- Public re-exports: `cli::{run, setup_logging, DEFAULT_DATASTORE_TYPE, DEFAULT_ENDPOINT, VERSION}`, `commands::Commands`, `error::CliError`
- Internal modules: `banner` (private), `cli`, `commands`, `error`, `handlers`, `health`, `migrate`, `run`

### `main.rs` (9 lines)
- `#[tokio::main] async fn main() -> process::exit(run().await)`

### `cli.rs` (699 lines) -- Core orchestrator
- **Constants**: `DEFAULT_ENDPOINT`, `DEFAULT_DATASTORE_TYPE`, `VERSION`, `GIT_COMMIT`
- **Public fns**: `run() -> i32`, `setup_logging() -> Result<(), CliError>`, `get_git_commit() -> String`
- **Private types**: `CliAction`, `ExitStatus`, `HelpVariant`
- **JSON payloads**: `json_help_payload`, `json_version_payload`, `json_success_payload`, `json_error_payload`
- **Dispatch**: `execute_command()` maps each `Commands` variant to handler fn calls
- **Config helpers**: `get_config_string`, `get_endpoint`, `get_datastore_type`, `get_postgres_dsn`
- **`#[cfg(test)]` module**: 15+ tests (const accessibility, parse args, endpoint env override, help rendering)
  - Naming: all `test_` or `snake_case` descriptive names (e.g., `get_endpoint_reads_client_endpoint_from_environment_override`)

### `commands.rs` (362 lines) -- clap definitions
- **`Cli`** struct: `json: bool`, `command: Option<Commands>`
- **`Commands`** enum: `Run{mode,hostname}`, `Migration{yes}`, `Health{endpoint}`, `Version`, `Task{command}`, `Queue{command}`, `Trigger{command}`, `Node{command}`, `Metrics{command}`, `User{command}`
- **`RunMode`** enum: `Standalone`, `Coordinator`, `Worker`
- **`TriggerCommand`** enum: `List`, `Get{id}`, `Create{body}`, `Update{id,body}`, `Delete{id}`
- **`QueueCommand`** enum: `List`, `Get{name}`, `Delete{name}`
- **`TaskCommand`** enum: `Get{id}`, `Log{id,page,size}`
- **`NodeCommand`** enum: `List`, `Get{id}`
- **`MetricsCommand`** enum: `Get`
- **`UserCommand`** enum: `Create{username}`
- **`#[cfg(test)]`** module: 10 tests (derive, default, help, version, parsing)
  - Naming: all `test_` prefixed

### `error.rs` (212 lines)
- **`ErrorKind`** enum: `Validation` (exit 2), `Runtime` (exit 1)
- **`CliError`** enum (15 variants):
  - `Config(String)` -- Validation
  - `Http(reqwest::Error)` -- Runtime
  - `HttpStatus { status: u16, reason: String }` -- Runtime
  - `HealthFailed { status: u16 }` -- Runtime
  - `InvalidBody(String)` -- Runtime
  - `MissingArgument(String)` -- Validation
  - `Migration(String)` -- Runtime
  - `UnknownDatastore(String)` -- Validation
  - `Logging(String)` -- Runtime
  - `Engine(String)` -- Runtime
  - `InvalidHostname(String)` -- Validation
  - `InvalidEndpoint(String)` -- Validation
  - `NotFound(String)` -- Runtime
  - `ApiError { code: u16, message: String }` -- Runtime
  - `Io(std::io::Error)` -- Runtime
- **`From` impls**: `DsnError -> Migration`, `EndpointError -> InvalidEndpoint`
- **Methods**: `kind() -> ErrorKind`, `exit_code() -> i32`
- **`#[cfg(test)]`** module: 10 tests covering display formatting for all variants
  - Naming: all `test_` prefixed

### `run.rs` (172 lines)
- `RunMode::into_engine_mode()` -- maps CLI mode to engine `Mode`
- `run_engine(mode, hostname) -> Result<(), CliError>` -- starts Engine + API server
- Private helpers: `validate_hostname`, `api_enabled`, `read_api_config`, `start_api_server`
- **`#[cfg(test)]`** module: 3 tests for RunMode mapping
  - Naming: all `test_` prefixed

### `health.rs` (92 lines)
- `HealthResponse` struct: `status: String`
- `health_check(endpoint, json_mode) -> Result<String, CliError>` -- GET /health
- **`#[cfg(test)]`** module: 3 tests (deserialization, debug)
  - Naming: `test_` prefixed + `#[allow(clippy::unwrap_used)]` annotations

### `migrate.rs` (125 lines)
- Re-exports `DEFAULT_POSTGRES_DSN` from `twerk_common::constants`
- `run_migration(datastore_type, postgres_dsn) -> Result<(), CliError>`
- **`#[cfg(test)]`** module: 5 tests (DSN format, variant matching, unknown rejection)
  - Naming: all `test_` prefixed

### `banner.rs` (134 lines)
- `BannerMode` enum: `Console` (default), `Log`, `Off`
- `display_banner(mode, version, git_commit)` -- prints or logs ASCII banner
- **`#[cfg(test)]`** module: 8 tests
  - Naming: descriptive snake_case (no `test_` prefix) -- **VIOLATION of Holzmann convention**

### `handlers/mod.rs` (10 lines)
- Public sub-modules: `metrics`, `node`, `queue`, `task`, `trigger`, `user`

### `handlers/trigger.rs` (326 lines) -- KEY FILE for this bead
- **Structs**: `TriggerView` (deserialize), `TriggerErrorResponse` (deserialize)
- **Public fns** (all return `Result<String, CliError>`):
  - `trigger_list(endpoint, json_mode)` -- GET /api/v1/triggers
  - `trigger_get(endpoint, id, json_mode)` -- GET /api/v1/triggers/{id}
  - `trigger_create(endpoint, body_json, json_mode)` -- POST /api/v1/triggers
  - `trigger_update(endpoint, id, body_json, json_mode)` -- PUT /api/v1/triggers/{id}
  - `trigger_delete(endpoint, id, json_mode)` -- DELETE /api/v1/triggers/{id}
- **Error handling patterns**:
  - `trigger_list`: reads body on error, attempts `TriggerErrorResponse` parse, falls back to `HttpStatus`
  - `trigger_get`: reads body first, then checks NOT_FOUND/BAD_REQUEST/is_success -- **GOOD**: body preserved
  - `trigger_create`: reads body first, checks BAD_REQUEST/CREATED/is_success
  - `trigger_update`: reads body first, checks BAD_REQUEST/NOT_FOUND/CONFLICT/is_success
  - `trigger_delete`: reads body, checks NOT_FOUND/BAD_REQUEST/NO_CONTENT|is_success
- **No `#[cfg(test)]` module** -- no inline tests

### `handlers/queue.rs` (126 lines) -- KEY FILE for this bead
- **Struct**: `QueueInfo` (deserialize)
- **Public fns**:
  - `queue_list(endpoint, json_mode)` -- GET /queues
  - `queue_get(endpoint, name, json_mode)` -- GET /queues/{name}
  - `queue_delete(endpoint, name, json_mode)` -- DELETE /queues/{name}
- **Error handling -- BODY DROPPED**:
  - `queue_list` (lines 22-28): on `!status.is_success()`, returns `HttpStatus` **without reading body** -- drops server error response
  - `queue_get` (lines 69-71): on NOT_FOUND, returns generic `NotFound` **without reading body** -- drops server error detail
  - `queue_get` (lines 73-77): on other non-success, returns `HttpStatus` **without reading body**
  - `queue_delete` (lines 108-109): on NOT_FOUND, returns generic `NotFound` **without reading body**
  - `queue_delete` (lines 112-116): on other non-success, returns `HttpStatus` **without reading body**
- **No `#[cfg(test)]` module**

### `handlers/task.rs` (178 lines) -- KEY FILE for this bead
- **Structs**: `TaskResponse`, `TaskLogPage`, `TaskLogEntry` (all deserialize)
- **Public fns**:
  - `task_get(endpoint, task_id, json_mode)` -- GET /tasks/{task_id}
  - `task_log(endpoint, task_id, page, size, json_mode)` -- GET /tasks/{task_id}/log?page=&size=
- **Error handling -- BODY DROPPED**:
  - `task_get` (lines 64-67): on NOT_FOUND, returns generic `NotFound` **without reading body**
  - `task_get` (lines 69-73): on other non-success, returns `HttpStatus` **without reading body**
  - `task_log` (lines 141-143): on NOT_FOUND, returns generic `NotFound` **without reading body**
  - `task_log` (lines 145-149): on other non-success, returns `HttpStatus` **without reading body**
- **URL encoding issue**: `task_id` is interpolated directly via `format!()` -- no percent-encoding
- **No `#[cfg(test)]` module**

### `handlers/node.rs` (119 lines)
- **Struct**: `Node` (deserialize)
- **Public fns**: `node_list`, `node_get`
- **Body dropped**: same pattern as queue/task -- error statuses return `HttpStatus` or `NotFound` without reading body
- **No `#[cfg(test)]` module**

### `handlers/metrics.rs` (92 lines)
- **Struct**: `Metrics` (deserialize)
- **Public fn**: `metrics_get`
- **Body dropped**: same pattern
- **No `#[cfg(test)]` module**

### `handlers/user.rs` (89 lines)
- **Structs**: `User`, `UserCreateResponse`
- **Public fn**: `user_create`
- Reads body before checking status -- **GOOD**: body preserved for BAD_REQUEST and CONFLICT

### `build.rs` (19 lines)
- Sets `GIT_COMMIT_HASH` env var from `git rev-parse --short HEAD`

---

## 3. Test Files

### `tests/bdd_behavior_report.rs` (301 lines)
- **Claim sheet**: 20 numbered claims covering constants, setup_logging, CLI parsing, error display, health, migration
- **Helper infra**: `LOGGING_ENV_LOCK` (LazyLock Mutex), `LoggingEnvGuard` (RAII env var restore)
- **Tests**: `claim_1_` through `claim_20_` prefix + `mod adversarial` with liar/breakage/completeness checks
- **Issues**: None significant -- clean Holzmann-style naming

### `tests/bdd_behavioral_contract_test.rs` (378 lines)
- **Nested BDD modules**: `bdd_constants_and_defaults`, `bdd_error_handling`, `bdd_commands_enum`, `bdd_setup_logging`, `bdd_completeness_check`, `bdd_liar_check_cli_error_display`, `bdd_breakage_check`
- **Pattern**: `mod given_X { fn then_Y() { ... } }`
- **Issues for this bead**:
  - Tests use `#[cfg(test)] mod` wrapping at top level, then nest further `mod given_X` -- this is technically fine but the outer `#[cfg(test)]` on a `mod` that is itself inside `tests/` is redundant
  - Test names inside nested modules like `then_config_error_formats_correctly` -- these are fine for BDD style
  - Missing coverage: `CliError::HttpStatus`, `CliError::NotFound`, `CliError::ApiError`, `CliError::InvalidHostname`, `CliError::InvalidEndpoint` variants are NOT exercised in this file's completeness check (only 9 of 15 variants constructed)

### `tests/trigger_contract_regression_test.rs` (210 lines)
- **Test infra**: `HttpTestServer` (spawn+shutdown), `spawn_router`, `parse_json`, `assert_rfc3339_field`, `assert_timestamp_fields`
- **Bad-timestamp test server**: `start_bad_timestamp_server` with routes returning `[2026,4,22]` arrays instead of RFC3339 strings
- **Tests**: 6 async tests -- RFC3339 acceptance via live server + 4 rejection tests + delete no-content
- **Dependency**: Uses `twerk_web::helpers::start_test_server` for real server integration
- **Good coverage** of trigger handlers but: no tests for negative HTTP statuses (400, 404, 409, 500), no TriggerId boundary tests, no mutation kill tests

### `tests/e2e_cli_test.rs` (238 lines)
- **Test infra**: `cli_binary()`, `run_cli()`, `stdout_string()`, `stderr_string()`, `JsonCliOutput` (deserialize), `parse_json_output`
- **Tests**: 14 tests covering --help, --version, version subcommand, JSON mode (help/version/error), propagated subcommand versions, invalid run mode, missing run mode, health endpoint validation, health connection failure
- **No tests for**: trigger/queue/task/node/metrics/user subcommand E2E flows

---

## 4. Cross-Crate Dependencies

### `crates/twerk-web/src/api/trigger_api/domain.rs` (278 lines)
- **Server-side domain types** that the CLI trigger handlers must interoperate with:
  - `TriggerId` -- newtype wrapper, `parse()` validates length 3-64 and `[a-zA-Z0-9_-]` chars
  - `TRIGGER_ID_MIN_LEN = 3`, `TRIGGER_ID_MAX_LEN = 64`
  - `TriggerUpdateRequest`, `Trigger`, `TriggerView` (with `serde(with = "time::serde::rfc3339")` timestamps)
  - `TriggerUpdateError` enum (InvalidIdFormat, UnsupportedContentType, MalformedJson, ValidationFailed, IdMismatch, TriggerNotFound, VersionConflict, Persistence, Serialization)
  - Validation fns: `validate_trigger_update`, `validate_trigger_create`, `apply_trigger_update`
- **Key for this bead**: CLI handler `trigger_get/trigger_update/trigger_delete` pass raw `id` string to URL -- no client-side TriggerId validation. Server validates and returns 400/404.

### `crates/twerk-web/src/helpers.rs` (93 lines)
- **`TestServer`** struct with `addr`, `broker`, `datastore`, `shutdown_tx`
- **`start_test_server()`** -- binds to `127.0.0.1:0`, spawns axum with in-memory broker+datastore
- Used by `trigger_contract_regression_test.rs`

---

## 5. Handler Error-Body Drop Summary

| Handler | Fn | Status Check | Body Read Before Error? | Issue |
|---------|-----|-------------|------------------------|-------|
| trigger | `trigger_list` | `!is_success` | YES -- reads body, parses `TriggerErrorResponse` | OK |
| trigger | `trigger_get` | NOT_FOUND, BAD_REQUEST | YES -- reads body first | OK |
| trigger | `trigger_create` | BAD_REQUEST | YES | OK |
| trigger | `trigger_update` | BAD_REQUEST, NOT_FOUND, CONFLICT | YES | OK |
| trigger | `trigger_delete` | NOT_FOUND, BAD_REQUEST | YES | OK |
| **queue** | **`queue_list`** | **`!is_success`** | **NO** | **DROPS body** |
| **queue** | **`queue_get`** | **NOT_FOUND, `!is_success`** | **NO** | **DROPS body** |
| **queue** | **`queue_delete`** | **NOT_FOUND, `!is_success`** | **NO** | **DROPS body** |
| **task** | **`task_get`** | **NOT_FOUND, `!is_success`** | **NO** | **DROPS body** |
| **task** | **`task_log`** | **NOT_FOUND, `!is_success`** | **NO** | **DROPS body** |
| node | `node_list` | `!is_success` | NO | Drops body |
| node | `node_get` | NOT_FOUND, `!is_success` | NO | Drops body |
| metrics | `metrics_get` | `!is_success` | NO | Drops body |
| user | `user_create` | BAD_REQUEST, CONFLICT | YES | OK |

**Fix targets for this bead**: `queue_list`, `queue_get`, `queue_delete`, `task_get`, `task_log` (per bead scope).

---

## 6. URL Encoding Issue

All handler functions interpolate IDs/names directly via `format!()`:
```rust
format!("{}/api/v1/triggers/{}", endpoint, id)       // trigger.rs:94
format!("{}/queues/{}", endpoint, name)                // queue.rs:63
format!("{}/tasks/{}", endpoint, task_id)              // task.rs:59
```

If an ID/name contains characters like `/`, `?`, `#`, `%`, or `+`, the URL will be malformed. No percent-encoding is applied. The `TriggerId` server-side validation restricts to `[a-zA-Z0-9_-]`, but queue names and task IDs have no such restriction documented at the CLI level.

---

## 7. Test Naming Convention Audit

### `#[cfg(test)]` modules inside source files:
| File | Prefix Used | Count | Violation? |
|------|------------|-------|-----------|
| `error.rs` | `test_` | 10 | No |
| `commands.rs` | `test_` | 10 | No |
| `cli.rs` | Mixed: `test_` + descriptive snake_case | 15 | **Mixed** -- some use `test_`, some don't (e.g., `default_endpoint_is_localhost_http`, `constants_are_accessible_without_mutation`) |
| `run.rs` | `test_` | 3 | No |
| `health.rs` | `test_` | 3 | No |
| `migrate.rs` | `test_` | 5 | No |
| `banner.rs` | **No prefix** | 8 | **Violation** -- names like `banner_mode_from_str_returns_expected_variant_for_supported_and_unknown_values` |

### Integration test files (`tests/`):
| File | Pattern | Notes |
|------|---------|-------|
| `bdd_behavior_report.rs` | `claim_N_` prefix + `mod adversarial` | Holzmann-compliant |
| `bdd_behavioral_contract_test.rs` | `then_` prefix in nested `given_` modules | BDD style -- OK |
| `trigger_contract_regression_test.rs` | Descriptive snake_case | No `test_` prefix required (integration tests) |
| `e2e_cli_test.rs` | Descriptive snake_case | No `test_` prefix required (integration tests) |

---

## 8. Missing Test Coverage for This Bead

### Trigger tests needed:
- **Negative HTTP status tests**: 400 (bad request), 404 (not found), 409 (conflict), 500 (internal server error) for all 5 trigger handler fns
- **TriggerId boundary tests**: 2-char (below min), 65-char (above max), special chars (`/`, `?`, spaces), empty string
- **Mutation kill**: verify that removing the `TriggerErrorResponse` parse path causes test failure

### Queue handler body-drops to test:
- `queue_list` with 500 response should preserve server error message
- `queue_get` with 404 should read body for server error detail
- `queue_delete` with 404 should read body for server error detail

### Task handler body-drops to test:
- `task_get` with 404 should read body for server error detail
- `task_get` with 500 should preserve server error message
- `task_log` with 404 should read body for server error detail

### URL encoding tests:
- Trigger ID with special chars (e.g., `trg/slash`, `trig?param`)
- Queue name with spaces or special chars
- Task ID with percent-encoded values

---

## 9. Crate Dependency Graph (twerk-cli perspective)

```
twerk-cli
  -> twerk-common     (load_config, constants::DEFAULT_POSTGRES_DSN)
  -> twerk-core       (domain::{Dsn, Endpoint, DsnError, EndpointError})
  -> twerk-app        (engine::{Config, Engine, Mode})
  -> twerk-infrastructure (config, reexec, datastore::postgres, broker::inmemory)
  -> twerk-web        (api::{create_router, AppState, Config}, helpers::start_test_server)
  -> axum, clap, tokio, anyhow, thiserror, tracing, serde, serde_json, config, time, itertools, tap, futures-util, reqwest
```

Test-only dependencies: `twerk_web::helpers::start_test_server` (used in trigger_contract_regression_test.rs), `axum` (for test routers), `tokio::net::TcpListener`, `tokio::sync::oneshot`.
