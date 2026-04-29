//! Hot-path benchmarks for the in-memory broker publish path.
//!
//! Baseline measurement BEFORE optimization changes.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dashmap::DashMap;
use std::sync::Arc;
use twerk_core::id::{JobId, TaskId};
use twerk_core::task::{Task, TaskState};

/// Creates a valid JobId for benchmarking using UUID format.
fn make_job_id() -> JobId {
    JobId::new("550e8400-e29b-41d4-a716-446655440000").unwrap()
}

/// Creates a task with minimal fields for benchmarking.
fn make_task(id: &str) -> Task {
    Task {
        id: Some(TaskId::new(id).unwrap()),
        job_id: Some(make_job_id()),
        state: TaskState::Pending,
        name: Some(format!("task-{}", id)),
        ..Default::default()
    }
}

/// Creates multiple tasks for batch benchmarks.
fn make_tasks(count: usize) -> Vec<Task> {
    (0..count).map(|i| make_task(&format!("task-{}", i))).collect()
}

// ── DashMap Operations (synchronous measurements) ───────────────────────────

fn dashmap_task_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("dashmap_task_insert");

    for size in [1, 10, 100, 1000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                let tasks: DashMap<String, Task> = DashMap::new();
                let tasks_vec = make_tasks(size);
                for task in tasks_vec {
                    if let Some(id) = &task.id {
                        tasks.insert(id.to_string(), task);
                    }
                }
            });
        });
    }

    group.finish();
}

fn dashmap_task_iterate(c: &mut Criterion) {
    let mut group = c.benchmark_group("dashmap_task_iterate");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        // Pre-populate
        let tasks: DashMap<String, Task> = DashMap::new();
        let tasks_vec = make_tasks(size);
        for task in tasks_vec {
            if let Some(id) = &task.id {
                tasks.insert(id.to_string(), task);
            }
        }

        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                let count = tasks.iter().count();
                criterion::black_box(count);
            });
        });
    }

    group.finish();
}

// ── Clone overhead ────────────────────────────────────────────────────────────

fn task_clone_overhead(c: &mut Criterion) {
    let task = make_task("clone-test");

    let mut group = c.benchmark_group("task_clone");
    group.throughput(Throughput::Elements(1));

    group.bench_function("full_task_clone", |b| {
        b.iter(|| {
            let _ = task.clone();
        });
    });

    group.bench_function("arc_clone_only", |b| {
        let arc_task = Arc::new(task.clone());
        b.iter(|| {
            let _ = Arc::clone(&arc_task);
        });
    });

    group.bench_function("arc_new_with_clone", |b| {
        b.iter(|| {
            let _ = Arc::new(task.clone());
        });
    });

    group.finish();
}

// ── Vec vs DashMap for queue ─────────────────────────────────────────────────

fn queue_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_operations");

    for size in [1, 10, 100, 1000] {
        group.throughput(Throughput::Elements(size as u64));

        // Pre-create tasks
        let tasks: Vec<Arc<Task>> = make_tasks(size)
            .into_iter()
            .map(|t| Arc::new(t))
            .collect();

        group.bench_function(BenchmarkId::new("vec_push", size), |b| {
            b.iter(|| {
                let mut queue: Vec<Arc<Task>> = Vec::new();
                for task in &tasks {
                    queue.push(Arc::clone(task));
                }
            });
        });

        group.bench_function(BenchmarkId::new("dashmap_insert", size), |b| {
            b.iter(|| {
                let queue: DashMap<String, Arc<Task>> = DashMap::new();
                for task in &tasks {
                    if let Some(id) = &task.id {
                        queue.insert(id.to_string(), Arc::clone(task));
                    }
                }
            });
        });
    }

    group.finish();
}

// ── Handler invocation simulation ─────────────────────────────────────────────

fn handler_invocation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("handler_invocation");

    // Simulate a simple handler
    async fn handler(_task: Arc<Task>) -> Result<(), ()> {
        Ok(())
    }

    let task = Arc::new(make_task("test"));

    group.bench_function("direct_call", |b| {
        b.iter(|| {
            let _ = criterion::black_box(handler(task.clone()));
        });
    });

    group.bench_function("arc_clone_only", |b| {
        let task = Arc::new(make_task("test"));
        b.iter(|| {
            let _ = Arc::clone(&task);
        });
    });

    group.finish();
}

// ── Main ─────────────────────────────────────────────────────────────────────

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(100);
    targets = dashmap_task_insert, dashmap_task_iterate, task_clone_overhead, queue_operations, handler_invocation_overhead
);
criterion_main!(benches);