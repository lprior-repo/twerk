#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::field_reassign_with_default)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tokio::runtime::Runtime;
use twerk_app::engine::{Config, Engine, MockRuntime, Mode};
use twerk_core::job::{Job, JobState};
use twerk_core::task::Task;

fn create_massive_parallel_job(id: &str, num_tasks: usize) -> Job {
    Job {
        id: Some(id.into()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("massive-parallel-task".to_string()),
            parallel: Some(twerk_core::task::ParallelTask {
                tasks: Some(
                    (0..num_tasks)
                        .map(|i| Task {
                            name: Some(format!("p{i}")),
                            image: Some("alpine".to_string()),
                            run: Some("echo 10x".to_string()),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1, // Technically 1 root task, but the scheduler expands it
        ..Default::default()
    }
}

fn bench_massive_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("10x_stress_test");
    group.sample_size(10);
    // Increase timeout since massive jobs take time
    group.measurement_time(std::time::Duration::from_secs(15));

    let rt = Runtime::new().unwrap();

    // Test with 100, 1000, 10000 parallel subtasks
    for size in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
                    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
                    let mut engine = Engine::new(Config {
                        mode: Mode::Standalone,
                        ..Default::default()
                    });
                    engine.register_runtime(Box::new(MockRuntime));

                    let _ = engine.start().await;

                    let job = create_massive_parallel_job("massive-job", size);

                    // Submit the job
                    let _ = engine.submit_job(job, vec![]).await;

                    // We don't await full completion in the microbenchmark iter because 10,000 tasks
                    // will take significant wall-clock time even with a MockRuntime.
                    // We are benchmarking the Coordinator's ability to ACCEPT and SCHEDULE
                    // the massive 10x influx without blocking or crashing.
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

                    let _ = engine.terminate().await;
                });
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_massive_scale);
criterion_main!(benches);
