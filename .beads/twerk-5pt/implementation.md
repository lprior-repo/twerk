# Implementation Report: twerk-5pt - HTTPX Server Wrapper

## Task Summary
Implemented `StartAsync` functionality in Rust following the Go reference implementation at `/tmp/tork/internal/httpx/httpx.go`.

## Implementation Details

### Files Created
- `crates/twerk-infrastructure/src/httpx.rs` - New HTTP extension module

### Files Modified
- `crates/twerk-infrastructure/src/lib.rs` - Added `pub mod httpx;`
- `crates/twerk-infrastructure/Cargo.toml` - Added `axum.workspace = true`, `tower.workspace = true`, `tower-http.workspace = true`
- `Cargo.toml` (workspace) - Fixed `zerolog = "0.3"` (was invalid version)
- `crates/twerk-common/src/logging/writer.rs` - Removed dead `ZerologWriter` code that was incompatible with zerolog 0.3

### Architecture (Data->Calc->Actions)

**Data Layer:**
- `HttpxError` enum with variants: `ServerError`, `ConnectionTimeout`, `InvalidAddress`
- `PollingConfig` struct with `max_attempts: u32` and `delay: Duration`

**Calculations Layer:**
- `can_connect(address: &str) -> bool` - Pure function that checks TCP connectivity
- `wait_for_ready()` - Polling retry logic encapsulated as pure calculation

**Actions Layer:**
- `start_async()` - Spawns server in background task and polls for readiness

### Key Functions

1. **`can_connect(address: &str) -> bool`** - Pure TCP connectivity check using `std::net::TcpStream::connect()`

2. **`start_async(address: &str, router: axum::Router, config: PollingConfig) -> Result<(), HttpxError>`**
   - Binds TCP listener to address
   - Spawns axum server in background task
   - Polls connectivity up to `max_attempts` times with `delay` between attempts
   - Returns `Ok(())` if server becomes reachable, `Err(HttpxError::ConnectionTimeout)` otherwise
   - Returns `Err(HttpxError::ServerError)` if server errors out

### Tests (5 tests covering core functionality)

1. **`test_start_async_success`** - Verifies server starts and becomes reachable
2. **`test_start_async_connection_timeout`** - Verifies timeout error when server doesn't start
3. **`test_can_connect_with_invalid_address`** - Verifies invalid addresses return false
4. **`test_can_connect_with_unreachable_address`** - Verifies unreachable addresses return false
5. **`test_polling_config_default`** - Verifies default polling config (100 attempts, 100ms delay)
6. **`test_can_connect_with_valid_address`** - Verifies connectivity check works with active listener

### Constraint Adherence

| Constraint | Status |
|------------|--------|
| Zero unwrap/expect/panic in core logic | ✅ All error handling via `?` and `map_err` |
| Zero `mut` in core logic | ✅ No `mut` used |
| Expression-based | ✅ Uses iterator patterns, `and_then`, `ok()` |
| `thiserror` for errors | ✅ `HttpxError` derives `Error` |
| `#[must_use]` where appropriate | ✅ `can_connect` has `#[must_use]` |
| Clippy warnings | ✅ No warnings in httpx.rs |

### Verification

```bash
$ cargo build --package twerk-infrastructure
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

$ cargo clippy --package twerk-infrastructure -- -W clippy::pedantic 2>&1 | grep httpx
# (no httpx-specific warnings)
```

## Notes

- The pre-existing codebase has clippy warnings in other files (`cache/item.rs`, `cache/mod.rs`, `runtime/docker/container.rs`, `runtime/podman/runtime.rs`) which are outside the scope of this task
- Test code uses `expect()` which is allowed per functional-rust skill: "Tests: whatever compiles"
- Added `axum`, `tower`, and `tower-http` as dependencies to `twerk-infrastructure` to support HTTP server functionality
