# Findings: tw-nv4 - Rate Limit Middleware Per-IP Testing

## Summary
Wrote integration tests for per-IP rate limiting middleware in `crates/twerk-web/tests/rate_limit_middleware_test.rs`.

## Bug Found: Rate Limiter Recreation Per Request

The current `rate_limit_middleware` implementation at `crates/twerk-app/src/engine/coordinator/limits.rs:33-45` has a critical bug:

```rust
pub async fn rate_limit_middleware(...) -> Result<Response, StatusCode> {
    let rps = NonZeroU32::new(config.rps.max(1)).unwrap_or(NonZeroU32::MIN);
    let limiter = RateLimiter::direct(Quota::per_second(rps));  // NEW LIMITER EVERY REQUEST!

    match limiter.check() {
        Ok(()) => Ok(next.run(request).await),
        Err(_not_until) => Err(StatusCode::TOO_MANY_REQUESTS),
    }
}
```

**Problem**: A NEW `RateLimiter::direct()` is created on EVERY request. Since rate limiters maintain state internally, and this creates a fresh one per request, NO rate limiting actually occurs. Each request starts with a fresh counter at zero.

## Bug Found: No Per-IP Tracking

The implementation claims to track per-IP (as stated in bead description) but uses `RateLimiter::direct()` which is a **global** rate limiter, not keyed by IP.

To support per-IP rate limiting, the implementation would need to:
1. Extract client IP from the request (via `axum::extract::ConnectInfo` or similar)
2. Use `RateLimiter::keyed()` instead of `RateLimiter::direct()`
3. Share a single keyed rate limiter across requests (not create one per request)

## Pre-existing Build Issue

The workspace has a pre-existing build failure in `twerk-common`:
```
error[E0583]: file not found for module `slot`
  --> crates/twerk-common/src/lib.rs:12:1
   |
12 | pub mod slot;
```

This prevents running any tests that depend on the workspace.

## Test File Written

Created `crates/twerk-web/tests/rate_limit_middleware_test.rs` with three tests:
1. `rate_limit_allows_requests_under_limit` - Verifies first 5 requests succeed
2. `rate_limit_blocks_excessive_requests` - Verifies 6th request gets 429
3. `rate_limit_per_ip_tracking_different_ip_succeeds` - Verifies different IPs are tracked separately

These tests would expose both bugs above but cannot be run due to the build issue.

## Recommendations

1. Fix the `rate_limit_middleware` to share a single `RateLimiter` instance across requests (use `Arc` or similar)
2. Implement per-IP tracking using `RateLimiter::keyed()` with IP extraction
3. Fix the missing `slot` module in `twerk-common` to enable testing