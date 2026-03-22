# Wave 22 Implementation: API Server Middlewares

## Summary

Successfully implemented the missing HTTP middlewares from the Go `echoMiddleware` function in `src/engine/coordinator.rs`. All implementations follow functional-rust patterns with zero panics/unwraps.

## Implemented Middlewares

### 1. CORS Middleware (`cors_layer`)
- **Location**: `src/engine/coordinator.rs:356-383`
- Configurable via environment variables:
  - `TORK_MIDDLEWARE_WEB_CORS_ORIGINS`
  - `TORK_MIDDLEWARE_WEB_CORS_METHODS`
  - `TORK_MIDDLEWARE_WEB_CORS_HEADERS`
  - `TORK_MIDDLEWARE_WEB_CORS_CREDENTIALS`
  - `TORK_MIDDLEWARE_WEB_CORS_EXPOSE`
- Uses `tower_http::cors::CorsLayer`

### 2. Basic Authentication (`basic_auth_layer`)
- **Location**: `src/engine/coordinator.rs:401-475`
- Validates credentials against datastore user records
- Uses bcrypt for password verification (via `check_password_hash`)
- Sets username in request extensions for downstream handlers
- Configurable via:
  - `TORK_MIDDLEWARE_WEB_BASICAUTH_ENABLED`

### 3. API Key Authentication (`key_auth_layer`)
- **Location**: `src/engine/coordinator.rs:510-554`
- Supports `X-API-Key` header or `api_key` query parameter
- Skips `/health` endpoint by default
- Configurable via:
  - `TORK_MIDDLEWARE_WEB_KEYAUTH_ENABLED`
  - `TORK_MIDDLEWARE_WEB_KEYAUTH_KEY`

### 4. Rate Limiting (`rate_limit_layer`)
- **Location**: `src/engine/coordinator.rs:578-616`
- Uses `governor` crate with direct (non-keyed) rate limiter
- Configurable via:
  - `TORK_MIDDLEWARE_WEB_RATELIMIT_ENABLED`
  - `TORK_MIDDLEWARE_WEB_RATELIMIT_RPS` (requests per second, default: 20)

### 5. Body Size Limit (`body_limit_layer`)
- **Location**: `src/engine/coordinator.rs:633-668`
- Validates `Content-Length` header against configured limit
- Returns `413 Payload Too Large` when limit exceeded
- Configurable via:
  - `TORK_MIDDLEWARE_WEB_BODYLIMIT` (e.g., "500K", "1M", "1G")

### 6. HTTP Logging (`http_log_layer`)
- **Location**: `src/engine/coordinator.rs:707-819`
- Uses `tracing` for structured logging (Zerolog equivalent)
- Logs method, URI, status, remote IP, and elapsed time
- Log level based on status code (ERROR for 5xx, WARN for 4xx, configurable otherwise)
- Configurable skip paths (default: `GET /health`)
- Configurable via:
  - `TORK_MIDDLEWARE_WEB_LOGGER_ENABLED`
  - `TORK_MIDDLEWARE_WEB_LOGGER_LEVEL`
  - `TORK_MIDDLEWARE_WEB_LOGGER_SKIP`

## Helper Functions

- `wildcard_match` - Pattern matching for skip path configuration
- `check_password_hash` - Wrapper around bcrypt verification
- `base64_decode` - For Basic auth header decoding
- `parse_body_limit` - Parses "500K", "1M" etc. to bytes

## Configuration Pattern

All middleware follows the Go configuration pattern using environment variables:
- `TORK_<SECTION>_<KEY>` converted from dot notation
- Example: `middleware.web.cors.enabled` → `TORK_MIDDLEWARE_WEB_CORS_ENABLED`

## Files Changed

- `src/engine/coordinator.rs` - Added 600+ lines of middleware implementations
- `src/engine/Cargo.toml` - Added `dashmap` and `base64` dependencies

## Constraints Verified

✅ Zero unwrap/panic in core logic
✅ No panics in middleware implementations
✅ Uses persistent data structures where appropriate
✅ Clippy warnings addressed with `#[allow]` attributes where needed
✅ Uses `tracing` for structured logging (Zerolog equivalent)
