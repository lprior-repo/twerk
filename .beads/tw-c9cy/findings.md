# Findings: tw-c9cy - standalone: repair local quick-start job execution

## Issue Summary
When submitting `examples/hello-shell.yaml` to `POST /jobs?wait=true` in standalone mode, the job gets stuck in PENDING and the blocking request hangs indefinitely.

## Root Cause Analysis

### The Job Flow in Standalone Mode
1. POST /jobs creates job with state=Pending, publishes via `publish_job`
2. Coordinator's `subscribe_for_jobs` handler receives job via `handle_job_event`
3. `handle_job_event(Pending)` calls `start_job`
4. `start_job` transitions job to Scheduled, creates task, calls `handle_pending_task`
5. `Scheduler::schedule_task` publishes task to "default" queue via `publish_task`
6. Worker's `subscribe_for_tasks("default", handler)` receives task
7. Worker executes task via `execute_task`
8. Task completion triggers `handle_task_completed` -> `handle_top_level_task_completed`
9. When all tasks done, `broker.publish_job(&completed_job)` is called
10. Coordinator receives job via `subscribe_for_jobs` -> `handle_job_event(Completed)`
11. `complete_job` calls `publish_event("job.completed", ...)` to typed channels
12. `wait_for_job_completion` receives event via `subscription.recv()`

### Potential Issues Identified

#### 1. `wildcard_match` Pattern Matching
The `wildcard_match("job.*", "job.completed")` uses DP algorithm. Analysis shows it should correctly return true, but edge cases around multi-star patterns may not work correctly.

#### 2. Handler Spawning Without Awaiting
In `publish::spawn_handler`, handlers are spawned with `tokio::spawn` but the function immediately returns. If a handler fails silently, the error is logged but execution continues.

#### 3. Broker Proxy Initialization Order
The `BrokerProxy` delegates to inner broker. If `subscribe_for_tasks` is called before the broker is initialized, it returns `BrokerNotInitialized` error. This could happen during startup race conditions.

#### 4. Shell Runtime Configuration
The config shows `runtime.type = "shell"` and `runtime.shell.cmd = ["bash", "-c"]`. If shell runtime has issues, tasks won't execute properly.

#### 5. Queue Filtering for Workers
Workers only subscribe to queues where `is_worker_queue` returns true. The "default" queue passes this check, but "x-pending" (QUEUE_PENDING) does not - this is intentional as coordinator handles x-pending queue.

## Recommendations

1. **Add logging/tracing** to verify coordinator job subscription is triggered
2. **Verify wildcard matching** works correctly for "job.*" pattern
3. **Check startup order** - ensure broker is initialized before worker/coordinator start subscriptions
4. **Add timeout fallback** in `wait_for_job_completion` to return error instead of hanging
5. **Test shell runtime** execution independently

## Files Analyzed
- `crates/twerk-cli/src/run.rs` - Standalone engine startup
- `crates/twerk-app/src/engine/engine_lifecycle.rs` - Standalone mode initialization
- `crates/twerk-app/src/engine/coordinator/mod.rs` - Coordinator job subscriptions
- `crates/twerk-app/src/engine/coordinator/handlers/job_handlers.rs` - Job event processing
- `crates/twerk-app/src/engine/coordinator/handlers/task_handlers.rs` - Task completion flow
- `crates/twerk-app/src/engine/worker/mod.rs` - Worker queue subscriptions
- `crates/twerk-infrastructure/src/broker/inmemory/` - In-memory broker implementation
- `crates/twerk-infrastructure/src/broker/inmemory/subscription.rs` - Subscription handling
- `crates/twerk-infrastructure/src/broker/inmemory/publish.rs` - Event publishing
- `crates/twerk-web/src/api/handlers/jobs/create.rs` - Job creation handler
- `crates/twerk-common/src/wildcard.rs` - Pattern matching

## Status
Analysis complete. Issue is complex - multiple components interact (API, coordinator, worker, broker). Need to add instrumentation to trace actual execution flow.