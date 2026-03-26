# Refactor Plan: File Split Implementation

## Overview
This document provides a step-by-step plan for splitting monolithic files while maintaining backward compatibility and ensuring all tests pass.

## Execution Order
1. **docker/auth.rs** (853 lines) - Smallest, simplest
2. **podman/mod.rs** (1367 lines) - Medium complexity
3. **postgres/records.rs** (1448 lines) - More complex, more conversions
4. **twerk-app/engine/mod.rs** (1379 lines) - Largest, most interconnected

---

## Phase 1: Split `docker/auth.rs`

### Step 1.1: Create `docker/auth_config.rs`
**Purpose**: Config structs and their methods

**Content**:
- `AuthConfig` struct (with serde derive)
- `ProxyConfig` struct (with serde derive)
- `KubernetesConfig` struct (with serde derive)
- `Config` struct (with serde derive)
- `Config::new()`
- `Config::load()`
- `Config::load_config()`
- `Config::load_from_path()`
- `Config::get_credentials()` (partial - just the struct part)

**Lines**: ~180 lines

### Step 1.2: Create `docker/auth_resolver.rs`
**Purpose**: Credential resolution logic

**Content**:
- `resolve_auth_config()` function
- `decode_base64_auth()` function
- `get_registry_credentials()` function

**Lines**: ~100 lines

### Step 1.3: Create `docker/credential_helper.rs`
**Purpose**: Credential helper logic (if any external helper calls)

**Content**:
- `get_from_helper()` function
- Any helper-related error handling

**Lines**: ~50 lines (may be minimal)

### Step 1.4: Update `docker/mod.rs`
**Content**:
- Module declarations
- Re-exports of all public types and functions
- Error type re-export

**Lines**: ~30 lines

### Step 1.5: Update imports in dependent files
**Files to update**:
- Any file that imports from `docker::auth`

### Step 1.6: Move tests
- Move tests to appropriate modules:
  - Config tests → `auth_config.rs`
  - Resolver tests → `auth_resolver.rs`
  - Helper tests → `credential_helper.rs`

### Verification
```bash
cd crates/twerk-infrastructure
cargo check
cargo test --package twerk-infrastructure --lib runtime::docker
```

---

## Phase 2: Split `podman/mod.rs`

### Step 2.1: Create `podman/error.rs`
**Purpose**: Error types

**Content**:
- `PodmanError` enum (all variants)
- `impl std::fmt::Debug for PodmanError`
- `impl std::error::Error for PodmanError`

**Lines**: ~80 lines

### Step 2.2: Create `podman/config.rs`
**Purpose**: Configuration

**Content**:
- `PodmanConfig` struct
- `impl std::fmt::Debug for PodmanConfig`
- Default implementation

**Lines**: ~25 lines

### Step 2.3: Create `podman/types.rs`
**Purpose**: Domain types

**Content**:
- `Task` struct
- `Mount` struct
- `MountType` enum (with Display impl)
- `TaskLimits` struct
- `Registry` struct
- `Probe` struct
- `RegistryCredentials` struct (internal use)

**Lines**: ~80 lines

### Step 2.4: Create `podman/runtime.rs`
**Purpose**: PodmanRuntime implementation

**Content**:
- `PodmanRuntime` struct
- `ContainerGuard` struct
- `impl PodmanRuntime`
  - Constructor: `new()`
  - Background tasks: `start_puller()`, `start_pruner()`, `do_pull_request()`
  - Image operations: `image_pull()`, `verify_image()`, `prune_images()`
  - Public API: `run()`, `run_inner()`, `do_run()`, `do_run_inner()`
  - Probe support: `get_host_port()`, `probe_container()`, `http_get()`
  - Resource parsing: `parse_cpus()`, `parse_memory()`, `parse_duration()`
  - Container lifecycle: `stop_container()`, `stop_container_static()`
  - Progress reporting: `report_progress()`
  - Health check: `health_check()`
  - Helpers: `extract_registry_host()`

**Lines**: ~900 lines (needs to be under 300!)

**NOTE**: `runtime.rs` will still be too long. Further split needed:
- `runtime/core.rs` - PodmanRuntime struct and constructor
- `runtime/lifecycle.rs` - Container lifecycle methods
- `runtime/images.rs` - Image operations
- `runtime/probe.rs` - Probe support
- `runtime/resources.rs` - Resource parsing
- `runtime/api.rs` - Public API methods

This is a **critical finding**: `podman/mod.rs` cannot be split into just 4-5 modules; the runtime implementation alone is ~900 lines.

### Step 2.5: Keep `podman/volume.rs` as-is
Already a separate module, no changes needed.

### Step 2.6: Create `podman/tests.rs`
**Purpose**: Test module

**Content**:
- Move all test functions from `mod.rs`
- Organize by feature:
  - `test_volume_mounting()`
  - `test_image_pull()`
  - `test_container_lifecycle()`
  - `test_probe_support()`
  - `test_resource_parsing()`

**Lines**: ~200 lines

### Step 2.7: Update `podman/mod.rs`
**Content**:
- Module declarations
- Re-exports
- Slug module (if needed)

**Lines**: ~25 lines

### Verification
```bash
cd crates/twerk-infrastructure
cargo check
cargo test --package twerk-infrastructure --lib runtime::podman
```

---

## Phase 3: Split `postgres/records.rs`

### Step 3.1: Create `postgres/helpers.rs`
**Purpose**: Shared helper functions

**Content**:
- `str_to_task_state()`
- `json_decode<T>()`
- `json_decode_flatten<T>()`

**Lines**: ~25 lines

### Step 3.2: Create `postgres/task.rs`
**Purpose**: TaskRecord

**Content**:
- `TaskRecord` struct
- `impl TaskRecord`
  - `to_task()` conversion
- Tests for TaskRecord conversion

**Lines**: ~150 lines

### Step 3.3: Create `postgres/job.rs`
**Purpose**: JobRecord and JobPermRecord

**Content**:
- `JobRecord` struct
- `impl JobRecord`
  - `to_job()` conversion
- `JobPermRecord` struct
- Tests for JobRecord conversion

**Lines**: ~200 lines

### Step 3.4: Create `postgres/scheduled_job.rs`
**Purpose**: ScheduledJobRecord and ScheduledPermRecord

**Content**:
- `ScheduledJobRecord` struct
- `impl ScheduledJobRecord`
  - `to_scheduled_job()` conversion
- `ScheduledPermRecord` struct
- Tests for ScheduledJobRecord conversion

**Lines**: ~180 lines

### Step 3.5: Create `postgres/node.rs`
**Purpose**: NodeRecord

**Content**:
- `NodeRecord` struct
- `impl NodeRecord`
  - `to_node()` conversion
- Tests for NodeRecord conversion

**Lines**: ~80 lines

### Step 3.6: Create `postgres/user.rs`
**Purpose**: UserRecord

**Content**:
- `UserRecord` struct
- `impl UserRecord`
  - `to_user()` conversion
- Tests for UserRecord conversion

**Lines**: ~50 lines

### Step 3.7: Create `postgres/role.rs`
**Purpose**: RoleRecord

**Content**:
- `RoleRecord` struct
- `impl RoleRecord`
  - `to_role()` conversion
- Tests for RoleRecord conversion

**Lines**: ~40 lines

### Step 3.8: Create `postgres/log.rs`
**Purpose**: TaskLogPartRecord

**Content**:
- `TaskLogPartRecord` struct
- `impl TaskLogPartRecord`
  - `to_task_log_part()` conversion
- Tests for TaskLogPartRecord conversion

**Lines**: ~40 lines

### Step 3.9: Update `postgres/mod.rs`
**Content**:
- Module declarations
- Re-exports of all record types
- Re-exports of helper functions
- Integration tests

**Lines**: ~30 lines

### Verification
```bash
cd crates/twerk-infrastructure
cargo check
cargo test --package twerk-infrastructure --lib datastore::postgres::records
```

---

## Phase 4: Split `twerk-app/engine/mod.rs`

### Step 4.1: Create `engine/engine/types.rs`
**Purpose**: Type definitions

**Content**:
- `Mode` enum
- `State` enum
- `TaskEventType` enum
- `JobEventType` enum
- Handler types:
  - `TaskHandlerFunc`
  - `JobHandlerFunc`
  - `LogHandlerFunc`
  - `NodeHandlerFunc`
  - `TaskMiddlewareFunc`
  - `JobMiddlewareFunc`
  - `LogMiddlewareFunc`
  - `NodeMiddlewareFunc`
  - `WebMiddlewareFunc`
  - `EndpointHandler`

**Lines**: ~120 lines

### Step 4.2: Create `engine/engine/errors.rs`
**Purpose**: Handler error types

**Content**:
- `TaskHandlerError` enum
- `JobHandlerError` enum
- `LogHandlerError` enum
- `NodeHandlerError` enum

**Lines**: ~40 lines

### Step 4.3: Create `engine/engine/config.rs`
**Purpose**: Configuration structs

**Content**:
- `Middleware` struct
- `Config` struct

**Lines**: ~25 lines

### Step 4.4: Create `engine/engine/impl.rs`
**Purpose**: Engine implementation

**Content**:
- `Engine` struct definition
- `impl std::fmt::Debug for Engine`
- `impl Engine`
  - Constructor: `new()`
  - Lifecycle: `state()`, `mode()`, `set_mode()`, `start()`, `run()`, `terminate()`, `await_shutdown()`
  - Getters: `broker()`, `datastore()`, `broker_proxy()`, `datastore_proxy()`
  - Registration: `register_web_middleware()`, `register_task_middleware()`, etc.
  - Runtime registration: `register_runtime()`, `register_datastore_provider()`, etc.
  - Job submission: `submit_job()`
  - Mode-specific: `run_coordinator()`, `run_worker()`, `run_standalone()`
  - Helpers: `resolve_locker_type()`, `await_signal_or_channel()`

**Lines**: ~700 lines (needs to be under 300!)

**NOTE**: `impl.rs` will still be too long. Further split needed:
- `engine/engine/core.rs` - Engine struct and constructor
- `engine/engine/lifecycle.rs` - Start, run, terminate methods
- `engine/engine/registration.rs` - All register_* methods
- `engine/engine/jobs.rs` - Job submission logic
- `engine/engine/modes.rs` - Mode-specific implementations
- `engine/engine/helpers.rs` - Helper functions

This is another **critical finding**: the engine implementation is too large for a single module.

### Step 4.5: Create `engine/engine/mock.rs`
**Purpose**: MockRuntime for testing

**Content**:
- `MockRuntime` struct
- `impl Runtime for MockRuntime`

**Lines**: ~20 lines

### Step 4.6: Create `engine/engine/tests.rs`
**Purpose**: Engine tests

**Content**:
- All test functions from original `mod.rs`
- Organized by test category:
  - `test_engine_new()`
  - `test_start_*()`
  - `test_middleware_*()`
  - `test_register_*()`
  - `test_submit_job()`

**Lines**: ~300 lines

### Step 4.7: Update `engine/mod.rs`
**Content**:
- Module declarations
- Re-exports of all public types and functions
- Re-exports from submodules

**Lines**: ~35 lines

### Verification
```bash
cd crates/twerk-app
cargo check
cargo test --package twerk-app --lib engine
```

---

## Final Verification

After all phases complete:

```bash
# Full project check
cargo check --workspace

# Full project tests
cargo test --workspace

# Check line counts
wc -l crates/twerk-app/src/engine/*.rs crates/twerk-app/src/engine/engine/*.rs
wc -l crates/twerk-infrastructure/src/runtime/docker/*.rs
wc -l crates/twerk-infrastructure/src/runtime/podman/*.rs
wc -l crates/twerk-infrastructure/src/datastore/postgres/*.rs

# Verify all files are under 300 lines
```

---

## Rollback Plan

If any phase fails:

1. **Phase 1** (docker): Keep `auth.rs` intact if compilation fails
2. **Phase 2** (podman): Keep `mod.rs` intact if compilation fails
3. **Phase 3** (postgres): Keep `records.rs` intact if compilation fails
4. **Phase 4** (engine): Keep `mod.rs` intact if compilation fails

Each phase is independent and can be rolled back without affecting others.

---

## Success Criteria

1. All files under 300 lines
2. `cargo check --workspace` passes
3. `cargo test --workspace` passes
4. Public API unchanged (backward compatibility)
5. All tests pass with same coverage
