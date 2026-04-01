# Performance Testing Guide for Twerk

> How we profile, benchmark, and optimize the Twerk distributed workflow engine.

---

## Table of Contents

1. [Overview](#overview)
2. [Baseline Metrics](#baseline-metrics)
3. [Benchmark Setup](#benchmark-setup)
4. [Profiling Tools](#profiling-tools)
5. [Key Bottlenecks](#key-bottlenecks)
6. [Test Infrastructure Notes](#test-infrastructure-notes)

---

## Overview

This document describes how we measure and improve Twerk's performance. Twerk is a distributed task execution system with three operational modes:

- **Coordinator**: Schedules jobs and coordinates task execution
- **Worker**: Executes tasks via Docker/Podman/Shell runtimes
- **Standalone**: Combined Coordinator + Worker in a single process

Performance testing focuses on measuring:
- Engine creation latency
- Job submission throughput
- Task scheduling efficiency
- End-to-end job completion time

---

## Baseline Metrics

Measured on `rustc 1.94.0` with `cargo bench --sample-size=100`:

### Microbenchmarks (Criterion)

| Operation | Time | Notes |
|-----------|------|-------|
| `engine_new` | **867 ns** | Fresh engine creation |
| `engine_config/Standalone` | **811 ns** | With inmemory broker/datastore |
| `engine_config/Worker` | **819 ns** | |
| `engine_config/Coordinator` | **817 ns** | |
| `job_creation/1` | **161 ns** | Single task job |
| `job_creation/5` | **458 ns** | 5 parallel tasks |
| `job_creation/10` | **659 ns** | 10 parallel tasks |
| `job_creation/50` | **4.79 µs** | 50 parallel tasks |

### E2E Tests

| Test Suite | Result | Time | Notes |
|------------|--------|------|-------|
| Standalone E2E | 6/6 pass | 0.78s | In-memory, MockRuntime |
| Distributed E2E | 25/26 pass | 126.9s | Real Postgres + RabbitMQ |

---

## Benchmark Setup

### 1. Add Criterion to `Cargo.toml`

```toml
# In crates/twerk-app/Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "engine"
harness = false
```

### 2. Create Benchmark File

```rust
// crates/twerk-app/benches/engine.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use twerk_app::engine::{Config, Engine, MockRuntime, Mode};

fn create_test_engine() -> Engine {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);
    engine.register_runtime(Box::new(MockRuntime));
    engine
}

fn engine_new(c: &mut Criterion) {
    c.bench_function("engine_new", |b| {
        b.iter(|| {
            black_box(create_test_engine());
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(100);
    targets = engine_new
}
criterion_main!(benches);
```

### 3. Run Benchmarks

```bash
# Run all benchmarks
cargo bench -p twerk-app --bench engine

# Run with specific sample size
cargo bench -p twerk-app --bench engine -- --sample-size=100
```

---

## Profiling Tools

### 1. `cargo-flamegraph` - CPU Flamegraphs

```bash
# Install
cargo install flamegraph

# Run (requires perf or sudo)
cargo flamegraph --deterministic --root -p twerk-app --test engine_lifecycle_test
```

**Note**: Requires `linux-perf` package:
```bash
sudo apt-get install linux-perf
```

### 2. `tokio-console` - Async Analysis

```bash
# Install
cargo install tokio-console

# Run your app with RUST_BACKTRACE and TOKIO_CONSOLE袜子
RUSTFLAGS="--cfg tokio_unstable" cargo run --bin twerk &
tokio-console
```

### 3. Built-in Timing

```bash
# Time test execution
time cargo test --workspace

# Time specific test
time cargo test -p twerk-app --test standalone_e2e_test
```

### 4. Memory Profiling

```bash
# Install dhat for heap analysis
cargo install dhat

# Run with dhat
dhat ./target/release/twerk run standalone
```

---

## Key Bottlenecks

### P0 - Critical (High Impact)

#### 1. Excessive `tokio::spawn` in Broker

**File**: `crates/twerk-infrastructure/src/broker/inmemory/publish.rs`

```rust
// Current: Spawns per handler per message
for handler in handlers {
    tokio::spawn(async move {
        let _ = handler(task_clone).await;
    });
}
```

**Fix**: Use `for_each_concurrent`:
```rust
use futures::stream::StreamExt;
futures::stream::iter(handlers)
    .for_each_concurrent(50, |handler| async {
        let _ = handler(task_clone).await;
    })
    .await;
```

#### 2. Sequential Task Creation in Scheduler

**File**: `crates/twerk-app/src/engine/coordinator/scheduler/parallel.rs`

```rust
// Current: Sequential awaits
for t in tasks {
    self.ds.create_task(&pt).await?;
    self.broker.publish_task(...).await?;
}
```

**Fix**: Parallel creation:
```rust
use futures::future::try_join_all;

let futures = tasks.map(|t| async {
    let pt = evaluate_and_prepare(t);
    self.ds.create_task(&pt).await?;
    self.broker.publish_task(...).await?;
    Ok::<(), anyhow::Error>(())
});
try_join_all(futures).await?;
```

### P1 - Important (Medium Impact)

#### 3. Full Collection Scans in Datastore

**File**: `crates/twerk-infrastructure/src/datastore/inmemory.rs`

```rust
// Current: O(n) scan
async fn get_active_tasks(&self, job_id: &str) -> Result<Vec<Task>> {
    Ok(self.tasks.iter()
        .filter(|e| e.value().job_id.as_deref() == Some(job_id))
        .map(|e| e.value().clone())
        .collect())
}
```

**Fix**: Add secondary index:
```rust
pub struct InMemoryDatastore {
    tasks: Arc<DashMap<TaskId, Task>>,
    job_tasks: Arc<DashMap<JobId, Vec<TaskId>>>,  // Secondary index
}

async fn create_task(&self, task: &Task) -> Result<()> {
    let id = task.id.clone().unwrap();
    self.tasks.insert(id, task.clone());
    if let Some(ref job_id) = task.job_id {
        self.job_tasks
            .entry(job_id.clone())
            .or_default()
            .push(id);
    }
    Ok(())
}
```

### P2 - Nice to Have (Low Impact)

#### 4. Busy-Wait Shutdown

**File**: `crates/twerk-infrastructure/src/worker/internal/worker.rs`

```rust
// Current: Busy wait with 100ms polling
while !is_complete() {
    sleep(check_interval).await;  // 100ms
}
```

**Fix**: Use `watch` channel:
```rust
let (tx, mut rx) = tokio::sync::watch::channel(());
tokio::select! {
    _ = rx.changed() => { /* signaled */ }
    _ = sleep(timeout) => { /* timed out */ }
}
```

---

## Test Infrastructure Notes

### Docker Container Management

The distributed E2E tests spin up real Postgres and RabbitMQ containers via `testcontainers`. When running tests in parallel, Docker can exhaust resources.

#### Symptoms
- `container startup timeout` errors
- Tests fail with `connection failed: relative URL without a base`
- Jobs stuck in `SCHEDULED` state

#### Mitigation

```bash
# Clean up Docker before running tests
docker ps -a --format "{{.Names}}" | grep -v -E "^(buildx|twerk|dagger)" | xargs -r docker rm -f
docker volume prune -f

# Run tests sequentially to avoid resource contention
cargo test -p twerk-web --test distributed_e2e_test -- --test-threads=1
```

#### Test Results by Mode

| Mode | Threads | Result |
|------|---------|--------|
| Sequential | `--test-threads=1` | 26/26 pass |
| Parallel | default | 21/26 pass (Docker resource exhaustion) |

The failures are **not engine bugs** but Docker infrastructure limitations under load.

---

## Benchmarking Best Practices

1. **Use `--release` for realistic numbers**
2. **Warm up before measuring** - Criterion does this automatically
3. **Run multiple samples** - `sample_size=100` minimum for statistical significance
4. **Isolate concerns** - Test engine_new separately from job_submit
5. **Use MockRuntime** for consistent, fast tests
6. **Clean Docker state** before E2E tests

---

## Baseline Command Reference

```bash
# Quick baseline
cargo bench -p twerk-app --bench engine -- --sample-size=100

# Full test suite with timing
time cargo test --workspace

# Distributed E2E (sequential)
cargo test -p twerk-web --test distributed_e2e_test -- --test-threads=1

# Flamegraph (if perf available)
cargo flamegraph --deterministic --root -p twerk-app --test engine_lifecycle_test

# Check Docker state
docker ps -a --format "table {{.Names}}\t{{.Status}}"
```

---

## Future Improvements

1. Add continuous profiling with `pprof` integration
2. Set up benchmarks in CI to catch regressions
3. Add latency percentiles (p50, p95, p99) to criterion output
4. Benchmark real runtime (Docker/Podman) overhead vs MockRuntime
5. Add load testing with multiple concurrent jobs

---

*Document version: 1.0*
*Last updated: 2026-03-31*
