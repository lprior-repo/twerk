# Architectural Review: File Split Analysis

## Overview
This document analyzes four monolithic files exceeding the 300-line limit and proposes module splits to improve maintainability, testability, and adherence to DDD principles.

---

## File 1: `crates/twerk-infrastructure/src/runtime/docker/auth.rs` (853 lines)

### Current Module Boundaries
- **Domain Errors**: `AuthError` enum (lines 25-51)
- **Data Structures**: `AuthConfig`, `ProxyConfig`, `KubernetesConfig`, `Config` (lines 57-176)
- **Credential Resolution**: `resolve_auth_config`, `decode_base64_auth`, `get_registry_credentials` (lines 286-283)
- **File Operations**: `config_path`, `user_home_config_path`, `load_from_path` (lines 341-366)
- **Tests**: 25+ test functions (lines 368-853)

### Proposed New Module Boundaries
```
docker/
├── mod.rs           # Exports and public API
├── credential_helper.rs  # Credential helper logic (get_from_helper)
├── auth_config.rs    # AuthConfig, Config structs and their methods
└── auth_resolver.rs  # Credential resolution logic (resolve_auth_config, decode_base64_auth)
```

### Dependencies Between Modules
- `mod.rs` imports from all three modules
- `auth_config.rs` uses `credential_helper` for `get_credentials`
- `auth_resolver.rs` is pure (no external dependencies)
- `credential_helper.rs` uses `io` and `serde_json`

### Migration Plan
1. Extract `AuthConfig`, `ProxyConfig`, `KubernetesConfig`, `Config` to `auth_config.rs`
2. Extract `resolve_auth_config`, `decode_base64_auth`, `get_registry_credentials` to `auth_resolver.rs`
3. Extract credential helper logic (if any) to `credential_helper.rs`
4. Update imports and re-exports in `mod.rs`
5. Keep tests in their original modules (module-level tests)

---

## File 2: `crates/twerk-infrastructure/src/runtime/podman/mod.rs` (1367 lines)

### Current Module Boundaries
- **Error Types**: `PodmanError` enum (lines 43-116)
- **Config**: `PodmanConfig` struct (lines 120-141)
- **Traits**: `Broker`, `Mounter` traits (lines 145-161)
- **Pull Request**: `PullRequest`, `RegistryCredentials` structs (lines 165-176)
- **Runtime Core**: `PodmanRuntime` struct and impl (lines 179-1282)
  - Background tasks: `start_puller`, `start_pruner`
  - Container lifecycle: `stop_container`, `stop_container_static`
  - Image operations: `image_pull`, `verify_image`, `prune_images`
  - Probe support: `probe_container`, `http_get`
  - Resource parsing: `parse_cpus`, `parse_memory`, `parse_duration`
- **Domain Types**: `Task`, `Mount`, `MountType`, `TaskLimits`, `Registry`, `Probe` (lines 1286-1356)
- **Tests**: `tests` module (line 15)
- **Helpers**: `slug` module (lines 1359-1367)

### Proposed New Module Boundaries
```
podman/
├── mod.rs           # Main exports, public API
├── error.rs         # PodmanError enum
├── config.rs        # PodmanConfig struct
├── types.rs         # Domain types (Task, Mount, MountType, etc.)
├── runtime.rs       # PodmanRuntime implementation
├── volume.rs        # VolumeMounter (already exists as separate module)
└── tests.rs         # Test module (re-exported from mod.rs)
```

### Dependencies Between Modules
- `mod.rs` imports from all modules
- `runtime.rs` uses `error.rs`, `config.rs`, `types.rs`
- `types.rs` is pure (no external dependencies)
- `volume.rs` already exists (no changes needed)
- `tests.rs` imports from all modules for tests

### Migration Plan
1. Extract `PodmanError` to `error.rs`
2. Extract `PodmanConfig` to `config.rs`
3. Extract all domain types (`Task`, `Mount`, `MountType`, etc.) to `types.rs`
4. Move `PodmanRuntime` impl to `runtime.rs`
5. Keep `volume.rs` as-is (already separate)
6. Keep `tests` module but split into test functions by feature
7. Update imports in all modules

---

## File 3: `crates/twerk-infrastructure/src/datastore/postgres/records.rs` (1448 lines)

### Current Module Boundaries
- **TaskRecord**: Task database record and conversion (lines 21-129)
- **JobRecord**: Job database record and conversion (lines 132-237)
- **ScheduledJobRecord**: Scheduled job record and conversion (lines 240-315)
- **JobPermRecord**: Job permission record (lines 317-325)
- **ScheduledPermRecord**: Scheduled job permission record (lines 328-335)
- **NodeRecord**: Node database record and conversion (lines 338-379)
- **TaskLogPartRecord**: Task log part record and conversion (lines 381-403)
- **UserRecord**: User database record and conversion (lines 406-430)
- **RoleRecord**: Role database record and conversion (lines 432-452)
- **Helpers**: `str_to_task_state`, `json_decode`, `json_decode_flatten` (lines 454-477)
- **Tests**: 35+ test functions covering all record conversions (lines 479-1448)

### Proposed New Module Boundaries
```
postgres/
├── mod.rs           # Exports all record types
├── task.rs          # TaskRecord and conversion
├── job.rs           # JobRecord, JobPermRecord and conversion
├── scheduled_job.rs # ScheduledJobRecord, ScheduledPermRecord and conversion
├── node.rs          # NodeRecord and conversion
├── user.rs          # UserRecord and conversion
├── role.rs          # RoleRecord and conversion
├── log.rs           # TaskLogPartRecord and conversion
└── helpers.rs       # Shared helper functions (json_decode, str_to_task_state)
```

### Dependencies Between Modules
- All record modules import from `helpers.rs`
- `mod.rs` re-exports all types
- No cross-dependencies between record modules
- Tests can be in individual modules or centralized in `mod.rs`

### Migration Plan
1. Extract `TaskRecord` to `task.rs`
2. Extract `JobRecord`, `JobPermRecord` to `job.rs`
3. Extract `ScheduledJobRecord`, `ScheduledPermRecord` to `scheduled_job.rs`
4. Extract `NodeRecord` to `node.rs`
5. Extract `UserRecord` to `user.rs`
6. Extract `RoleRecord` to `role.rs`
7. Extract `TaskLogPartRecord` to `log.rs`
8. Extract helper functions to `helpers.rs`
9. Move tests to appropriate modules or keep in `mod.rs`
10. Update imports in `mod.rs`

---

## File 4: `crates/twerk-app/src/engine/mod.rs` (1379 lines)

### Current Module Boundaries
The file is already partially split with submodules:
- `broker` (line 9)
- `coordinator` (line 10)
- `datastore` (line 11)
- `default` (line 12)
- `locker` (line 13)
- `worker` (line 14)

However, the main `engine` module (lines 36-1379) contains:
- **Type Definitions**: `Mode`, `State`, handler types, middleware types (lines 59-200)
- **Error Types**: `TaskHandlerError`, `JobHandlerError`, `LogHandlerError`, `NodeHandlerError` (lines 167-200)
- **Config & Middleware**: `Config`, `Middleware` structs (lines 206-224)
- **Engine Struct**: `Engine` definition (lines 226-256)
- **Engine Implementation**: All methods (lines 258-828)
  - Lifecycle: `start`, `run`, `terminate`
  - Mode-specific: `run_coordinator`, `run_worker`, `run_standalone`
  - Registration: `register_*` methods
  - Helper functions: `resolve_locker_type`, `await_signal_or_channel`
- **Mock Runtime**: `MockRuntime` for tests (lines 888-903)
- **Tests**: 25+ test functions (lines 905-1379)

### Proposed New Module Boundaries
```
engine/
├── mod.rs                      # Main exports
├── engine/                     # Core engine (already exists but needs split)
│   ├── types.rs               # Type definitions (Mode, State, handler types)
│   ├── errors.rs              # Handler error types
│   ├── config.rs              # Config, Middleware structs
│   ├── impl.rs                # Engine implementation (all methods)
│   └── tests.rs               # Engine tests
├── broker/                     # Already exists
├── coordinator/                # Already exists
├── datastore/                  # Already exists
├── locker/                     # Already exists
├── worker/                     # Already exists
└── default/                    # Already exists
```

### Dependencies Between Modules
- `engine/types.rs` is pure (no dependencies)
- `engine/errors.rs` depends on `thiserror`
- `engine/config.rs` depends on `types.rs`
- `engine/impl.rs` depends on `types.rs`, `errors.rs`, `config.rs`
- `engine/tests.rs` imports from all engine modules

### Migration Plan
1. Extract type definitions to `engine/types.rs`
2. Extract error types to `engine/errors.rs`
3. Extract `Config`, `Middleware` to `engine/config.rs`
4. Move `Engine` struct to `engine/impl.rs` (or keep in `impl.rs`)
5. Move all `Engine` methods to `engine/impl.rs`
6. Extract `MockRuntime` to `engine/mock.rs`
7. Move tests to `engine/tests.rs`
8. Update imports in main `mod.rs`

---

## Summary of Dependencies

```
twerk-app/
├── src/
│   └── engine/
│       ├── mod.rs                    (exports all modules)
│       ├── engine/
│       │   ├── types.rs              (Mode, State, handler types)
│       │   ├── errors.rs             (handler errors)
│       │   ├── config.rs             (Config, Middleware)
│       │   ├── impl.rs               (Engine implementation)
│       │   ├── mock.rs               (MockRuntime)
│       │   └── tests.rs              (tests)
│       ├── broker/
│       ├── coordinator/
│       ├── datastore/
│       ├── locker/
│       ├── worker/
│       └── default/

twerk-infrastructure/
├── src/
│   └── runtime/
│       ├── docker/
│       │   ├── mod.rs
│       │   ├── auth_config.rs
│       │   ├── auth_resolver.rs
│       │   └── credential_helper.rs
│       └── podman/
│           ├── mod.rs
│           ├── error.rs
│           ├── config.rs
│           ├── types.rs
│           ├── runtime.rs
│           ├── volume.rs
│           └── tests.rs
│
└── src/
    └── datastore/
        └── postgres/
            ├── mod.rs
            ├── task.rs
            ├── job.rs
            ├── scheduled_job.rs
            ├── node.rs
            ├── user.rs
            ├── role.rs
            ├── log.rs
            └── helpers.rs
```

---

## Implementation Strategy

### Phase 1: docker/auth.rs (Smallest - 853 lines)
1. Create `auth_config.rs` with Config structs
2. Create `auth_resolver.rs` with resolution logic
3. Create `credential_helper.rs` (if needed)
4. Update `mod.rs` with re-exports

### Phase 2: podman/mod.rs (1367 lines)
1. Create `error.rs` with PodmanError
2. Create `config.rs` with PodmanConfig
3. Create `types.rs` with domain types
4. Move runtime to `runtime.rs`
5. Split tests

### Phase 3: postgres/records.rs (1448 lines)
1. Create individual record modules
2. Create `helpers.rs` for shared functions
3. Move tests to appropriate modules

### Phase 4: twerk-app/engine/mod.rs (1379 lines)
1. Create `engine/types.rs`
2. Create `engine/errors.rs`
3. Create `engine/config.rs`
4. Move implementation to `engine/impl.rs`
5. Move tests to `engine/tests.rs`

---

## Verification Steps

After each phase:
1. `cargo check` - Verify compilation
2. `cargo test` - Verify tests pass
3. Check line counts of new files (must be < 300 lines)
4. Verify public API is unchanged (backward compatibility)

---

## Notes

- All splits maintain backward compatibility - public exports unchanged
- Tests are preserved and moved with their respective code
- No business logic changes, only structural refactoring
- Each new module follows single responsibility principle
