#![allow(clippy::unwrap_used)]

use anyhow::Result;
use std::sync::Arc;
use twerk_app::engine::{Config, Engine, Mode, State};
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobState};
use twerk_core::task::Task;
use twerk_core::task::TaskState;

fn engine_standalone() -> Engine {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");
    std::env::set_var("TWERK_BROKER_TYPE", "inmemory");
    Engine::new(Config {
        mode: Mode::Standalone,
        ..Default::default()
    })
}

fn to_job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("test job id should be valid")
}

#[tokio::test]
async fn engine_graceful_shutdown_cancels_pending_tasks() -> Result<()> {
    let mut engine = engine_standalone();
    engine.start().await?;
    assert_eq!(engine.state(), State::Running);

    let job_ids: Vec<JobId> = (0..5)
        .map(|i| to_job_id(format!("shutdown-test-job-{}", i)))
        .collect();

    for job_id in &job_ids {
        let job = Job {
            id: Some(job_id.clone()),
            state: JobState::Pending,
            tasks: Some(vec![Task {
                name: Some(format!("shutdown-test-task-{}", job_id)),
                image: Some("alpine".to_string()),
                run: Some("sleep 60".to_string()),
                ..Default::default()
            }]),
            task_count: 1,
            ..Default::default()
        };
        let result = engine.submit_job(job, vec![]).await;
        assert!(result.is_ok(), "submit_job should succeed for job {}", job_id);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    engine.terminate().await?;
    assert_eq!(engine.state(), State::Terminated);

    let broker = engine.broker_proxy();
    let queues = broker.queues().await?;

    for queue_info in queues {
        assert_eq!(
            queue_info.size, 0,
            "queue '{}' should be empty after shutdown, but has {} messages",
            queue_info.name, queue_info.size
        );
    }

    Ok(())
}

#[tokio::test]
async fn engine_shutdown_returns_terminated_state() -> Result<()> {
    let mut engine = engine_standalone();
    engine.start().await?;
    assert_eq!(engine.state(), State::Running);

    let job = Job {
        id: Some(to_job_id("shutdown-state-test-job")),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("test task".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    engine.submit_job(job, vec![]).await?;

    engine.terminate().await?;
    assert_eq!(engine.state(), State::Terminated);

    let result = engine.terminate().await;
    assert!(result.is_err(), "terminate should fail when engine is not running");

    Ok(())
}

#[tokio::test]
async fn engine_shutdown_no_resource_leak() -> Result<()> {
    let mut engine = engine_standalone();
    engine.start().await?;

    for i in 0..5 {
        let job = Job {
            id: Some(to_job_id(format!("resource-leak-test-job-{}", i))),
            state: JobState::Pending,
            tasks: Some(vec![Task {
                name: Some(format!("task-{}", i)),
                image: Some("alpine".to_string()),
                run: Some("echo hello".to_string()),
                ..Default::default()
            }]),
            task_count: 1,
            ..Default::default()
        };
        engine.submit_job(job, vec![]).await?;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    engine.terminate().await?;

    assert_eq!(engine.state(), State::Terminated);

    let job_ids: Vec<String> = (0..5)
        .map(|i| format!("resource-leak-test-job-{}", i))
        .collect();

    for job_id in &job_ids {
        let ds = engine.datastore_proxy();
        let tasks_result = ds.get_all_tasks_for_job(job_id).await;
        if let Ok(tasks) = tasks_result {
            for task in tasks {
                assert!(
                    !task.state.is_active(),
                    "task {} should not be in active state after shutdown, but is {:?}",
                    job_id,
                    task.state
                );
            }
        }
    }

    Ok(())
}