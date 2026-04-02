#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::field_reassign_with_default)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use twerk_core::task::Task;
use twerk_infrastructure::datastore::inmemory::InMemoryDatastore;
use twerk_infrastructure::datastore::Datastore;

fn bench_get_active_tasks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("get_active_tasks");

    for size in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let ds = InMemoryDatastore::new();

            // Populate datastore
            rt.block_on(async {
                for i in 0..size {
                    let task = Task {
                        id: Some(format!("task-{i}").into()),
                        job_id: Some("target-job".into()),
                        ..Default::default()
                    };
                    let _ = ds.create_task(&task).await;
                }
                // Add noise tasks
                for i in 0..(size * 2) {
                    let task = Task {
                        id: Some(format!("noise-task-{i}").into()),
                        job_id: Some("other-job".into()),
                        ..Default::default()
                    };
                    let _ = ds.create_task(&task).await;
                }
            });

            b.iter(|| {
                rt.block_on(async {
                    let _active_tasks = ds.get_active_tasks("target-job").await;
                });
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_get_active_tasks
}
criterion_main!(benches);
