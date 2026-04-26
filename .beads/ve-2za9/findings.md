# Security Audit: crates/twerk-web

**Auditor**: pipboy (veloxide polecat)
**Date**: 2026-04-24
**Bead**: ve-2za9
**Scope**: Full security review of `twerk-web` HTTP API crate

## Summary

17 findings: 3 CRITICAL, 3 HIGH, 4 MEDIUM, 7 LOW

---

## CRITICAL

### C1: OpenAPI spec unconditionally exposed without auth gating
- **File**: `src/api/router.rs:65`
- **Detail**: `router.route("/openapi.json", get(serve_openapi_spec))` is always mounted regardless of auth middleware configuration. The full API schema (endpoint paths, request/response types, field names) is accessible to unauthenticated users.
- **Impact**: Complete API surface enumeration enabling targeted attacks.
- **Fix**: Gate OpenAPI endpoint behind auth middleware or make it configurable.

### C2: Default bind address `0.0.0.0:8000` exposes service to all interfaces
- **File**: `src/api/state.rs:28`
- **Detail**: `Config::default()` sets `address: "0.0.0.0:8000"`. Combined with auth being optional, a default deployment exposes the full API to all network interfaces.
- **Impact**: Unintended network exposure in default configurations.
- **Fix**: Default to `127.0.0.1:8000` for localhost-only binding.

### C3: In-memory trigger datastore `std::sync::Mutex` enables DoS via lock contention
- **File**: `src/api/trigger_api/datastore/state.rs:13-14`
- **Detail**: `Mutex<HashMap>` blocks all trigger operations globally. `update_trigger` (line 102-120) acquires the lock twice (clone + insert). Concurrent requests create severe contention.
- **Impact**: Denial of service through concurrent trigger API requests.
- **Fix**: Use `tokio::sync::RwLock` or `dashmap::DashMap` for concurrent access. Consider `RwLock` for read-heavy workloads.

---

## HIGH

### H1: `extract_current_user` silently defaults to empty string — authorization bypass potential
- **File**: `src/api/handlers/mod.rs:80-85`
- **Detail**: `unwrap_or_default()` returns `""` when no `UsernameValue` extension is present (auth middleware not configured). This empty string is passed to `ds.get_jobs()` and `ds.get_scheduled_jobs()` — if the datastore treats empty user as "return all", all data is visible without auth.
- **Impact**: Potential complete data exposure when auth is misconfigured or absent.
- **Fix**: Return 401 Unauthorized when auth is expected but no user extension is present, or make auth mandatory for list endpoints.

### H2: Password policy only enforces minimum 8 characters
- **File**: `src/api/domain/auth.rs:116-134`
- **Detail**: `Password::new()` only checks non-empty + minimum 8 characters. No requirements for uppercase, lowercase, digits, special characters, maximum length, or common-password checking.
- **Impact**: Weak user accounts susceptible to credential attacks.
- **Fix**: Add complexity requirements, maximum length (prevent bcrypt DoS with extremely long inputs), and common-password blacklist.

### H3: TOCTOU race condition in job cancel/restart — state check then update
- **File**: `src/api/handlers/jobs/mutation.rs:17-47` (cancel), `132-160` (restart)
- **Detail**: Both handlers read the job state, check validity, then update. Between read and update, concurrent requests can change state. The `update_job` closure unconditionally sets the new state without re-checking the precondition.
- **Impact**: Invalid state transitions (e.g., double-cancel, cancel-during-restart).
- **Fix**: Move state precondition check into the `update_job` closure for atomic conditional update, or use optimistic concurrency with version field (like triggers do).

---

## MEDIUM

### M1: Secret redaction uses naive string replacement — partial matches cause false positives/negatives
- **File**: `src/api/redact.rs:196-217`
- **Detail**: `val.replace(secret_val, REDACTED_STR)` replaces ALL occurrences. Short secret values (e.g., "a") could redact legitimate content. Partial substring matches could corrupt data while missing the full secret.
- **Impact**: False redaction or incomplete redaction of secrets in API responses.
- **Fix**: Use word-boundary-aware matching or only redact exact-value matches for known sensitive fields.

### M2: YAML parsing allows resource consumption within budget limits
- **File**: `src/api/yaml.rs:10-13`
- **Detail**: `MAX_YAML_BODY_SIZE` (512KB), `DEFAULT_MAX_DEPTH` (64), `DEFAULT_MAX_NODES` (10,000) are generous. Deeply nested YAML with alias references can still consume disproportionate CPU/memory before hitting budgets.
- **Impact**: Potential DoS via crafted YAML payloads.
- **Fix**: Reduce `DEFAULT_MAX_NODES` to 1,000 and `DEFAULT_MAX_DEPTH` to 32. Consider request timeout.

### M3: Search query parameter passed unvalidated to datastore
- **File**: `src/api/handlers/tasks.rs:113`, `src/api/handlers/jobs/read.rs:79`
- **Detail**: The `q` query parameter is an arbitrary-length string passed directly to datastore query methods (`get_task_log_parts`, `get_jobs`). No length limit or character sanitization.
- **Impact**: Potential query injection if datastore uses SQL/regex-based search. Unbounded query complexity.
- **Fix**: Enforce maximum length on search query (e.g., 256 chars). Sanitize or escape special characters.

### M4: `serialize_view` serializes twice — validation then response
- **File**: `src/api/trigger_api/handlers/response.rs:66-72`
- **Detail**: `serde_json::to_vec(&view)` checks serialization, then `axum::Json(view)` serializes again. The error message reveals internal serialization failure details.
- **Impact**: Minor performance waste. Error message could leak type information.
- **Fix**: Remove the pre-serialization check, or use the `to_vec` result directly.

---

## LOW

### L1: `Password` type exposes `AsRef<str>` — allows plaintext access
- **File**: `src/api/domain/auth.rs:149-153`
- **Detail**: `AsRef<str>` returns raw password. While `Display` properly redacts, `AsRef` bypasses this protection.
- **Fix**: Remove `AsRef<str>` impl. Use dedicated method like `verify()` for comparison.

### L2: `logging_middleware` logs full URIs including query parameters
- **File**: `src/middleware/mod.rs:21-31`
- **Detail**: Logs complete URI which may contain sensitive query parameters (search terms, pagination tokens).
- **Fix**: Strip query parameters before logging, or log only the path component.

### L3: `Password` struct derives `Serialize` with `#[serde(transparent)]`
- **File**: `src/api/domain/auth.rs:104-106`
- **Detail**: If accidentally included in a response struct, password plaintext will serialize directly.
- **Fix**: Remove `Serialize` derive or use custom serializer that always outputs `[REDACTED]`.

### L4: `health_handler` leaks software version
- **File**: `src/api/handlers/system.rs:33,37`
- **Detail**: Both UP and DOWN responses include `VERSION` from `CARGO_PKG_VERSION`.
- **Impact**: Helps attackers identify vulnerable versions.
- **Fix**: Make version in health response configurable, default off.

### L5: Trigger metadata values have no length limit
- **File**: `src/api/trigger_api/domain.rs:66-76`
- **Detail**: `metadata: Option<HashMap<String, String>>` — keys validated for non-empty ASCII but values are unbounded. No limit on number of metadata entries.
- **Fix**: Enforce maximum value length (e.g., 1024 chars) and maximum entry count (e.g., 50).

### L6: `create_read_task_middleware` always passes empty secrets
- **File**: `src/middleware/hooks.rs:185`
- **Detail**: `let secrets = HashMap::new();` — the middleware never has secrets to redact. However, current handlers correctly redact inline (not via middleware), so this is a latent bug, not an active vulnerability.
- **Fix**: Wire the actual job secrets into the middleware, or remove the unused middleware.

### L7: JSON job creation has no body size check — relies on optional middleware
- **File**: `src/api/handlers/jobs/create.rs:19-27`
- **Detail**: YAML path has `validate_yaml_input` with size limits, but JSON path (`serde_json::from_slice`) has no size check. Body limit depends on `body_limit_middleware` being configured.
- **Fix**: Add explicit body size check for JSON requests matching the YAML limit, or make body limit middleware mandatory.

---

## Positive Security Observations

1. **Error masking**: `ApiError::Internal` correctly masks internal error details in responses (`INTERNAL_ERROR_MSG` constant)
2. **Bcrypt hashing**: User passwords hashed with `bcrypt::DEFAULT_COST` (12 rounds)
3. **YAML safety**: `serde_saphyr` with budget limits (depth + nodes) and duplicate key detection
4. **Input validation**: Domain newtypes (`Username`, `Password`, `TriggerId`) enforce parse-don't-validate
5. **Clippy enforcement**: `#![deny(clippy::unwrap_used)]` prevents panics in API module
6. **Secret redaction**: Jobs, tasks, and logs are redacted before API response
7. **Trigger versioning**: Optimistic concurrency control prevents lost updates
8. **No unsafe code**: `yaml.rs` has `#![forbid(unsafe_code)]`

---

## Audit Scope

Files reviewed:
- `Cargo.toml` — Dependencies
- `src/lib.rs` — Module structure
- `src/api/router.rs` — Route definitions and middleware ordering
- `src/api/mod.rs`, `state.rs`, `types.rs` — API infrastructure
- `src/api/error/core.rs`, `conversions.rs` — Error handling
- `src/api/handlers/mod.rs` — Shared handler utilities
- `src/api/handlers/system.rs` — Health, nodes, metrics, user creation
- `src/api/handlers/tasks.rs` — Task read endpoints
- `src/api/handlers/jobs/` — Job CRUD (create, read, mutation, types)
- `src/api/handlers/queues.rs` — Queue operations
- `src/api/handlers/scheduled/` — Scheduled job lifecycle
- `src/api/handlers/triggers.rs` — Trigger re-exports
- `src/api/domain/auth.rs` — Username/password validation
- `src/api/domain/api.rs` — Server address, content type, feature flags
- `src/api/domain/search.rs` — Search query type
- `src/api/trigger_api/domain.rs` — Trigger domain model and validation
- `src/api/trigger_api/datastore/state.rs` — In-memory trigger store
- `src/api/trigger_api/handlers/` — Trigger handler parsing, command, query, response
- `src/api/redact.rs` — Secret redaction logic
- `src/api/yaml.rs` — YAML parsing with safety limits
- `src/api/content_type.rs` — Content type classification
- `src/middleware/mod.rs`, `hooks.rs` — Middleware layer
- `src/helpers.rs` — Test server helpers
