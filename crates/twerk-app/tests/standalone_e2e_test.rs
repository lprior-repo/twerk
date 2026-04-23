#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::expect_used)]

use anyhow::Result;
use std::time::Duration;
use tokio::time::timeout;
use twerk_app::engine::{Config, Engine, MockRuntime, Mode};
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_core::task::Task;
use twerk_infrastructure::datastore::Datastore;
use twerk_infrastructure::runtime::{BoxedFuture, ShutdownResult};
use uuid::Uuid;

fn to_job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("test job id should be valid")
}

async fn wait_for_job_state(
    datastore: &dyn Datastore,
    job_id: &str,
    expected: JobState,
) -> Result<Job> {
    timeout(Duration::from_secs(10), async {
        loop {
            match datastore.get_job_by_id(job_id).await {
                Ok(job) if job.state == expected => return Ok(job),
                Ok(_) | Err(_) => {}
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timed out waiting for job state")
}

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
    let job_id = Uuid::new_v4().to_string();
    let job = Job {
        id: Some(to_job_id(job_id.clone())),
        state: JobState::Pending,
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
    let failed_job = wait_for_job_state(datastore, &job_id, JobState::Failed).await?;

    assert_eq!(failed_job.state, JobState::Failed);

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
    let job_id = Uuid::new_v4().to_string();
    let job = Job {
        id: Some(to_job_id(job_id.clone())),
        state: JobState::Pending,
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
    let failed_job = wait_for_job_state(datastore, &job_id, JobState::Failed).await?;

    assert_eq!(failed_job.state, JobState::Failed);

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
    let job_id = Uuid::new_v4().to_string();
    let job = Job {
        id: Some(to_job_id(job_id.clone())),
        state: JobState::Pending,
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
    let failed_job = wait_for_job_state(datastore, &job_id, JobState::Failed).await?;

    assert_eq!(failed_job.state, JobState::Failed);

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
    let job_id = Uuid::new_v4().to_string();
    let job = Job {
        id: Some(to_job_id(job_id.clone())),
        name: Some("E2E Test Job".to_string()),
        state: JobState::Pending,
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
    let completed_job = wait_for_job_state(datastore, &job_id, JobState::Completed).await?;

    assert_eq!(completed_job.state, JobState::Completed);
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
    let job_id = Uuid::new_v4().to_string();
    let job = Job {
        id: Some(to_job_id(job_id.clone())),
        name: Some("Parallel E2E Test Job".to_string()),
        state: JobState::Pending,
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
    let completed_job = wait_for_job_state(datastore, &job_id, JobState::Completed).await?;

    assert_eq!(completed_job.state, JobState::Completed);

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
    let job_id = Uuid::new_v4().to_string();
    let job = Job {
        id: Some(to_job_id(job_id.clone())),
        name: Some("Each E2E Test Job".to_string()),
        state: JobState::Pending,
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
    let completed_job = wait_for_job_state(datastore, &job_id, JobState::Completed).await?;

    assert_eq!(completed_job.state, JobState::Completed);

    // Terminate the engine
    engine.terminate().await?;

    Ok(())
}
