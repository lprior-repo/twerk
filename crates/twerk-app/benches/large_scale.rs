use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
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
                    let mut task = Task::default();
                    task.id = Some(format!("task-{}", i).into());
                    task.job_id = Some("target-job".into());
                    ds.create_task(&task).await.unwrap();
                }
                // Add noise tasks
                for i in 0..(size * 2) {
                    let mut task = Task::default();
                    task.id = Some(format!("noise-task-{}", i).into());
                    task.job_id = Some("other-job".into());
                    ds.create_task(&task).await.unwrap();
                }
            });

            b.iter(|| {
                rt.block_on(async {
                    black_box(ds.get_active_tasks("target-job").await.unwrap());
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
