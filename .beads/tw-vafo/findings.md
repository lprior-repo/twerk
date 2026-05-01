# Findings: tw-vafo - Test API error handler logs request context

## Bead Status
- **Bead ID**: tw-vafo
- **Title**: twerk-web: Test API error handler logs request context for debugging
- **Closed by**: gecko (Completed-by-gecko)
- **Priority**: 1

## Task Requirements
1. trigger 500 error, assert log contains request_id, method, path
2. trigger 404, assert log at WARN level
3. trigger 500, assert log at ERROR level with stack trace

## Code Analysis

### Error Handler Implementation (`crates/twerk-web/src/api/error/core.rs`)

The `ApiErrorWithContext::into_response` method correctly logs with request context:

```rust
// Lines 165-171 - NotFound logs at WARN with context
ApiError::NotFound(ref msg) => {
    if let Some(ref ctx) = self.context {
        tracing::warn!(request_id = %ctx.request_id, method = %ctx.method, path = %ctx.path, error = %msg, "not found");
    }
    StatusCode::NOT_FOUND
}

// Lines 173-180 - Internal logs at ERROR with context
ApiError::Internal(ref msg) => {
    if let Some(ref ctx) = self.context {
        tracing::error!(request_id = %ctx.request_id, method = %ctx.method, path = %ctx.path, error = %msg, "internal server error");
    }
    StatusCode::INTERNAL_SERVER_ERROR
}
```

### Existing Tests

**api_error_logging_test.rs** - Tests response behavior only:
- `not_found_returns_404_with_problem_details` - verifies 404 response
- `invalid_json_returns_400_with_problem_details` - verifies 400 response
- `internal_error_returns_500_with_problem_details` - verifies 500 response
- `internal_error_response_does_not_leak_stack_trace` - verifies no stack in response

**error_logging_test.rs** - Tests response content:
- `error_response_contains_problem_json_content_type`
- `trigger_404_returns_not_found`
- `internal_error_does_not_leak_sensitive_details`

### Gap Analysis

**The existing tests do NOT verify log output.** They only test HTTP response behavior.

Tests that would be needed but don't exist:
1. Verification that logs contain `request_id`, `method`, `path` fields
2. Verification that 404 logs appear at WARN level
3. Verification that 500 logs appear at ERROR level
4. Verification that stack trace appears in logs (not in response) for 500

## Conclusion

The logging code exists and appears correct in `core.rs`. The missing piece is **log output verification tests** - the existing tests only verify response structure, not the actual log emissions.

The bead was marked completed by gecko, but the log verification tests described in the requirements do not appear to exist in the test files. This may be a QA verification gap or the tests may have been run manually without being codified.

## Recommendations

1. Add tracing subscriber tests using `tracing_subscriber` with a `Vec` layer to capture logs
2. Verify log events contain expected fields (request_id, method, path)
3. Verify log levels match error severity (WARN for 404, ERROR for 500)
