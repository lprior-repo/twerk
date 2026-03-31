# Twerk Testing Suite Design

## Problem Statement

When running `distributed_e2e_test` with `--test-threads=4`, tests fail with jobs stuck in `SCHEDULED` state. The root cause: all engine instances share the same RabbitMQ queues, causing message cross-talk between test instances.

This document describes the **testing strategy and suite design** to:
1. Verify the fix works correctly
2. Prevent regression of concurrent test isolation
3. Maintain backward compatibility for single-instance deployments

---

## Section 1 — Test Suite Philosophy

### 1.1 The Testing Trophy (Based on Gary Bloch's model)

```
                    ┌─────────────────────────────────────┐
                    │           E2E / Integration         │
                    │   (real broker + real datastore)    │
                    │         60% of test effort          │
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
                    │            Integration               │
                    │  (component interaction, mocks OK)   │
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
                    │              Unit Tests              │
                    │   (pure functions, isolated logic)   │
                    │         30% of test effort          │
                    └─────────────────────────────────────┘
                    ┌─────────────────────────────────────┐
                    │           Static Analysis           │
                    │  (clippy, types, compile-time)      │
                    │         10% of test effort          │
                    └─────────────────────────────────────┘
```

### 1.2 Test Quality Criteria (Holzmann's Rules adapted)

| # | Rule | Application |
|---|------|-------------|
| 1 | Test purpose documented | Every test has a doc comment explaining what it verifies |
| 2 | Test assumptions checked | Preconditions are validated before assertions |
| 3 | Tests implemented to fail | Tests fail when behavior is broken (not pass vacuously) |
| 4 | Test deterministic | No flaky tests; no random data that affects pass/fail |
| 5 | Test independence | Tests don't depend on execution order or shared state |
| 6 | Results inspectable | Failures show inputs, expected, actual |
| 7 | No `is_ok()` / `is_err()` | Assert exact values, not just success/failure |
| 8 | Mutation-aware | Tests designed to catch specific mutations |

### 1.3 No `is_ok()` / `is_err()` Rule

**BAD:**
```rust
broker.health_check().await?; // assumes success
assert!(result.is_ok()); // too weak
```

**GOOD:**
```rust
let result = broker.health_check().await?;
// On failure, ? propagates the error
// On success, we assert on actual behavior
let info = broker.queue_info("x-jobs".into()).await?;
assert_eq!(info.name, "x-jobs");
```

---

## Section 2 — Test File Organization

### 2.1 Directory Structure

```
crates/
├── twerk-core/
│   ├── src/
│   │   └── lib.rs
│   └── tests/                    # Core logic tests (validation, eval)
├── twerk-infrastructure/
│   ├── src/
│   │   ├── broker/
│   │   │   ├── mod.rs           # Queue constants, helpers, is_* functions
│   │   │   ├── rabbitmq.rs      # RabbitMQ implementation
│   │   │   └── tests.rs         # Unit tests (inline #[cfg(test)])
│   │   ├── datastore/
│   │   ├── worker/
│   │   └── tests/               # Integration tests
│   │       ├── rabbitmq_test.rs  # Existing broker tests
│   │       └── postgres_test.rs  # Existing datastore tests
├── twerk-app/
│   ├── src/
│   │   └── engine/
│   │       ├── coordinator/
│   │       ├── worker/
│   │       └── engine.rs
│   └── tests/
│       ├── engine_lifecycle_test.rs
│       ├── coordinator_test.rs
│       └── handlers_test.rs
└── twerk-web/
    ├── src/
    │   └── api/
    └── tests/
        ├── distributed_e2e_test.rs  # Main concurrent E2E tests
        └── api_test.rs
```

### 2.2 Test Naming Conventions

| Pattern | Purpose | Example |
|---------|---------|---------|
| `*_test.rs` | Unit/integration tests for module | `handlers_test.rs` |
| `*_e2e_test.rs` | End-to-end scenario tests | `distributed_e2e_test.rs` |
| `test_*.rs` (in `tests/`) | Exhaustive test files | `test_evaluator.rs` |
| `#[test]` | Individual test cases | `fn it_handles_empty_queue()` |
| `#[tokio::test]` | Async test cases | `async fn it_publishes_task()` |
| `mod tests { ... }` | Inline unit tests | inside `mod.rs` files |

### 2.3 Test Module Patterns

**Pattern A: Inline tests in source module (preferred for pure logic)**
```rust
// crates/twerk-infrastructure/src/broker/mod.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_coordinator_queue_returns_true_for_x_jobs() {
        assert!(is_coordinator_queue(queue::QUEUE_JOBS));
    }
}
```

**Pattern B: Separate integration test file (for tests needing real infra)**
```rust
// crates/twerk-infrastructure/tests/rabbitmq_test.rs

#[tokio::test]
async fn it_delivers_task_when_published_to_rabbitmq() { ... }
```

---

## Section 3 — Concurrent E2E Test Design

### 3.1 The DistributedEnv Pattern

Each test creates its own infrastructure stack:

```rust
struct DistributedEnv {
    _postgres: testcontainers::ContainerAsync<Postgres>,
    _rabbitmq: testcontainers::ContainerAsync<RabbitMq>,
    coordinator: Engine,
    worker: Engine,
    base_url: String,
    api_handle: tokio::task::JoinHandle<()>,
    client: reqwest::Client,
}

impl DistributedEnv {
    async fn new() -> anyhow::Result<Self> {
        // 1. Start fresh Postgres
        let postgres = Postgres::default().with_tag("16-alpine").start().await?;
        let dsn = format!("postgres://...{}", postgres.get_host_port_ipv4(5432).await?);

        // 2. Start fresh RabbitMQ
        let rabbitmq = RabbitMq::default().start().await?;
        let url = format!("amqp://...{}", rabbitmq.get_host_port_ipv4(5672).await?);

        // 3. Set env vars for THIS instance only
        set_distributed_env(&dsn, &url);

        // 4. Create coordinator with UNIQUE engine ID
        let mut coordinator = Engine::new(EngineConfig {
            mode: Mode::Coordinator,
            engine_id: Some(uuid()), // <-- KEY: unique per test
            ..Default::default()
        });
        coordinator.start().await?;

        // 5. Create worker
        let mut worker = Engine::new(EngineConfig {
            mode: Mode::Worker,
            engine_id: Some(uuid()), // <-- KEY: unique per test
            ..Default::default()
        });
        worker.start().await?;

        // 6. Start API server
        let (base_url, api_handle) = start_api(&coordinator).await?;
        wait_for_health(&base_url).await?;

        Ok(Self { ... })
    }

    async fn teardown(self) {
        self.api_handle.abort();
        let _ = self.worker.terminate().await;
        let _ = self.coordinator.terminate().await;
        // Env vars cleared automatically when struct drops
    }
}
```

### 3.2 The Problem with Current Design

**Current issue:** All engines subscribe to `x-jobs`, `x-pending`, etc. When 4 tests run:
```
Test A coordinator ──────► x-jobs ◄────── Test B coordinator
         │                      ▲
         │                      │
         └──────────────────────┘
              RabbitMQ round-robins
```

**Desired:** Each engine has isolated queues:
```
Test A coordinator ──────► x-jobs.test-a ◄────── only Test A
Test B coordinator ──────► x-jobs.test-b ◄────── only Test B
```

### 3.3 Required Engine Configuration

```rust
// Engine configuration must support engine_id
pub struct Config {
    pub mode: Mode,
    pub engine_id: Option<String>,  // None = auto-generate
    pub middleware: Middleware,
    pub endpoints: HashMap<String, EndpointHandler>,
}

// Engine must expose engine_id accessor
impl Engine {
    pub fn engine_id(&self) -> String { ... }
}
```

### 3.4 Queue Naming Strategy

| Scope | Queue Name Pattern | Example |
|-------|-------------------|---------|
| Coordinator queues (prefixed) | `x-{name}.{engine_id}` | `x-jobs.engine-abc-123` |
| Worker queues (per-worker, not per-engine) | `{queue_name}` | `default` |
| Exclusive queues (temp) | `x-exclusive.{uuid}` | `x-exclusive.abc123` (unchanged) |

### 3.5 Backward Compatibility

| Deployment | `engine_id` | Queue Names | Behavior |
|------------|-------------|-------------|----------|
| Single instance (existing) | empty/None | `x-jobs` | **UNCHANGED** |
| Single instance (new) | empty/None | `x-jobs` | Same |
| Multi-instance | unique per instance | `x-jobs.{engine_id}` | Isolated |
| E2E tests | unique per test | `x-jobs.{test-uuid}` | Isolated |

**Key invariant:** Empty `engine_id` must produce empty prefix (no dot, no suffix).

---

## Section 4 — Test Categories for Queue Isolation

### 4.1 Unit Tests (Pure Functions)

**File:** `crates/twerk-infrastructure/src/broker/mod.rs`

These test pure functions with no side effects.

**NOTE:** The functions `prefixed_queue()` and `extract_engine_id()` are **new helper functions** to be implemented in `broker/mod.rs` as part of the fix. They are not yet present in the codebase.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Queue naming behavior
    // Tests for prefixed_queue(qname, engine_id) and extract_engine_id(qname)
    // These are NEW functions to implement alongside the engine_id feature
    #[test]
    fn empty_engine_id_produces_no_prefix() {
        let engine_id = "";
        let queue = "x-pending";
        let result = prefixed_queue(queue, engine_id);
        assert_eq!(result, "x-pending"); // unchanged
    }

    #[test]
    fn non_empty_engine_id_produces_prefix() {
        let engine_id = "test-abc";
        let queue = "x-pending";
        let result = prefixed_queue(queue, engine_id);
        assert_eq!(result, "x-pending.test-abc");
    }

    #[test]
    fn extract_engine_id_from_prefixed_queue() {
        assert_eq!(extract_engine_id("x-jobs.test-abc"), Some("test-abc".to_string()));
    }

    #[test]
    fn extract_engine_id_returns_none_for_unprefixed() {
        assert_eq!(extract_engine_id("x-jobs"), None);
        assert_eq!(extract_engine_id("default"), None);
    }

    // Classification behavior
    // Note: x-pending is a WORKER queue, x-jobs is a COORDINATOR queue
    #[test]
    fn is_coordinator_queue_recognizes_prefixed() {
        assert!(is_coordinator_queue("x-jobs.engine-abc"));
        assert!(is_coordinator_queue("x-completed.engine-abc"));
        assert!(is_coordinator_queue("x-failed.engine-abc"));
    }

    #[test]
    fn is_worker_queue_rejects_coordinator_queues() {
        assert!(!is_worker_queue("x-jobs.engine-abc"));
        assert!(!is_worker_queue("x-completed.engine-abc"));
        assert!(is_worker_queue("default"));
        // x-pending is a worker queue (no prefix in original)
        assert!(is_worker_queue("x-pending"));
    }
}
```

### 4.2 Integration Tests (Real RabbitMQ)

**File:** `crates/twerk-infrastructure/tests/rabbitmq_isolation_test.rs`

These test with real RabbitMQ container:

```rust
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::rabbitmq::RabbitMq;

#[tokio::test]
async fn two_engines_with_different_ids_do_not_cross_talk() -> anyhow::Result<()> {
    // Setup: two brokers with different engine_ids
    let container = RabbitMq::default().start().await?;
    let url = format!("amqp://guest:guest@localhost:{}", 
        container.get_host_port_ipv4(5672).await?);

    let broker_a = RabbitMQBroker::new(&url, RabbitMQOptions::default(), "engine-a").await?;
    let broker_b = RabbitMQBroker::new(&url, RabbitMQOptions::default(), "engine-b").await?;

    // Engine A publishes to its prefixed queue
    let queue_a = "x-pending.engine-a";
    let task = Task::default();

    // Engine B subscribes to its prefixed queue
    let (tx, mut rx) = mpsc::channel(1);
    broker_b.subscribe_for_tasks("x-pending.engine-b", Arc::new(move |t| {
        let tx = tx.clone();
        Box::pin(async move {
            tx.send(()).await?;
            Ok(())
        })
    })).await?;

    // Publish to engine A's queue
    broker_a.publish_task(queue_a.to_string(), &task).await?;

    // Engine B should NOT receive anything (timeout = test passes)
    let result = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
    assert!(result.is_err(), "Engine B should not receive Engine A's task");

    Ok(())
}

#[tokio::test]
async fn engine_with_empty_id_uses_unprefixed_queues() -> anyhow::Result<()> {
    let container = RabbitMq::default().start().await?;
    let url = format!("amqp://guest:guest@localhost:{}", 
        container.get_host_port_ipv4(5672).await?);

    let broker = RabbitMQBroker::new(&url, RabbitMQOptions::default(), "").await?;

    let (tx, mut rx) = mpsc::channel(1);
    broker.subscribe_for_tasks("x-pending".to_string(), Arc::new(move |_| {
        let tx = tx.clone();
        Box::pin(async move {
            tx.send(()).await?;
            Ok(())
        })
    })).await?;

    let task = Task::default();
    broker.publish_task("x-pending".to_string(), &task).await?;

    // Should receive on unprefixed queue
    tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await?
        .ok_or_else(|| anyhow::anyhow!("timeout"))?;

    Ok(())
}
```

### 4.3 E2E Concurrent Isolation Test

**File:** `crates/twerk-web/tests/distributed_e2e_test.rs`

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_concurrent_engines_complete_independent_jobs() -> anyhow::Result<()> {
    // Spawn 4 test tasks concurrently
    let mut handles = Vec::new();

    for i in 0..4 {
        handles.push(tokio::spawn(async move {
            let env = DistributedEnv::new().await?;

            let (status, body) = submit_job(
                &env.client,
                &env.base_url,
                &json!({
                    "name": format!("concurrent-job-{i}"),
                    "tasks": [{
                        "name": format!("task-{i}"),
                        "run": format!("echo 'job {i}'")
                    }]
                }),
            ).await?;

            assert_eq!(status, StatusCode::OK);
            let job_id = body["id"].as_str().unwrap();

            let job = poll_job_until_terminal(
                &env.client,
                &env.base_url,
                job_id,
                Duration::from_secs(60)
            ).await?;

            let state = job["state"].as_str().unwrap();
            assert_eq!(state, "COMPLETED", "job {i} should complete");

            env.teardown().await;
            Ok::<_, anyhow::Error>(())
        }));
    }

    // All must complete
    for handle in handles {
        handle.await??;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_cross_talk_between_concurrent_engines() -> anyhow::Result<()> {
    // This is the REGRESSION TEST for the original bug
    // If queues are shared, jobs get stuck in SCHEDULED

    let mut handles = Vec::new();

    for i in 0..4 {
        handles.push(tokio::spawn(async move {
            let env = DistributedEnv::new().await?;

            let (status, body) = submit_job(
                &env.client,
                &env.base_url,
                &json!({
                    "name": format!("isolation-test-{i}"),
                    "tasks": [{
                        "name": "simple",
                        "run": "echo 'ok'"
                    }]
                }),
            ).await?;

            let job_id = body["id"].as_str().unwrap();

            // Poll with generous timeout
            let job = poll_job_until_terminal(
                &env.client,
                &env.base_url,
                job_id,
                Duration::from_secs(30)  // Should not timeout if isolated
            ).await?;

            env.teardown().await;
            Ok::<_, anyhow::Error>(job)
        }));
    }

    let mut states = Vec::new();
    for handle in handles {
        let job = handle.await??;
        states.push(job["state"].as_str().unwrap().to_string());
    }

    // All should be COMPLETED, none should be stuck in SCHEDULED
    for (i, state) in states.iter().enumerate() {
        assert_eq!(state, "COMPLETED", "job {i} got unexpected state: {state}");
    }

    Ok(())
}
```

---

## Section 5 — Regression Test Suite

### 5.1 Critical Regression Tests

These tests MUST pass before any merge:

| Test | Purpose | File |
|------|---------|------|
| `four_concurrent_engines_complete_independent_jobs` | Verify fix works | `distributed_e2e_test.rs` |
| `no_cross_talk_between_concurrent_engines` | Verify isolation | `distributed_e2e_test.rs` |
| `two_engines_with_different_ids_do_not_cross_talk` | Broker isolation | `rabbitmq_isolation_test.rs` |
| `engine_with_empty_id_uses_unprefixed_queues` | Backward compat | `rabbitmq_isolation_test.rs` |

### 5.2 Running the Tests

```bash
# Run just the concurrent isolation tests
cargo test --package twerk-web --test distributed_e2e_test -- no_cross_talk

# Run concurrent isolation tests with 4 threads
cargo test --package twerk-web --test distributed_e2e_test -- --test-threads=4 no_cross_talk

# Run RabbitMQ isolation tests
cargo test --package twerk-infrastructure --test rabbitmq_isolation_test

# Run all tests
cargo test --workspace
```

---

## Section 6 — Test Data Patterns

### 6.1 Job Payloads

```rust
fn simple_shell_task(name: &str, run: &str) -> serde_json::Value {
    json!({
        "name": name,
        "tasks": [{
            "name": format!("{}-task", name),
            "run": run
        }]
    })
}

fn failing_task(name: &str, exit_code: u8) -> serde_json::Value {
    json!({
        "name": name,
        "tasks": [{
            "name": format!("{}-task", name),
            "run": format!("exit {}", exit_code)
        }]
    })
}

fn multi_task_job(name: &str, steps: &[&str]) -> serde_json::Value {
    json!({
        "name": name,
        "tasks": steps.iter().enumerate().map(|(i, cmd)| {
            json!({
                "name": format!("step-{}", i),
                "run": cmd
            })
        }).collect::<Vec<_>>()
    })
}
```

### 6.2 Poll Until Terminal Pattern

```rust
async fn poll_job_until_terminal(
    client: &reqwest::Client,
    base_url: &str,
    job_id: &str,
    timeout: Duration,
) -> anyhow::Result<serde_json::Value> {
    let start = std::time::Instant::now();
    loop {
        let resp = client.get(format!("{}/jobs/{}", base_url, job_id)).send().await?;
        let job: serde_json::Value = resp.json().await?;
        let state = job["state"].as_str().unwrap_or("UNKNOWN");

        if matches!(state, "COMPLETED" | "FAILED" | "CANCELLED") {
            return Ok(job);
        }

        if start.elapsed() > timeout {
            anyhow::bail!(
                "job {} did not reach terminal state within {:?}, last state: {}",
                job_id, timeout, state
            );
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
```

---

## Section 7 — Mock Patterns

### 7.1 When to Use Mocks

| Use Case | Use Mock? | Reason |
|----------|-----------|--------|
| Unit tests of pure logic | No | Real logic is being tested |
| Integration tests with real broker | No | Broker behavior is part of test |
| Handler tests | Yes | Datastore/queue state controlled |
| API tests | Yes (middleware) | Focus on HTTP layer |

### 7.2 Mock Broker Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    struct MockBroker {
        published_tasks: Arc<std::sync::Mutex<Vec<Task>>>,
    }

    impl MockBroker {
        fn new() -> Self {
            Self {
                published_tasks: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
    }

    impl Broker for MockBroker {
        fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
            let task = task.clone();
            Box::pin(async move {
                self.published_tasks.lock().unwrap().push(task);
                Ok(())
            })
        }

        fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
            Box::pin(async move { Ok(()) })
        }

        // ... implement other methods as no-ops or capture args
    }
}
```

---

## Section 8 — Exit Criteria

### 8.1 For the Fix (Queue Isolation)

| Criterion | Verification |
|-----------|---------------|
| Concurrent tests pass with `--test-threads=4` | Run full suite 3 times |
| No jobs stuck in SCHEDULED state | Observe test output |
| Backward compatible (empty engine_id) | `engine_with_empty_id_uses_unprefixed_queues` passes |
| Queue names are correctly prefixed | Integration test verifies actual queue names |

### 8.2 For Test Quality

| Criterion | Verification |
|-----------|---------------|
| No `is_ok()` / `is_err()` assertions | Code review |
| All tests have doc comments | `cargo doc --test` |
| Tests are deterministic | Run with `--test-threads=1` and `--test-threads=4` |
| No flaky timeouts | Run 5 times, all pass |

### 8.3 For Coverage

| Layer | Target | Verification |
|-------|--------|--------------|
| Unit (pure functions) | 100% of queue naming logic | `cargo test --lib` |
| Integration | 4+ concurrent isolation scenarios | `rabbitmq_isolation_test.rs` |
| E2E | 2 concurrent tests (4 engines each) | `distributed_e2e_test.rs` |

---

## Section 9 — File Locations

| Purpose | Location | Notes |
|---------|----------|-------|
| Queue naming unit tests | `broker/mod.rs #[cfg(test)]` | Pure function tests |
| RabbitMQ integration tests | `tests/rabbitmq_test.rs` | Existing file, add isolation tests |
| New isolation tests | `tests/rabbitmq_isolation_test.rs` | New file |
| Concurrent E2E tests | `tests/distributed_e2e_test.rs` | Existing file, add concurrent tests |
| Handler unit tests | `tests/handlers_test.rs` | Existing file |

---

## Section 10 — Existing Tests to Preserve

The following existing tests must continue to pass:

```bash
# From distributed_e2e_test.rs (25 existing tests)
cargo test --package twerk-web --test distributed_e2e_test

# From twerk-infrastructure
cargo test --package twerk-infrastructure

# From twerk-app
cargo test --package twerk-app

# From twerk-core
cargo test --package twerk-core
```

**Do not modify or delete existing tests without explicit approval.**
