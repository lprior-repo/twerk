# Findings: tw-wdyd - Test API error handler logs request context

## Summary

Wrote tests to verify the API error handler logs request context for debugging purposes as specified in bead tw-wdyd.

## Tests Written

Added 4 new tests to `crates/twerk-web/src/api/error/tests.rs`:

1. **`error_handler_logs_500_with_request_context`** - Verifies 500 errors log at ERROR level with error message
2. **`error_handler_logs_404_at_warn_level`** - Verifies 404 errors log at WARN level
3. **`error_handler_logs_500_at_error_level_with_context`** - Verifies 500 errors log at ERROR level with stack trace
4. **`error_logs_contain_method_path_for_500_errors`** - Verifies logs contain HTTP method and path

## Current Behavior vs. Expected Behavior

### 404 Errors (NotFound)
- **Current**: No logging occurs for 404 errors
- **Expected**: Should log at WARN level
- **Test Status**: FAILING - log is empty `[]`

### 500 Errors (Internal)
- **Current**: Logs `ERROR twerk_web::api::error::core: internal server error error=<message>`
- **Expected**: Should include request_id, method, path in logs
- **Test Status**: PARTIALLY FAILING - logs ERROR level but missing request context

## Technical Analysis

The error handler in `core.rs` uses `IntoResponse` for `ApiError`:

```rust
Self::Internal(ref msg) => {
    error!(error = %msg, "internal server error");
    StatusCode::INTERNAL_SERVER_ERROR
}
```

**Issue**: The `IntoResponse` trait implementation doesn't have access to request context (method, path, request_id). The error is created with just a message string.

### Options for Adding Request Context

1. **Custom error wrapper** - Create `ApiErrorWithContext` that includes request details
2. **Middleware approach** - Log request context in a layer before `IntoResponse` is called
3. **Span enrichment** - Use `tracing::Span::current()` but requires handlers to record method/path in span

## Bug Fixes Also Made

- Fixed `extract_response_body()` helper to properly extract headers before consuming body
- Fixed test assertion that was checking for "secret" which was part of the test message itself

## Recommendations

To make the tests pass, the error handler needs to be enhanced to:

1. Log 404 errors at WARN level with resource info
2. Include request_id, method, and path in 500 error logs

This would require either:
- Modifying `ApiError` to carry request context when created
- Adding a middleware/logging layer that captures request context before error conversion

## Files Modified

- `crates/twerk-web/src/api/error/tests.rs` - Added 4 new logging tests
- `crates/twerk-web/src/api/error/tests.rs` - Fixed existing test helper and test assertions
- `crates/twerk-web/Cargo.toml` - Added `tracing-subscriber.workspace = true` to dev-dependencies
