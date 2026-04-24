# Red Queen Findings: middleware.rs

## Target
`crates/twerk-app/src/engine/coordinator/middleware.rs` (143 lines)
Plus dependent files: `limits.rs` (127 lines), `auth.rs` (187 lines), `utils.rs` (102 lines)

## Execution Summary
- **Generations**: 4
- **Lineage**: 25 checks
- **Survivors**: 20
- **Beads Filed**: 7
- **Crown**: FORFEIT (2 CRITICAL survivors)

## CRITICAL Findings

### [GEN-2-4] Rate Limit Middleware Is Ineffective
**File**: `limits.rs:33-44`
**Issue**: `RateLimiter::direct(Quota::per_second(rps))` is created inside the middleware function body on every request. Since `RateLimiter::direct` uses an in-memory counter, each request gets a fresh limiter with a full quota. The rate limit is never actually enforced — every request passes.
**Fix**: The `RateLimiter` must be created once and stored in the `RateLimitConfig` state, shared across requests via `Arc`.
**Bead**: ve-m5ks2 (P0)

### [GEN-3-7] CORS allow_origin(Any) + allow_credentials Conflict
**File**: `middleware.rs:22-32`
**Issue**: `cors_layer()` sets `allow_origin(Any)` and conditionally `allow_credentials(true)`. Per the CORS specification, browsers reject `Access-Control-Allow-Origin: *` when `Access-Control-Allow-Credentials: true` is set. When credentials are enabled, the origin must be explicitly listed, not a wildcard.
**Fix**: When credentials are true, use a dynamic origin check instead of `Any`.
**Bead**: tw-5syl (P0)

## MAJOR Findings

### [GEN-1-1] Dead Code: create_web_middlewares()
**File**: `middleware.rs:115-143`
**Issue**: `create_web_middlewares()` is `pub` but never called anywhere in the codebase. `router.rs` has its own ad-hoc middleware setup. This function is dead code.
**Fix**: Either remove it or refactor `router.rs` to use it.
**Bead**: tw-90dc (P1)

### [GEN-1-2] Zero Test Coverage for Middleware Modules
**Files**: `middleware.rs`, `limits.rs`, `auth.rs`
**Issue**: None of the three middleware files contain `#[cfg(test)]` modules. The only tests in the coordinator directory are in `utils.rs`. 457 lines of middleware code have zero dedicated tests.
**Fix**: Add unit tests for `cors_layer()`, `http_log_middleware`, `rate_limit_middleware`, `body_limit_middleware`, `basic_auth_middleware`, `key_auth_middleware`.
**Bead**: tw-elib (P1)

### [GEN-2-3] Rate Limiter Per-Request Creation
**File**: `limits.rs:39`
**Issue**: `RateLimiter::direct()` called per-request rather than shared via state.
**Bead**: ve-by01a (P1)

### [GEN-3-5] CORS allow_origin(Any) Wildcard
**File**: `middleware.rs:27`
**Issue**: `allow_origin(Any)` permits any origin. While configurable, the default is overly permissive for production.
**Bead**: tw-jcok (P1)

## MINOR Findings

### [GEN-3-8] X-Forwarded-For Header Spoofing
**File**: `middleware.rs:70-76`
**Issue**: The middleware trusts `X-Forwarded-For` header directly for IP extraction. This header is client-controlled and trivially spoofable. The logged IP may not reflect the actual client.
**Fix**: Prefer `ConnectInfo<SocketAddr>` from axum when behind a trusted reverse proxy.

## OBSERVATION Findings

### [GEN-3-6] Key Auth Uses Constant-Time Comparison (Good)
**File**: `auth.rs:146`
**Issue**: `ct_eq` from `subtle` crate is used for API key comparison — this is correct and prevents timing attacks. Positive finding.
**Bead**: tw-asp1 (P3)

## Positive Findings
- No `unwrap()`, `expect()`, or `panic!()` in middleware files (deny lints enforced)
- No `todo!()` or `unimplemented!()` macros
- Key auth uses `subtle::ConstantTimeEq` for timing-safe comparison
- `parse_body_limit` uses `checked_mul` to prevent overflow
- `body_limit_middleware` rejects chunked transfer encoding
- All 59 existing crate tests pass

## Automated Weapon Results

### Quality Gates
- PASS: Tests (59 passed)
- FAIL: No Panic (project-wide `expect` in twerk-core, not target)
- FAIL: Format (project-wide, not target file)
- FAIL: Lint (project-wide, not target file)
- SKIP: Coverage (tarpaulin not installed)

### Fowler Review
- PASS: No `.unwrap()` in target files
- PASS: No `.expect()` in target files
- PASS: No `todo!()`/`unimplemented!()` in target files
- FAIL: Test coverage below 80% (no tests in middleware files)
- FAIL: Security vulnerabilities (project-wide `cargo audit`)
- FAIL: License issues (project-wide `cargo deny`)

## Dimension Fitness (Final)

| Dimension | Fitness | Status |
|-----------|---------|--------|
| dead-code | 0.5 | EXHAUSTED |
| test-coverage | 0.5 | EXHAUSTED |
| cors-configuration | 0 | EXHAUSTED |
| http-log-behavior | 0 | EXHAUSTED |
| client-ip-extraction | 0 | EXHAUSTED |
| error-handling | 0 | EXHAUSTED |
| rate-limit-design | 1.0 | DORMANT |
| rate-limit-ineffective | 1.0 | DORMANT |
| cors-wildcard | 1.0 | HEMORRHAGING |
| cors-credentials-conflict | 1.0 | HEMORRHAGING |

## Verdict: CROWN FORFEIT

The codebase has 2 CRITICAL vulnerabilities (ineffective rate limiting, CORS credentials conflict) that must be addressed before this module can be considered production-ready.
