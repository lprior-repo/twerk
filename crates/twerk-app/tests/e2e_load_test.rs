//! E2E Load Test - Measures real coordinator + database throughput
//!
//! This test runs the full stack with `PostgreSQL` and `RabbitMQ` to find actual bottlenecks.
//!
//! Usage:
//!   cargo test --test `e2e_load_test` -- --nocapture

#![allow(clippy::expect_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_precision_loss)]

use std::time::{Duration, Instant};
use twerk_app::engine::{Config, Engine, MockRuntime, Mode};
use twerk_core::job::Job;
use twerk_core::task::Task;

/// Creates a parallel job struct with N tasks
fn create_parallel_job(job_id: &str, num_tasks: usize) -> Job {
    Job {
        id: Some(job_id.into()),
        name: Some("load-test-job".to_string()),
        state: "PENDING".to_string(),
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
async fn e2e_load_test_100_tasks() -> anyhow::Result<()> {
    e2e_load_test(100).await
}

#[tokio::test]
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

    let job_id = format!("load-test-{task_count}");
    let job = create_parallel_job(&job_id, task_count);

    let schedule_start = Instant::now();
    engine.submit_job(job, vec![]).await?;
    let schedule_time = schedule_start.elapsed();

    // Wait for coordinator to process all tasks
    tokio::time::sleep(Duration::from_millis(500)).await;

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
        // Use short_uuid (22 chars) to fit in varchar(32)
        let job_id = new_short_uuid();
        for i in 0..size {
            let task = Task {
                id: Some(format!("{}-{:04}", job_id, i).into()),
                job_id: Some(job_id.clone().into()),
                name: Some("db-test".to_string()),
                state: "CREATED".to_string(),
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
        // Create test tasks with unique job ID (short_uuid is 22 chars, fits in varchar(32))
        let job_id = new_short_uuid();
        for i in 0..size {
            let task = Task {
                id: Some(format!("{}-{:04}", job_id, i).into()),
                job_id: Some(job_id.clone().into()),
                name: Some("query-test".to_string()),
                state: "CREATED".to_string(),
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
                    // Each thread gets its own job_id (short_uuid is 22 chars, fits varchar(32))
                    let job_id = new_short_uuid();
                    for i in 0..tasks_per_thread {
                        let task = Task {
                            id: Some(format!("{}-{t}-{i:04}", &job_id[..8]).into()),
                            job_id: Some(job_id.clone().into()),
                            name: Some("concurrent-test".to_string()),
                            state: "CREATED".to_string(),
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
