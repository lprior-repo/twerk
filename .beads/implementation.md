# Implementation Summary: Two Missing Features

## TASK 1: User-Based Job Filtering (P0-3)

### Problem
`list_jobs_handler` and `list_scheduled_jobs_handler` passed `String::new()` as the `current_user` parameter to datastore queries, meaning all jobs were returned regardless of user permissions.

### Solution

#### Files Modified

1. **`tork/user.rs`** — Added `UsernameValue` newtype:
   ```rust
   #[derive(Clone, Debug)]
   pub struct UsernameValue(pub String);
   ```
   This type is stored in request extensions by auth middleware and extracted by handlers.

2. **`tork/mod.rs`** — Re-exported `UsernameValue`:
   ```rust
   pub use user::{User, UsernameValue};
   ```

3. **`engine/coordinator.rs`** — Changed private `UsernameValue` struct to a public re-export:
   ```rust
   pub use tork::user::UsernameValue;
   ```
   The basic auth middleware already inserts `UsernameValue(username)` into request extensions.

4. **`coordinator/api/handlers.rs`** — Three changes:
   - Added `extract_current_user()` helper that reads `UsernameValue` from request extensions
   - Updated `list_jobs_handler` to accept `req: Request` and extract the current user
   - Updated `list_scheduled_jobs_handler` to accept `req: Request` and extract the current user

### Constraint Adherence
- **Zero panics/unwraps**: `extensions().get::<UsernameValue>()` returns `Option`, handled via `.map().unwrap_or_default()`
- **No mutability**: Pure extraction functions, no mutable state
- **Expression-based**: `extract_current_user` is a single expression chain
- **Make illegal states unrepresentable**: `UsernameValue` newtype prevents string confusion

---

## TASK 2: Wait Mode for POST /jobs (P0-2)

### Problem
Go's `POST /jobs` supports `wait=true` query parameter. When set, the API subscribes to broker events and blocks until the job reaches a terminal state (COMPLETED, FAILED, CANCELLED) or timeout, then returns the full job.

### Solution

#### Files Modified

**`coordinator/api/handlers.rs`** — Four additions:

1. **`CreateJobQuery` struct** — Query parameter for wait mode:
   ```rust
   #[derive(Debug, Clone, Deserialize, Default)]
   pub struct CreateJobQuery {
       pub wait: Option<bool>,
   }
   ```

2. **`is_terminal_state()` function** — Pure predicate for terminal states:
   ```rust
   fn is_terminal_state(state: &str) -> bool {
       matches!(state, JOB_STATE_COMPLETED | JOB_STATE_FAILED | JOB_STATE_CANCELLED)
   }
   ```

3. **`create_job_handler` updated** — Checks `wait` query parameter:
   ```rust
   if cq.wait == Some(true) {
       let job_id = job.id.clone().unwrap_or_default();
       return wait_for_terminal_state(&state, job_id).await;
   }
   ```

4. **`wait_for_terminal_state()` function** — Core wait logic:
   - Creates a `tokio::sync::oneshot` channel wrapped in `Arc<Mutex<Option>>` for single-use sending
   - Subscribes to `"job.*"` broker events via `subscribe_for_events`
   - Handler checks each event: matches job ID and terminal state
   - Uses `tokio::time::timeout` with 60-second limit
   - On terminal event: returns full job JSON
   - On timeout: fetches current job from datastore and returns it
   - On channel error: returns internal error

### Key Design Decisions

- **`Arc<Mutex<Option<Sender>>>` pattern**: The broker handler is `Fn` (called multiple times), but `oneshot::Sender::send` consumes the sender. `Option::take()` ensures only the first terminal event sends the result.
- **Timeout fallback**: On timeout, the job's current state is fetched from the datastore rather than returning an error, matching Go's behavior of always returning the job.
- **`TOPIC_JOB` constant**: Already existed as `"job.*"` with `#[allow(dead_code)]` — removed the allow attribute since it's now used.

### Constraint Adherence
- **Zero panics/unwraps**: All `Option`/`Result` handled via `match`, `ok()`, `and_then()`, `.take()`
- **No mutability in core logic**: Mutex only wraps the single-use sender, state transitions are value-based
- **Expression-based**: Match arms return responses directly
- **Error handling**: All broker/datastore errors mapped to `ApiError`
- **Functional composition**: Event filtering is a pure predicate chain

---

## Additional Fixes

**`coordinator/api/mod.rs`** — Fixed pre-existing broken imports:
- Removed references to `tork::middleware::*` and `tork_runtime::middleware::*` modules that don't exist
- Removed unused `ServiceBuilder` import
- Simplified `create_router` to use `CorsLayer::new()` directly
- Removed middleware config fields (`cors_config`, `rate_limit_config`, etc.) that referenced non-existent types

These were pre-existing compilation errors unrelated to the two features.

## Build Verification

```bash
cargo check -p coordinator     # SUCCESS - compiles cleanly
cargo check -p tork            # SUCCESS
cargo check -p tork-engine     # SUCCESS
cargo test -p tork             # 39 passed
cargo test -p tork-engine      # 103 passed
```

Coordinator tests (`cargo test -p coordinator`) fail to compile due to pre-existing errors in other modules (`completed/mod.rs` type mismatches, `config.rs` unused imports). These are unrelated to the implemented features.
