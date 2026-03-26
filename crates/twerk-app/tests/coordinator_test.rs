use anyhow::Result;
use std::sync::Arc;
use twerk_app::engine::coordinator::{create_coordinator, Coordinator};
use twerk_app::engine::{BrokerProxy, DatastoreProxy};
use twerk_core::job::{Job, JOB_STATE_PENDING, JOB_STATE_SCHEDULED};
use twerk_core::task::Task;
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;

#[tokio::test]
async fn test_job_completion() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory").await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("test-job-2".to_string()),
        name: Some("test job 2".to_string()),
        state: JOB_STATE_PENDING.to_string(),
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
    let mut attempts = 0;
    let mut tasks = datastore.get_active_tasks("test-job-2").await?;
    while tasks.is_empty() && attempts < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tasks = datastore.get_active_tasks("test-job-2").await?;
        attempts += 1;
    }

    assert_eq!(tasks.len(), 1);
    let task = tasks[0].clone();

    // Simulate task completion
    let mut completed_task = task.clone();
    completed_task.state = twerk_core::task::TASK_STATE_COMPLETED.to_string();
    completed_task.completed_at = Some(time::OffsetDateTime::now_utc());

    broker.publish_task_progress(&completed_task).await?;

    // Wait for job to be completed
    let mut persisted = datastore.get_job_by_id("test-job-2").await?;
    attempts = 0;
    while persisted.state != "COMPLETED" && attempts < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        persisted = datastore.get_job_by_id("test-job-2").await?;
        attempts += 1;
    }

    assert_eq!(persisted.state, "COMPLETED");
    assert!(persisted.completed_at.is_some());

    Ok(())
}

#[tokio::test]
async fn test_parallel_scheduling() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory").await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("parallel-job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
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
    let mut attempts = 0;
    let mut tasks = datastore.get_active_tasks("parallel-job").await?;
    // We expect 3 active tasks: 1 parallel (running) + 2 subtasks (pending)
    while tasks.len() < 3 && attempts < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tasks = datastore.get_active_tasks("parallel-job").await?;
        attempts += 1;
    }

    assert_eq!(tasks.len(), 3);

    let parallel_task = tasks.iter().find(|t| t.parallel.is_some()).unwrap();
    assert_eq!(parallel_task.state, "RUNNING");

    let subtasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.parent_id.as_deref() == parallel_task.id.as_deref())
        .collect();
    assert_eq!(subtasks.len(), 2);
    for st in subtasks {
        assert_eq!(st.state, "SCHEDULED");
    }

    Ok(())
}

#[tokio::test]
async fn test_each_scheduling() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory").await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let mut context_map = std::collections::HashMap::new();
    context_map.insert("list".to_string(), serde_json::json!(["a", "b"]));

    let job = Job {
        id: Some("each-job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
        context: Some(twerk_core::job::JobContext {
            inputs: Some(std::collections::HashMap::new()), // not used for eval in this test
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

    // We need to inject "list" into the job context as a Value
    // But JobContext only takes HashMap<String, String>.
    // Wait! evaluate_task takes HashMap<String, Value> as context.
    // In start_job:
    // let job_ctx = job.context.as_ref().map(|c| c.as_map()).unwrap_or_default();

    // So if I put it in JobContext.inputs, it will be in the map as Value::String.
    let mut inputs = std::collections::HashMap::new();
    inputs.insert("list".to_string(), "[\"a\", \"b\"]".to_string());

    let mut job = job;
    job.context.as_mut().unwrap().inputs = Some(inputs);

    coordinator.submit_job(job).await?;

    // Wait for the each task to be "running" and subtasks to be "scheduled"
    let mut attempts = 0;
    let mut tasks = datastore.get_active_tasks("each-job").await?;
    while tasks.len() < 3 && attempts < 10 {
        // If empty, check why
        if tasks.is_empty() {
            let all_tasks = datastore.get_active_tasks("each-job").await?;
            println!("Current tasks: {:?}", all_tasks);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tasks = datastore.get_active_tasks("each-job").await?;
        attempts += 1;
    }

    // Check if task failed
    let all_tasks = datastore.get_active_tasks("each-job").await?;
    if all_tasks.len() < 3 {
        // Maybe it's not active because it failed?
        // Check all tasks for the job
        // I don't have get_all_tasks, but I can check by id if I knew it.
    }

    assert_eq!(tasks.len(), 3);

    let each_task = tasks.iter().find(|t| t.each.is_some()).unwrap();
    assert_eq!(each_task.state, "RUNNING");

    let subtasks: Vec<_> = tasks
        .iter()
        .filter(|t| t.parent_id.as_deref() == each_task.id.as_deref())
        .collect();
    assert_eq!(subtasks.len(), 2);
    for st in subtasks {
        assert_eq!(st.state, "SCHEDULED");
    }

    Ok(())
}

#[tokio::test]
async fn test_subjob_scheduling() -> Result<()> {
    // Set up in-memory datastore
    std::env::set_var("TWERK_DATASTORE_TYPE", "inmemory");

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.init("inmemory").await?;
    datastore.init().await?;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("parent-job".to_string()),
        state: JOB_STATE_PENDING.to_string(),
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
    let mut attempts = 0;
    let mut tasks = datastore.get_active_tasks("parent-job").await?;
    while (tasks.is_empty() || tasks[0].state != "RUNNING") && attempts < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tasks = datastore.get_active_tasks("parent-job").await?;
        attempts += 1;
    }

    assert_eq!(tasks.len(), 1);
    let subjob_task = &tasks[0];
    assert_eq!(subjob_task.state, "RUNNING");

    let subjob_id = subjob_task
        .subjob
        .as_ref()
        .and_then(|s| s.id.clone())
        .expect("subjob id missing");

    // Verify subjob is created
    let subjob = datastore.get_job_by_id(&subjob_id).await?;
    assert_eq!(subjob.name.as_deref(), Some("my subjob"));

    // Subjob should be scheduled because coordinator handles PENDING jobs
    attempts = 0;
    let mut persisted_sj = subjob;
    while persisted_sj.state == "PENDING" && attempts < 10 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        persisted_sj = datastore.get_job_by_id(&subjob_id).await?;
        attempts += 1;
    }
    assert_eq!(persisted_sj.state, "SCHEDULED");

    Ok(())
}
