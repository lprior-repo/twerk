use criterion::{black_box, criterion_group, criterion_main, Bencher, BenchmarkId, Criterion};
use twerk_app::engine::{Config, Engine, MockRuntime, Mode};
use twerk_core::job::{Job, JOB_STATE_PENDING};
use twerk_core::task::Task;

fn create_test_engine() -> Engine {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);
    engine.register_runtime(Box::new(MockRuntime));
    engine
}

fn create_simple_job(id: &str) -> Job {
    Job {
        id: Some(id.into()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("test-task".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    }
}

fn create_parallel_job(id: &str, num_tasks: usize) -> Job {
    Job {
        id: Some(id.into()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("parallel-task".to_string()),
            parallel: Some(twerk_core::task::ParallelTask {
                tasks: Some(
                    (0..num_tasks)
                        .map(|i| Task {
                            name: Some(format!("p{}", i)),
                            image: Some("alpine".to_string()),
                            run: Some("echo hello".to_string()),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    }
}

fn engine_new(c: &mut Criterion) {
    c.bench_function("engine_new", |b: &mut Bencher| {
        b.iter(|| {
            black_box(create_test_engine());
        });
    });
}

fn engine_config(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_config");

    for mode in ["Standalone", "Worker", "Coordinator"] {
        group.bench_function(mode, |b: &mut Bencher| {
            b.iter(|| {
                std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
                std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
                let mut config = Config::default();
                config.mode = match mode {
                    "Standalone" => Mode::Standalone,
                    "Worker" => Mode::Worker,
                    "Coordinator" => Mode::Coordinator,
                    _ => Mode::Standalone,
                };
                black_box(Engine::new(config));
            });
        });
    }

    group.finish();
}

fn job_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("job_creation");

    for size in [1, 5, 10, 50] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b: &mut Bencher, &size| {
                b.iter(|| {
                    let job = if size == 1 {
                        create_simple_job("job-1")
                    } else {
                        create_parallel_job("job-parallel", size)
                    };
                    black_box(job);
                });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(100);
    targets = engine_new, engine_config, job_creation
}
criterion_main!(benches);
