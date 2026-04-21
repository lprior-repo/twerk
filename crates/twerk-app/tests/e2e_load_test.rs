//! E2E Load Test - Measures real coordinator + database throughput
//!
//! These tests require a running PostgreSQL and RabbitMQ instance.
//! They are ignored by default. Run with:
//!   cargo test --test e2e_load_test -- --ignored --nocapture
//!
//! Prerequisites:
//!   docker compose up -d  (starts Postgres + RabbitMQ)
//!   cargo run -- migration (runs DB migrations)

#![allow(clippy::expect_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_precision_loss)]

use std::time::Instant;
use twerk_app::engine::{Config, Engine, MockRuntime, Mode};
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_core::task::{Task, TaskState};

fn to_job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("test job id should be valid")
}

/// Creates a parallel job struct with N tasks
fn create_parallel_job(job_id: &str, num_tasks: usize) -> Job {
    Job {
        id: Some(to_job_id(job_id)),
        name: Some("load-test-job".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("parallel-root".to_string()),
            parallel: Some(twerk_core::task::ParallelTask {
                tasks: Some(
                    (0..num_tasks)
                        .map(|i| Task {
                            name: Some(format!("p{i}")),
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

/// E2E Load Test - Measures coordinator throughput with real `PostgreSQL`
#[tokio::test]
#[ignore = "requires running PostgreSQL and RabbitMQ"]
async fn e2e_load_test_100_tasks() -> anyhow::Result<()> {
    e2e_load_test(100).await
}

#[tokio::test]
#[ignore = "requires running PostgreSQL and RabbitMQ"]
async fn e2e_load_test_1000_tasks() -> anyhow::Result<()> {
    e2e_load_test(1000).await
}

async fn e2e_load_test(task_count: usize) -> anyhow::Result<()> {
    // Use postgres for real database pressure
    std::env::set_var("TWERK_DATASTORE_TYPE", "postgres");
    std::env::set_var("TWERK_BROKER_TYPE", "rabbitmq");
    std::env::set_var(
        "BROKER_RABBITMQ_URL",
        "amqp://guest:guest@localhost:5672/%2f",
    );

    let mut engine = Engine::new(Config {
        mode: Mode::Standalone,
        ..Default::default()
    });
    engine.register_runtime(Box::new(MockRuntime));

    let start = Instant::now();
    engine.start().await?;

    let job_id = twerk_core::uuid::new_short_uuid();
    let job = create_parallel_job(&job_id, task_count);

    let schedule_start = Instant::now();
    engine.submit_job(job, vec![]).await?;
    let schedule_time = schedule_start.elapsed();

    // Yield to let coordinator process queued tasks
    tokio::task::yield_now().await;

    engine.terminate().await?;
    let total_time = start.elapsed();

    let throughput = task_count as f64 / total_time.as_secs_f64();

    println!("\n=== E2E Load Test Results ===");
    println!("Task count:     {task_count}");
    println!("Schedule time:   {schedule_time:?}");
    println!("Total time:     {total_time:?}");
    println!("Throughput:     {throughput:.0} tasks/sec");
    println!("===========================\n");

    Ok(())
}

/// Database Write Throughput Test
/// Measures how fast PostgreSQL can handle sequential inserts
#[tokio::test]
#[ignore = "requires running PostgreSQL with migrated schema"]
async fn db_write_throughput_test() -> anyhow::Result<()> {
    use twerk_core::uuid::new_short_uuid;
    use twerk_infrastructure::datastore::postgres::PostgresDatastore;
    use twerk_infrastructure::datastore::{Datastore, Options};

    let dsn = "postgres://twerk:twerk@localhost:5433/twerk";
    let options = Options::default();
    let ds = PostgresDatastore::new(dsn, options).await?;

    let test_sizes = [100, 500, 1000, 5000];

    println!("\n=== Database Write Throughput ===");

    for size in test_sizes {
        let start = Instant::now();
        let job_id = new_short_uuid();

        for i in 0..size {
            let task = Task {
                id: Some(format!("{}-{:04}", job_id, i).into()),
                job_id: Some(to_job_id(job_id.clone())),
                name: Some("db-test".to_string()),
                state: TaskState::Created,
                ..Default::default()
            };
            ds.create_task(&task).await?;
        }
        let elapsed = start.elapsed();
        let rate = f64::from(size) / elapsed.as_secs_f64();

        println!("Size: {size} | Time: {elapsed:?} | Rate: {rate:.0} tasks/sec");
    }

    println!("================================\n");
    Ok(())
}

/// Database Query Under Load Test
/// Measures get_active_tasks performance as data grows
#[tokio::test]
#[ignore = "requires running PostgreSQL with migrated schema"]
async fn db_query_under_load_test() -> anyhow::Result<()> {
    use twerk_core::uuid::new_short_uuid;
    use twerk_infrastructure::datastore::postgres::PostgresDatastore;
    use twerk_infrastructure::datastore::{Datastore, Options};

    let dsn = "postgres://twerk:twerk@localhost:5433/twerk";
    let options = Options::default();
    let ds = PostgresDatastore::new(dsn, options).await?;

    let sizes = [100, 1000, 5000, 10000];

    println!("\n=== Database Query Under Load ===");

    for size in sizes {
        let job_id = new_short_uuid();
        for i in 0..size {
            let task = Task {
                id: Some(format!("{}-{:04}", job_id, i).into()),
                job_id: Some(to_job_id(job_id.clone())),
                name: Some("query-test".to_string()),
                state: TaskState::Created,
                ..Default::default()
            };
            ds.create_task(&task).await?;
        }

        // Measure query time
        let start = Instant::now();
        let active = ds.get_active_tasks(&job_id).await?;
        let elapsed = start.elapsed();

        println!(
            "Total tasks: {size} | Query time: {elapsed:?} | Active found: {}",
            active.len()
        );
    }

    println!("================================\n");
    Ok(())
}

/// Concurrent Database Writes Test
/// Measures database performance under concurrent load
#[tokio::test]
#[ignore = "requires running PostgreSQL with migrated schema"]
async fn db_concurrent_write_test() -> anyhow::Result<()> {
    use twerk_core::uuid::new_short_uuid;
    use twerk_infrastructure::datastore::postgres::PostgresDatastore;
    use twerk_infrastructure::datastore::{Datastore, Options};

    let dsn = "postgres://twerk:twerk@localhost:5433/twerk";

    let concurrency_levels = [1, 5, 10, 25];
    let tasks_per_thread = 100;

    println!("\n=== Concurrent Database Writes ===");

    for concurrency in concurrency_levels {
        let total_tasks = concurrency * tasks_per_thread;

        let start = Instant::now();

        // Spawn concurrent writers - each gets its own connection and job_id
        let handles: Vec<_> = (0..concurrency)
            .map(|t| {
                let dsn = dsn.to_string();
                async move {
                    let opts = Options::default();
                    let ds = PostgresDatastore::new(&dsn, opts)
                        .await
                        .expect("failed to connect");
                    let job_id = new_short_uuid();
                    for i in 0..tasks_per_thread {
                        let task = Task {
                            id: Some(format!("{}-{t}-{i:04}", &job_id[..8]).into()),
                            job_id: Some(to_job_id(job_id.clone())),
                            name: Some("concurrent-test".to_string()),
                            state: TaskState::Created,
                            ..Default::default()
                        };
                        ds.create_task(&task).await.expect("failed to create task");
                    }
                }
            })
            .collect();

        for h in handles {
            h.await;
        }

        let elapsed = start.elapsed();
        let rate = f64::from(total_tasks) / elapsed.as_secs_f64();

        println!(
            "Concurrency: {concurrency} | Total: {total_tasks} | Time: {elapsed:?} | Rate: {rate:.0} tasks/sec"
        );
    }

    println!("===============================\n");
    Ok(())
}
