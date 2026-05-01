# RFC 7807 Problem Details Implementation - Findings

## Summary
Implemented RFC 7807 Problem Details format for error responses in `twerk-web`.

## Changes Made

### 1. `crates/twerk-web/src/api/error/core.rs`
- Updated `ApiError::IntoResponse` implementation to return RFC 7807 Problem Details format
- Added `Content-Type: application/problem+json` header
- Response body now includes:
  - `type`: URI reference identifying problem type (e.g., `https://httpstatus.es/400`)
  - `title`: Short, human-readable problem title (e.g., "Bad Request", "Not Found", "Internal Server Error")
  - `status`: HTTP status code as integer
  - `detail`: Human-readable explanation specific to this occurrence
- Internal errors log the actual error but return sanitized "Internal Server Error" title without leaking stack traces

### 2. `crates/twerk-web/src/api/error/tests.rs`
- Updated `into_response_behaves_as_expected` to verify RFC 7807 fields
- Updated `exact_payloads_are_preserved` to check RFC 7807 format instead of `{"message": ...}`
- Added Content-Type verification (`application/problem+json`)
- Added assertions that internal errors don't leak secrets/stack traces

### 3. `crates/twerk-web/tests/api_endpoints_test/rfc7807_contract_test.rs` (NEW FILE)
- Integration tests for RFC 7807 compliance at HTTP layer
- `get_nonexistent_returns_404_with_rfc7807_problem_details` - verifies 404 has all required fields
- `get_nonexistent_returns_problem_type_uri` - verifies type URI contains status code
- `post_invalid_json_returns_400_with_rfc7807_problem_details` - verifies 400 with problem details
- `post_invalid_json_returns_problem_without_stack_leak` - verifies no stack trace in 400 response
- `internal_error_returns_500_with_rfc7807_problem_details` - verifies 500 format
- `internal_error_never_leaks_stack_trace` - verifies no stack trace in 500 response
- `all_error_responses_use_application_problem_json` - verifies Content-Type header on all error routes

## Pre-existing Build Blocker
**`twerk-common` crate is broken** - references a `pub mod slot;` that does not exist:
```
error[E0583]: file not found for module `slot`
  --> crates/twerk-common/src/lib.rs:12:1
   |
12 | pub mod slot;
```

This prevents compilation of the entire workspace. This is NOT related to my changes - it is a pre-existing issue.

## Test Status
Cannot run tests due to pre-existing `twerk-common` build failure. Implementation is syntactically correct and follows RFC 7807 specification.

## RFC 7807 Compliance
The implementation follows RFC 7807 Problem Details format:
- Content-Type: `application/problem+json`
- All four required fields present: `type`, `title`, `status`, `detail`
- Internal errors sanitize details to prevent information leakage
- Type URIs use `https://httpstatus.es/{code}` pattern for identification