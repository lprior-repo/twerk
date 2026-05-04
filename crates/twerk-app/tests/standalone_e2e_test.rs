#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::expect_used)]

use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;
use twerk_app::engine::{Config, Engine, JobListener, MockRuntime, Mode};
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_core::task::Task;
use twerk_infrastructure::runtime::{BoxedFuture, ShutdownResult};
use uuid::Uuid;

fn to_job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("test job id should be valid")
}

fn listeners_for_job_state(expected_state: JobState) -> (Vec<JobListener>, oneshot::Receiver<Job>) {
    let (tx, rx) = oneshot::channel();
    let sender = Arc::new(Mutex::new(Some(tx)));

    let listener: JobListener = Arc::new(move |job| {
        if job.state == expected_state {
            let mut guard = sender
                .lock()
                .expect("job state listener lock should not be poisoned");

            if let Some(tx) = guard.take() {
                drop(tx.send(job));
            }
        }
    });

    (vec![listener], rx)
}

async fn submit_job_and_wait_for_state(
    engine: &Engine,
    job: Job,
    job_id: &str,
    expected_state: JobState,
    timeout_message: &str,
) -> Result<Job> {
    let (listeners, receiver) = listeners_for_job_state(expected_state);
    engine.submit_job(job, listeners).await?;
    timeout(Duration::from_secs(10), receiver)
        .await
        .expect(timeout_message)
        .expect("job state listener should receive the terminal job");
    engine
        .datastore()
        .get_job_by_id(job_id)
        .await
        .map_err(Into::into)
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
    let failed_job = submit_job_and_wait_for_state(
        &engine,
        job,
        &job_id,
        JobState::Failed,
        "timeout waiting for job failure",
    )
    .await?;

    assert_eq!(failed_job.state, JobState::Failed);

    // Terminate the engine
    engine.terminate().await?;

    Ok(())
}

#[tokio::test]
async fn standalone_engine_retrieves_logs_for_failed_shell_task() -> Result<()> {
    // Set up environment for in-memory components and shell runtime
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    std::env::set_var("TWERK_RUNTIME_TYPE", "shell");
    std::env::set_var("TWERK_RUNTIME_SHELL_CMD", "bash,-c");

    // Initialize engine in Standalone mode
    let mut config = Config::default();
    config.mode = Mode::Standalone;
    let mut engine = Engine::new(config);

    // Start the engine (shell runtime auto-created from env)
    engine.start().await?;

    // Create a job with a task that produces output and fails
    let job_id = Uuid::new_v4().to_string();
    let job = Job {
        id: Some(to_job_id(job_id.clone())),
        name: Some("Failed Shell Task Log Test".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("failing-shell-task".to_string()),
            run: Some("echo 'stdout-line' && echo 'stderr-line' >&2 && exit 42".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    // Submit and wait for failure
    let failed_job = submit_job_and_wait_for_state(
        &engine,
        job,
        &job_id,
        JobState::Failed,
        "timeout waiting for shell task failure",
    )
    .await?;

    assert_eq!(failed_job.state, JobState::Failed);

    // Retrieve the task from the datastore
    let tasks = engine.datastore().get_all_tasks_for_job(&job_id).await?;
    assert_eq!(tasks.len(), 1, "should have exactly one task");
    let task = &tasks[0];
    let task_id = task.id.as_ref().expect("task should have id");

    // Query task logs from the datastore — must be available immediately
    // because InMemoryBroker::task_log_part now awaits handlers directly.
    let log_parts = engine
        .datastore()
        .get_task_log_parts(task_id.as_ref(), "", 1, 100)
        .await?;

    assert!(
        !log_parts.items.is_empty(),
        "task logs should not be empty for failed shell task; got 0 parts"
    );

    let combined: String = log_parts
        .items
        .iter()
        .filter_map(|p| p.contents.clone())
        .collect();
    assert!(
        combined.contains("stdout-line"),
        "logs should contain stdout output: {combined}"
    );
    assert!(
        combined.contains("stderr-line"),
        "logs should contain stderr output: {combined}"
    );

    // Terminate the engine
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
    let failed_job = submit_job_and_wait_for_state(
        &engine,
        job,
        &job_id,
        JobState::Failed,
        "timeout waiting for parallel job failure",
    )
    .await?;

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
    let completed_job = submit_job_and_wait_for_state(
        &engine,
        job,
        &job_id,
        JobState::Completed,
        "timeout waiting for job completion",
    )
    .await?;

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
    let completed_job = submit_job_and_wait_for_state(
        &engine,
        job,
        &job_id,
        JobState::Completed,
        "timeout waiting for parallel job completion",
    )
    .await?;

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
    let completed_job = submit_job_and_wait_for_state(
        &engine,
        job,
        &job_id,
        JobState::Completed,
        "timeout waiting for each job completion",
    )
    .await?;

    assert_eq!(completed_job.state, JobState::Completed);

    // Terminate the engine
    engine.terminate().await?;

    Ok(())
}
