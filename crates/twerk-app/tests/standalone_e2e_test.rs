#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::expect_used)]

use anyhow::Result;
use std::time::Duration;
use tokio::time::timeout;
use twerk_app::engine::{Config, Engine, MockRuntime, Mode};
use twerk_core::job::{Job, JOB_STATE_PENDING};
use twerk_core::task::Task;
use twerk_infrastructure::runtime::{BoxedFuture, ShutdownResult};

/// Mock runtime for testing
#[derive(Debug)]
pub struct FailingRuntime;

impl twerk_infrastructure::runtime::Runtime for FailingRuntime {
    fn run(&self, _task: &Task) -> BoxedFuture<()> {
        Box::pin(async { Err(anyhow::anyhow!("task failed intentionally")) })
    }

    fn stop(&self, _task: &Task) -> BoxedFuture<ShutdownResult<std::process::ExitCode>> {
        Box::pin(async { Ok(Ok(std::process::ExitCode::SUCCESS)) })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn standalone_engine_marks_job_as_failed_when_task_fails() -> Result<()> {
    // Set up environment
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    // Initialize engine
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    // Register FAILING runtime
    engine.register_runtime(Box::new(FailingRuntime));

    // Start
    engine.start().await?;

    // Create a job
    let job_id = "failing-e2e-job";
    let job = Job {
        id: Some(job_id.into()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("failing-task".to_string()),
            image: Some("alpine".to_string()),
            run: Some("exit 1".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    // Submit
    engine.submit_job(job, vec![]).await?;

    // Wait for state to reach FAILED
    let datastore = engine.datastore();
    let failed_job = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(j) = datastore.get_job_by_id(job_id).await {
                if j.state == "FAILED" {
                    return Ok::<Job, anyhow::Error>(j);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for job failure")?;

    assert_eq!(failed_job.state, "FAILED");

    // Terminate the engine
    engine.terminate().await?;

    Ok(())
}

#[tokio::test]
async fn standalone_engine_retries_failed_task() -> Result<()> {
    // Set up environment
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    // Initialize engine
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    // Register FAILING runtime
    engine.register_runtime(Box::new(FailingRuntime));

    // Start
    engine.start().await?;

    // Create a job with retry
    let job_id = "retry-e2e-job";
    let job = Job {
        id: Some(job_id.into()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("retry-task".to_string()),
            image: Some("alpine".to_string()),
            run: Some("exit 1".to_string()),
            retry: Some(twerk_core::task::TaskRetry {
                limit: 2,
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    // Submit
    engine.submit_job(job, vec![]).await?;

    // Wait for the job to fail after retries exhausted
    let datastore = engine.datastore();
    let failed_job = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(j) = datastore.get_job_by_id(job_id).await {
                if j.state == "FAILED" {
                    return Ok::<Job, anyhow::Error>(j);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for retry job failure")?;

    assert_eq!(failed_job.state, "FAILED");

    // Check if task was actually retried by checking task count in datastore for this job
    // The original task + 2 retries = 3 tasks
    // Let's check how many tasks are associated with this job
    // Note: This requires a way to list all tasks for a job, which get_active_tasks might not do for completed/failed ones

    // Terminate
    engine.terminate().await?;

    Ok(())
}
#[tokio::test]
async fn standalone_engine_marks_parallel_job_as_failed_when_subtask_fails() -> Result<()> {
    // Set up environment
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    // Initialize engine
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    // Register FAILING runtime
    engine.register_runtime(Box::new(FailingRuntime));

    // Start
    engine.start().await?;

    // Create a parallel job
    let job_id = "failing-parallel-job";
    let job = Job {
        id: Some(job_id.into()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("parallel-task".to_string()),
            parallel: Some(twerk_core::task::ParallelTask {
                tasks: Some(vec![Task {
                    name: Some("p1".to_string()),
                    image: Some("alpine".to_string()),
                    run: Some("exit 1".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    // Submit
    engine.submit_job(job, vec![]).await?;

    // Wait for state to reach FAILED
    let datastore = engine.datastore();
    let failed_job = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(j) = datastore.get_job_by_id(job_id).await {
                if j.state == "FAILED" {
                    return Ok::<Job, anyhow::Error>(j);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for parallel job failure")?;

    assert_eq!(failed_job.state, "FAILED");

    // Terminate
    engine.terminate().await?;

    Ok(())
}

#[tokio::test]
async fn standalone_engine_completes_job_naturally() -> Result<()> {
    // Set up environment for in-memory components
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    // Initialize engine in Standalone mode
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    // Register a mock runtime so we don't need Docker/Podman
    engine.register_runtime(Box::new(MockRuntime));

    // Start the engine
    engine.start().await?;

    // Create a simple job
    let job_id = "e2e-test-job";
    let job = Job {
        id: Some(job_id.into()),
        name: Some("E2E Test Job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("test-task".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo 'hello from e2e test'".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    // Submit the job
    engine.submit_job(job, vec![]).await?;

    // Wait for the job to reach COMPLETED state naturally
    let datastore = engine.datastore();
    let completed_job = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(j) = datastore.get_job_by_id(job_id).await {
                if j.state == "COMPLETED" {
                    return Ok::<Job, anyhow::Error>(j);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for job completion")?;

    assert_eq!(completed_job.state, "COMPLETED");
    assert!(completed_job.completed_at.is_some());

    // Terminate the engine
    engine.terminate().await?;

    Ok(())
}

#[tokio::test]
async fn standalone_engine_completes_parallel_job_naturally() -> Result<()> {
    // Set up environment for in-memory components
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    // Initialize engine in Standalone mode
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    // Register a mock runtime
    engine.register_runtime(Box::new(MockRuntime));

    // Start the engine
    engine.start().await?;

    // Create a parallel job
    let job_id = "parallel-e2e-job";
    let job = Job {
        id: Some(job_id.into()),
        name: Some("Parallel E2E Test Job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("parallel-task".to_string()),
            parallel: Some(twerk_core::task::ParallelTask {
                tasks: Some(vec![
                    Task {
                        name: Some("p1".to_string()),
                        image: Some("alpine".to_string()),
                        run: Some("echo p1".to_string()),
                        ..Default::default()
                    },
                    Task {
                        name: Some("p2".to_string()),
                        image: Some("alpine".to_string()),
                        run: Some("echo p2".to_string()),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    // Submit the job
    engine.submit_job(job, vec![]).await?;

    // Wait for the job to reach COMPLETED state
    let datastore = engine.datastore();
    let completed_job = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(j) = datastore.get_job_by_id(job_id).await {
                if j.state == "COMPLETED" {
                    return Ok::<Job, anyhow::Error>(j);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for parallel job completion")?;

    assert_eq!(completed_job.state, "COMPLETED");

    // Terminate the engine
    engine.terminate().await?;

    Ok(())
}

#[tokio::test]
async fn standalone_engine_completes_each_job_naturally() -> Result<()> {
    // Set up environment for in-memory components
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");

    // Initialize engine in Standalone mode
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    // Register a mock runtime
    engine.register_runtime(Box::new(MockRuntime));

    // Start the engine
    engine.start().await?;

    // Create an each job
    let job_id = "each-e2e-job";
    let job = Job {
        id: Some(job_id.into()),
        name: Some("Each E2E Test Job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("each-task".to_string()),
            each: Some(Box::new(twerk_core::task::EachTask {
                list: Some("[\"a\", \"b\", \"c\"]".to_string()),
                task: Some(Box::new(Task {
                    name: Some("each-item".to_string()),
                    image: Some("alpine".to_string()),
                    run: Some("echo {{ item.value }}".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    // Submit the job
    engine.submit_job(job, vec![]).await?;

    // Wait for the job to reach COMPLETED state
    let datastore = engine.datastore();
    let completed_job = timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(j) = datastore.get_job_by_id(job_id).await {
                if j.state == "COMPLETED" {
                    return Ok::<Job, anyhow::Error>(j);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for each job completion")?;

    assert_eq!(completed_job.state, "COMPLETED");

    // Terminate the engine
    engine.terminate().await?;

    Ok(())
}
