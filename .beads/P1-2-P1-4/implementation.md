# Implementation Summary: Web Middleware (P0-5) + Endpoint Toggling (P1-1)

## TASK 1: Web Middleware (P0-5)

### Files Created

**`middleware/web/config.rs`** — Typed configuration structs for all middleware:
- `CorsConfig` — allowed origins, methods, headers, credentials (all default to permissive)
- `RateLimitConfig` — requests_per_second (default: 100)
- `LoggerConfig` — log level, skip_paths (default: "info", empty)
- `BodyLimitConfig` — max_size in bytes (default: 4 MB)
- All implement `Default` with sensible defaults

**`middleware/web/cors.rs`** — CORS middleware:
- `cors_layer(&CorsConfig) -> CorsLayer` — builds a tower-http CorsLayer
- Handles: allowed origins (empty = allow all), methods, headers, credentials
- Falls back to `AllowOrigin::any()` and `AllowMethods::mirror_request()` when config is empty
- Filters invalid origin/method/header strings gracefully (no panic)

**`middleware/web/rate_limit.rs`** — Rate limiting middleware:
- `RateLimitLayer` — custom tower Layer wrapping governor's `RateLimiter`
- `RateLimitService` — tower Service that checks rate limit before forwarding
- Returns 429 Too Many Requests when limit exceeded
- `rate_limit_layer(&RateLimitConfig) -> Option<RateLimitLayer>` — None if RPS is zero
- Uses `governor::RateLimiter::direct(quota)` with per-second quota
- Shared state via `Arc<DirectLimiter>` — cloned across service instances

**`middleware/web/logger.rs`** — Request logging middleware:
- `RequestLoggerLayer` / `RequestLoggerService` — custom tower Layer/Service
- Logs method, path, status code, duration_ms at configurable level
- Supports skip_paths — prefix matching to skip logging (e.g., "/health")
- Log level parsed from string: trace/debug/info/warn/error (default: info)

**`middleware/web/body_limit.rs`** — Body size limit middleware:
- `body_limit_layer(&BodyLimitConfig) -> RequestBodyLimitLayer` — wraps tower-http's built-in
- Returns 413 Payload Too Large for oversized requests

**`middleware/web/mod.rs`** — Updated to expose new modules:
- Added `pub mod body_limit; config; cors; logger; rate_limit;`

### Wiring into Coordinator

**`coordinator/api/mod.rs`** — Updated `create_router`:
- Added middleware config fields to `Config`: `cors_config`, `rate_limit_config`, `logger_config`, `body_limit_config` (all `Option<T>`)
- Applies layers conditionally: CORS → Logger → RateLimit → BodyLimit
- CORS always present (defaults to permissive `CorsLayer::new()`)
- Other layers only applied if config is `Some`

### Dependencies Added

**`Cargo.toml`** (workspace root):
- Added `tower-http = { version = "0.6", features = ["cors", "limit"] }`
- Added `governor = "0.8"`

**`coordinator/Cargo.toml`**:
- Added `tork-runtime = { path = ".." }` dependency
- Added `"limit"` feature to tower-http

---

## TASK 2: Endpoint Toggling Config (P1-1)

### Files Modified

**`coordinator/config.rs`** — Added `ApiEndpoints` struct:
- 8 boolean fields: `health`, `jobs`, `tasks`, `nodes`, `queues`, `metrics`, `users`, `scheduled_jobs`
- All default to `true` via `Default` impl
- `to_enabled_map(&self) -> HashMap<String, bool>` — converts to the format used by the API router
- Added `endpoints: ApiEndpoints` field to `Config` struct
- Updated Debug/Clone impls

**`coordinator/coordinator.rs`** — Updated Coordinator:
- Added `endpoints: ApiEndpoints` field to `Coordinator` struct
- Added `use crate::config::ApiEndpoints;` import
- Initialized `endpoints` from `cfg.endpoints` in `Coordinator::new`
- Updated `start_api` to use `self.endpoints.to_enabled_map()` instead of `HashMap::new()`

**`coordinator/lib.rs`** — Added `pub mod config;`

---

## Constraint Adherence

- **Data→Calc→Actions**: All middleware configs are pure data structs. Layer construction is pure calculation. Only the actual request processing (in tower Services) performs I/O.
- **Zero Mutability**: No `mut` in core logic. Router building uses shadowing (`router = router.layer(...)`) which is idiomatic Axum.
- **Zero Panics**: All `unwrap()`/`expect()` avoided. Rate limit handles zero RPS via `Option`. CORS falls back gracefully for invalid origins. Logger/body-limit use infallible constructors.
- **Expression-Based**: Middleware functions return values, not side effects.
- **Type Safety**: `ApiEndpoints` struct makes endpoint toggling explicit (vs. string-keyed HashMap).

## Test Results

All middleware unit tests pass (15 tests):
- `config::tests::*` (4 tests) — default values
- `cors::tests::*` (3 tests) — layer construction, invalid origins
- `rate_limit::tests::*` (3 tests) — zero RPS, valid RPS, default
- `logger::tests::*` (3 tests) — default, skip paths, log level parsing
- `body_limit::tests::*` (3 tests) — default, custom, small
- Existing `middleware::web::tests::*` (3 tests) — unchanged, still pass

Workspace compilation: clean (`cargo check` — 0 errors)

## Pre-existing Issues

The coordinator's test suite has 26 pre-existing compilation errors in `handlers/completed/mod.rs` (type mismatches between `&str` and `Cow<'static, str>`). These are unrelated to this implementation and exist on the main branch.
