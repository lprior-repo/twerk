//! Hot-path benchmarks for the in-memory datastore.
//!
//! Measures the performance of secondary index vs full scan queries.

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
fn make_task(id: &str, job_id: &JobId) -> Task {
    Task {
        id: Some(TaskId::new(id).unwrap()),
        job_id: Some(job_id.clone()),
        state: TaskState::Pending,
        name: Some(format!("task-{}", id)),
        ..Default::default()
    }
}

// ── Secondary Index Benchmark (synchronous) ─────────────────────────────────────

/// Simulates the old full-scan query
fn query_full_scan(tasks: &DashMap<String, Task>, job_id: &str) -> Vec<Task> {
    tasks
        .iter()
        .filter(|e| e.value().job_id.as_deref() == Some(job_id))
        .map(|e| e.value().clone())
        .collect()
}

/// Simulates the new indexed query
fn query_indexed(tasks: &DashMap<String, Task>, tasks_by_job: &DashMap<JobId, Vec<TaskId>>, job_id: &JobId) -> Vec<Task> {
    if let Some(task_ids) = tasks_by_job.get(job_id) {
        task_ids
            .value()
            .iter()
            .filter_map(|tid| {
                // TaskId implements AsRef<str> and Deref<Target = str>
                let tid_str: &str = tid.as_ref();
                tasks.get(tid_str).map(|t| t.value().clone())
            })
            .collect()
    } else {
        Vec::new()
    }
}

fn secondary_index_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("secondary_index_comparison");

    for total_tasks in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(total_tasks as u64));

        let job_id = make_job_id();
        let job_id_str = job_id.as_str().to_string();

        // Pre-populate DashMap
        let tasks: DashMap<String, Task> = DashMap::new();
        let tasks_by_job: DashMap<JobId, Vec<TaskId>> = DashMap::new();

        for i in 0..total_tasks {
            let task_id = format!("task-{}", i);
            let task = make_task(&task_id, &job_id);
            let tid = task.id.clone().unwrap();
            tasks.insert(task_id, task);
            tasks_by_job.entry(job_id.clone()).or_default().push(tid);
        }

        // Benchmark: old full scan
        group.bench_with_input(
            BenchmarkId::new("full_scan", total_tasks),
            &total_tasks,
            |b, &_total| {
                b.iter(|| {
                    let result = query_full_scan(&tasks, &job_id_str);
                    criterion::black_box(result);
                });
            },
        );

        // Benchmark: new indexed lookup
        group.bench_with_input(
            BenchmarkId::new("indexed_lookup", total_tasks),
            &total_tasks,
            |b, &_total| {
                let job_id = make_job_id();
                b.iter(|| {
                    let result = query_indexed(&tasks, &tasks_by_job, &job_id);
                    criterion::black_box(result);
                });
            },
        );
    }

    group.finish();
}

// ── Clone overhead ────────────────────────────────────────────────────────────

fn task_clone_overhead(c: &mut Criterion) {
    let job_id = make_job_id();
    let task = make_task("clone-test", &job_id);

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

// ── DashMap vs Vec comparison ────────────────────────────────────────────────

fn queue_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_operations");

    for size in [1, 10, 100, 1000] {
        group.throughput(Throughput::Elements(size as u64));

        // Pre-create tasks
        let tasks: Vec<Arc<Task>> = (0..size)
            .map(|i| Arc::new(make_task(&format!("task-{}", i), &make_job_id())))
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

// ── Main ─────────────────────────────────────────────────────────────────────

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(100);
    targets = secondary_index_benchmark, task_clone_overhead, queue_operations
);
criterion_main!(benches);