#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use anyhow::Result;
use twerk_app::engine::coordinator::create_coordinator;
use twerk_app::engine::{BrokerProxy, DatastoreProxy};
use twerk_core::job::{Job, JobState};
use twerk_core::task::{Task, TaskState};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

#[tokio::test(start_paused = true)]
async fn job_completes_when_tasks_are_finished() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("test-job-2".into()),
        name: Some("test job 2".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("task 1".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    // Wait for task to be created
    let tasks = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let tasks = datastore.get_active_tasks("test-job-2").await?;
            if !tasks.is_empty() {
                return Ok::<_, anyhow::Error>(tasks);
            }
            tokio::time::advance(std::time::Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout waiting for tasks")?;

    assert_eq!(tasks.len(), 1);
    let task = tasks[0].clone();

    // Simulate task completion
    let mut completed_task = task.clone();
    completed_task.state = twerk_core::task::TaskState::Completed;
    completed_task.completed_at = Some(time::OffsetDateTime::now_utc());

    broker.publish_task_progress(&completed_task).await?;

    // Wait for job to be completed
    let persisted = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let persisted = datastore.get_job_by_id("test-job-2").await?;
            if persisted.state == JobState::Completed {
                return Ok::<_, anyhow::Error>(persisted);
            }
            tokio::time::advance(std::time::Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout waiting for job completion")?;

    assert_eq!(persisted.state, JobState::Completed);
    assert!(persisted.completed_at.is_some());

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn first_top_level_task_is_scheduled_immediately_when_job_submitted() -> Result<()> {
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("first-task-job".into()),
        name: Some("first task scheduling".to_string()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("task 1".to_string()),
            image: Some("alpine".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    let tasks = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let tasks = datastore.get_all_tasks_for_job("first-task-job").await?;
            if !tasks.is_empty() {
                return Ok::<_, anyhow::Error>(tasks);
            }
            tokio::time::advance(std::time::Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout waiting for scheduled task")?;

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].state, TaskState::Scheduled);
    assert_eq!(tasks[0].queue, Some("default".to_string()));
    assert!(tasks[0].scheduled_at.is_some());

    let persisted = datastore.get_job_by_id("first-task-job").await?;
    assert_eq!(persisted.state, JobState::Scheduled);
    assert_eq!(persisted.position, 1);

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn parallel_tasks_scheduled_when_job_submitted() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("parallel-job".into()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("parallel task".to_string()),
            parallel: Some(twerk_core::task::ParallelTask {
                tasks: Some(vec![
                    Task {
                        name: Some("p1".to_string()),
                        ..Default::default()
                    },
                    Task {
                        name: Some("p2".to_string()),
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

    coordinator.submit_job(job).await?;

    // Wait for the parallel task to be "running" and subtasks to be "pending"
    let tasks = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let tasks = datastore.get_active_tasks("parallel-job").await?;
            if tasks.len() >= 3 {
                return Ok::<_, anyhow::Error>(tasks);
            }
            tokio::time::advance(std::time::Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout waiting for parallel tasks")?;

    assert_eq!(tasks.len(), 3);

    let parallel_task = tasks.iter().find(|t| t.parallel.is_some()).unwrap();
    assert_eq!(parallel_task.state, TaskState::Running);

    let subtasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.parent_id.as_deref() == parallel_task.id.as_deref())
        .collect();
    assert_eq!(subtasks.len(), 2);
    for st in subtasks {
        assert_eq!(st.state, TaskState::Scheduled);
    }

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn each_tasks_scheduled_when_job_submitted() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let mut inputs = std::collections::HashMap::new();
    inputs.insert("list".to_string(), "[\"a\", \"b\"]".to_string());

    let job = Job {
        id: Some("each-job".into()),
        state: JobState::Pending,
        context: Some(twerk_core::job::JobContext {
            inputs: Some(inputs),
            ..Default::default()
        }),
        tasks: Some(vec![Task {
            name: Some("each task".to_string()),
            each: Some(Box::new(twerk_core::task::EachTask {
                list: Some("{{ list }}".to_string()),
                task: Some(Box::new(Task {
                    name: Some("each-item".to_string()),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    // Wait for the each task to be "running" and subtasks to be "scheduled"
    let tasks = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let tasks = datastore.get_active_tasks("each-job").await?;
            if tasks.len() >= 3 {
                return Ok::<_, anyhow::Error>(tasks);
            }
            tokio::time::advance(std::time::Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout waiting for each tasks")?;

    assert_eq!(tasks.len(), 3);

    let each_task = tasks.iter().find(|t| t.each.is_some()).unwrap();
    assert_eq!(each_task.state, TaskState::Running);

    let subtasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.parent_id.as_deref() == each_task.id.as_deref())
        .collect();
    assert_eq!(subtasks.len(), 2);
    for st in subtasks {
        assert_eq!(st.state, TaskState::Scheduled);
    }

    Ok(())
}

#[tokio::test(start_paused = true)]
async fn subjob_scheduled_when_parent_job_running() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory", Some("")).await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("parent-job".into()),
        state: JobState::Pending,
        tasks: Some(vec![Task {
            name: Some("subjob task".to_string()),
            subjob: Some(twerk_core::task::SubJobTask {
                name: Some("my subjob".to_string()),
                tasks: Some(vec![Task {
                    name: Some("st1".to_string()),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        }]),
        task_count: 1,
        ..Default::default()
    };

    coordinator.submit_job(job).await?;

    // Wait for the subjob task to be "running"
    let tasks = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let tasks = datastore.get_active_tasks("parent-job").await?;
            if !tasks.is_empty() && tasks[0].state == TaskState::Running {
                return Ok::<_, anyhow::Error>(tasks);
            }
            tokio::time::advance(std::time::Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout waiting for subjob task")?;

    assert_eq!(tasks.len(), 1);
    let subjob_task = &tasks[0];
    assert_eq!(subjob_task.state, TaskState::Running);

    let subjob_id = subjob_task
        .subjob
        .as_ref()
        .and_then(|s| s.id.clone())
        .expect("subjob id missing");

    // Verify subjob is created
    let subjob = datastore.get_job_by_id(&subjob_id).await?;
    assert_eq!(subjob.name.as_deref(), Some("my subjob"));

    // Subjob should be scheduled because coordinator handles PENDING jobs
    let persisted_sj = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let persisted = datastore.get_job_by_id(&subjob_id).await?;
            if persisted.state == JobState::Scheduled {
                return Ok::<_, anyhow::Error>(persisted);
            }
            tokio::time::advance(std::time::Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout waiting for subjob scheduling")?;

    assert_eq!(persisted_sj.state, JobState::Scheduled);

    Ok(())
}
