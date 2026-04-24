# Findings: tw-r8yl - QA-MANUAL: exploratory 3

## Summary

Exploratory QA audit of the twerk codebase at `/home/lewis/gt/twerk/polecats/radrat/twerk/`.

**Build Status**: BUILD SUCCESSFUL
**Test Status**: 4608 tests PASSED (0 failures)
**Clippy**: 0 errors, 2 warnings (missing `# Errors` docs sections)

## Pre-existing Issues (from prior audit)

The following issues were documented in a prior audit (tw-0kin) and **still exist** in the codebase:

### Issue 1: queue_delete ignores API response body (MEDIUM)
**File**: `crates/twerk-cli/src/handlers/queue.rs:100-126`
**Status**: CONFIRMED - Issue still present

```rust
pub async fn queue_delete(...) -> Result<String, CliError> {
    // ...
    Ok(format!(r#"{{"deleted":true,"name":"{}"}}"#, name))  // HARDCODED
}
```
If server returns a different body (e.g., `{"success": true}`), it's ignored. The function always returns hardcoded JSON regardless of actual server response.

### Issue 2: Redundant success check in trigger_delete (LOW)
**File**: `crates/twerk-cli/src/handlers/trigger.rs:311`
**Status**: CONFIRMED - Issue still present

```rust
if status == reqwest::StatusCode::NO_CONTENT || status.is_success() {
```
NO_CONTENT (204) is already a success status, so `|| status.is_success()` is redundant.

### Issue 3: Inconsistent error handling patterns (LOW)
**Status**: CONFIRMED - Issue still present

| Handler | NOT_FOUND handling |
|---------|-------------------|
| `trigger_list` (line 45-59) | Returns `ApiError` |
| `queue_list` (line 24-29) | Returns `HttpStatus` |
| `trigger_get` (line 105-112) | Returns `ApiError` or `NotFound` |
| `queue_get` (line 69-71) | Returns `NotFound` |

Inconsistent error types make it harder for callers to handle errors uniformly.

### Issue 4: Missing validation for required fields (MEDIUM)
**File**: `crates/twerk-cli/src/handlers/queue.rs:10-15`
**Status**: CONFIRMED - Issue still present

```rust
#[derive(Debug, Deserialize)]
pub struct QueueInfo {
    pub name: String,  // NOT Option<String>, no #[serde(default)]
    pub size: i32,
    pub subscribers: i32,
    pub unacked: i32,
}
```
If API response is `{}` or missing `name`, `serde_json::from_str` returns error. Compare to `TaskResponse` which uses `Option<String>` for all fields - more defensive.

### Issue 5: trigger_create only handles CREATED (201) explicitly (LOW)
**File**: `crates/twerk-cli/src/handlers/trigger.rs:191-200`
**Status**: CONFIRMED - Still present

If API returns 200 OK (instead of 201 Created), the response still gets printed but falls through to line 202 and returns `Ok(body)`. Not a bug but inconsistent with how other handlers check for specific success codes.

### Issue 6: No timeout on HTTP requests (MEDIUM)
**Status**: CONFIRMED - Issue still present

All HTTP handlers use `reqwest::get()` or `client.get().send()` without explicit timeouts:

- `task.rs:61`: `reqwest::get(&url)` - no timeout
- `queue.rs:20, 65`: `reqwest::get(&url)` - no timeout  
- `trigger.rs:41, 96`: `reqwest::get(&url)` - no timeout

If the API is unresponsive, the CLI will hang indefinitely.

## New Issues Found

None - the codebase is well-structured with proper state machines, error handling (within the core crate), and test coverage.

## Code Quality Assessment

### Strengths
1. **State machines are well-designed** - `TaskState` and `JobState` have proper transition validation
2. **Good test coverage** - 4608 tests passing
3. **Clean error types** - Core crate uses proper error types with `thiserror`
4. **Proper use of serde attributes** - Most structs use `#[serde(skip_serializing_if = "Option::is_none")]`
5. **Clippy clean** - Only 2 minor documentation warnings

### Weaknesses (CLI handlers only)
1. Inconsistent error handling across handlers
2. No HTTP timeouts
3. Hardcoded response bodies in delete operations
4. Missing defensive deserialization

## Recommendation

The core business logic (twerk-core, twerk-infrastructure, twerk-web) is well-written and tested. Issues are confined to the CLI handlers (twerk-cli/src/handlers/). Consider:

1. Add HTTP timeout configuration to all CLI handlers
2. Standardize error handling across all handlers
3. Use `#[serde(default)]` on QueueInfo fields for defensive deserialization
4. Parse actual server response in queue_delete instead of returning hardcoded JSON