#![allow(clippy::needless_update)]
#![allow(clippy::to_string_in_format_args)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::float_cmp)]
#![allow(clippy::non_std_lazy_statics)]

use futures_util::FutureExt;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::postgres::Postgres;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;
use twerk_core::id::JobId;
use twerk_core::job::{Job, JobContext};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TaskLimits, TaskLogPart, TaskRetry};
use twerk_core::user::User;
use twerk_infrastructure::datastore::postgres::PostgresDatastore;
use twerk_infrastructure::datastore::{Datastore, Options};
use uuid::Uuid;

static SHARED_DSN: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

async fn setup_postgres() -> PostgresDatastore {
    let mut shared_dsn = SHARED_DSN.lock().await;
    let dsn = if let Some(dsn) = &*shared_dsn {
        dsn.clone()
    } else {
        let node = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("failed to start postgres");
        let host = node.get_host().await.expect("failed to get host");
        let port = node
            .get_host_port_ipv4(5432)
            .await
            .expect("failed to get port");
        let dsn = format!("postgres://postgres:postgres@{host}:{port}/postgres");
        *shared_dsn = Some(dsn.clone());
        // Leak the node to keep the container running for all tests
        Box::leak(Box::new(node));
        dsn
    };

    // Use a unique schema for each test to avoid interference
    let schema_name = format!("twerk{}", Uuid::new_v4().to_string().replace('-', ""));
    let dsn_with_schema = format!("{dsn}?options=-csearch_path={schema_name}");

    let ds = PostgresDatastore::new(&dsn_with_schema, Options::default())
        .await
        .expect("failed to create datastore");

    sqlx::query(&format!("CREATE SCHEMA \"{schema_name}\""))
        .execute(ds.pool().unwrap())
        .await
        .expect("failed to create schema");

    ds.exec_script(twerk_infrastructure::datastore::postgres::SCHEMA)
        .await
        .expect("failed to initialize schema");

    ds
}

async fn get_guest_user(ds: &PostgresDatastore) -> User {
    ds.get_user("guest")
        .await
        .expect("failed to get guest user")
}

fn job_id(value: impl Into<String>) -> JobId {
    JobId::new(value).expect("generated UUID should be a valid JobId")
}

fn new_job_id() -> JobId {
    job_id(twerk_core::uuid::new_short_uuid())
}

#[tokio::test]
async fn test_postgres_all() {
    let ds = setup_postgres().await;
    let guest = get_guest_user(&ds).await;
    let now = OffsetDateTime::now_utc();

    // 1. Create and get task
    let j1 = Job {
        id: Some(new_job_id()),
        created_by: Some(guest.clone()),
        tags: Some(vec![]),
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };
    ds.create_job(&j1).await.expect("failed to create job");

    let j2 = ds
        .get_job_by_id(j1.id.as_ref().unwrap())
        .await
        .expect("failed to get job");
    assert_eq!(
        j2.created_by.as_ref().unwrap().username.as_ref().unwrap(),
        "guest"
    );

    let t1 = Task {
        id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
        created_at: Some(now),
        job_id: j1.id.clone(),
        description: Some("some description".to_string()),
        networks: Some(vec!["some-network".to_string()]),
        files: Some(HashMap::from([(
            "myfile".to_string(),
            "hello world".to_string(),
        )])),
        registry: Some(twerk_core::task::Registry {
            username: Some("me".to_string()),
            password: Some("secret".to_string()),
            ..Default::default()
        }),
        gpus: Some("all".to_string()),
        r#if: Some("true".to_string()),
        tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
        workdir: Some("/some/dir".to_string()),
        priority: 2,
        ..Task::default()
    };
    ds.create_task(&t1).await.expect("failed to create task");

    let t2 = ds
        .get_task_by_id(t1.id.as_ref().unwrap())
        .await
        .expect("failed to get task");
    assert_eq!(t2.id, t1.id);
    assert_eq!(t2.description, t1.description);
    assert_eq!(t2.networks, t1.networks);
    assert_eq!(t2.files, t1.files);
    assert_eq!(
        t2.registry.as_ref().unwrap().username,
        Some("me".to_string())
    );
    assert_eq!(
        t2.registry.as_ref().unwrap().password,
        Some("secret".to_string())
    );
    assert_eq!(t2.gpus, Some("all".to_string()));
    assert_eq!(t2.r#if, Some("true".to_string()));
    assert_eq!(t2.tags, t1.tags);
    assert_eq!(t2.workdir, Some("/some/dir".to_string()));
    assert_eq!(t2.priority, 2);

    // 2. Parallel task
    let t_para = Task {
        id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
        created_at: Some(now),
        job_id: j1.id.clone(),
        parallel: Some(twerk_core::task::ParallelTask {
            tasks: Some(vec![
                Task {
                    name: Some("parallel task1".to_string()),
                    ..Task::default()
                },
                Task {
                    name: Some("parallel task2".to_string()),
                    ..Task::default()
                },
            ]),
            ..Default::default()
        }),
        ..Task::default()
    };
    ds.create_task(&t_para)
        .await
        .expect("failed to create parallel task");
    let t_para2 = ds
        .get_task_by_id(t_para.id.as_ref().unwrap())
        .await
        .expect("failed to get parallel task");
    assert!(t_para2.parallel.is_some());
    assert_eq!(t_para2.parallel.unwrap().tasks.unwrap().len(), 2);

    // 3. Active tasks
    let states = vec![
        twerk_core::task::TASK_STATE_PENDING,
        twerk_core::task::TASK_STATE_SCHEDULED,
        twerk_core::task::TASK_STATE_RUNNING,
        twerk_core::task::TASK_STATE_CANCELLED,
        twerk_core::task::TASK_STATE_COMPLETED,
        twerk_core::task::TASK_STATE_FAILED,
    ];
    let j_active = Job {
        id: Some(new_job_id()),
        created_by: Some(guest.clone()),
        tags: Some(vec![]),
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };
    ds.create_job(&j_active)
        .await
        .expect("failed to create j_active");
    for state in states {
        let t = Task {
            id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
            state: state.parse().unwrap_or_default(),
            created_at: Some(now),
            job_id: j_active.id.clone(),
            ..Task::default()
        };
        ds.create_task(&t).await.expect("failed to create task");
    }
    let at = ds
        .get_active_tasks(j_active.id.as_ref().unwrap())
        .await
        .expect("failed to get active tasks");
    assert_eq!(at.len(), 3);

    // 4. Update task
    ds.update_task(
        t1.id.as_ref().unwrap(),
        Box::new(|mut u| {
            u.state = twerk_core::task::TaskState::Scheduled;
            u.result = Some("my result".to_string());
            u.queue = Some("somequeue".to_string());
            u.progress = 57.3;
            u.priority = 5;
            Ok(u)
        }),
    )
    .await
    .expect("failed to update task");
    let t1_updated = ds
        .get_task_by_id(t1.id.as_ref().unwrap())
        .await
        .expect("failed to get task");
    assert_eq!(t1_updated.state, twerk_core::task::TaskState::Scheduled);
    assert_eq!(t1_updated.progress, 57.3);
    assert_eq!(t1_updated.priority, 5);
    assert_eq!(t1_updated.queue, Some("somequeue".to_string()));

    ds.update_task(
        t1.id.as_ref().unwrap(),
        Box::new(|mut u| {
            u.limits = Some(TaskLimits {
                cpus: Some("2".to_string()),
                memory: Some("1g".to_string()),
            });
            u.timeout = Some("45s".to_string());
            u.retry = Some(TaskRetry {
                attempts: 1,
                limit: 4,
            });
            Ok(u)
        }),
    )
    .await
    .expect("failed to update scheduling fields");
    let t1_scheduling = ds
        .get_task_by_id(t1.id.as_ref().unwrap())
        .await
        .expect("failed to get updated scheduling fields");
    assert_eq!(
        t1_scheduling
            .limits
            .as_ref()
            .and_then(|limits| limits.cpus.clone()),
        Some("2".to_string())
    );
    assert_eq!(
        t1_scheduling
            .limits
            .as_ref()
            .and_then(|limits| limits.memory.clone()),
        Some("1g".to_string())
    );
    assert_eq!(t1_scheduling.timeout, Some("45s".to_string()));
    assert_eq!(
        t1_scheduling.retry.as_ref().map(|retry| retry.limit),
        Some(4)
    );

    // 4b. Test cascading cleanup
    let j_cascade_id = twerk_core::uuid::new_short_uuid();
    let t_cascade_id = Uuid::new_v4().to_string().replace('-', "");
    let l_cascade_id = Uuid::new_v4().to_string().replace('-', "");
    let j_cascade = Job {
        id: Some(job_id(j_cascade_id.clone())),
        created_by: Some(guest.clone()),
        tags: Some(vec![]),
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };
    ds.create_job(&j_cascade)
        .await
        .expect("failed to create job_cascade");
    let t_cascade = Task {
        id: Some(t_cascade_id.clone().into()),
        job_id: Some(job_id(j_cascade_id.clone())),
        created_at: Some(now),
        ..Task::default()
    };
    ds.create_task(&t_cascade)
        .await
        .expect("failed to create task_cascade");
    let l_cascade = TaskLogPart {
        id: Some(l_cascade_id.clone()),
        task_id: Some(t_cascade_id.clone().into()),
        number: 1,
        contents: Some("log message".to_string()),
        created_at: Some(now),
    };
    ds.create_task_log_part(&l_cascade)
        .await
        .expect("failed to create log_cascade");

    // Delete job manually (simulating cleanup)
    sqlx::query(&format!("DELETE FROM jobs WHERE id = '{j_cascade_id}'"))
        .execute(ds.pool().unwrap())
        .await
        .expect("failed to delete job");

    // Check if task and logs are gone
    let t_res = ds.get_task_by_id(&t_cascade_id).await;
    assert!(t_res.is_err(), "task should be deleted by cascade");
    let l_res = ds
        .get_task_log_parts(&t_cascade_id, "", 1, 10)
        .await
        .expect("get logs failed");
    assert_eq!(l_res.items.len(), 0, "logs should be deleted by cascade");

    // 5. Create job with user
    let u = User {
        id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
        username: Some(format!("u{}", Uuid::new_v4().to_string()[..8].to_string())),
        name: Some("Tester".to_string()),
        created_at: Some(now),
        password_hash: Some("hash".to_string()),
        ..User::default()
    };
    ds.create_user(&u).await.expect("failed to create user");
    let j_u = Job {
        id: Some(new_job_id()),
        created_by: Some(u.clone()),
        tags: Some(vec!["tag-a".to_string(), "tag-b".to_string()]),
        auto_delete: Some(twerk_core::task::AutoDelete {
            after: Some("5h".to_string()),
        }),
        secrets: Some(HashMap::from([(
            "password".to_string(),
            "secret".to_string(),
        )])),
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };
    ds.create_job(&j_u).await.expect("failed to create j_u");
    let j_u2 = ds
        .get_job_by_id(j_u.id.as_ref().unwrap())
        .await
        .expect("failed to get j_u");
    assert_eq!(j_u2.created_by.as_ref().unwrap().username, u.username);
    assert_eq!(j_u2.tags.as_ref().unwrap().len(), 2);

    // 6. Node CRUD
    let n1 = Node {
        id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
        name: Some("some node".to_string()),
        hostname: Some("some-name".to_string()),
        port: Some(1234),
        version: Some("1.0.0".to_string()),
        queue: Some("default".to_string()),
        status: Some(NodeStatus::UP),
        started_at: Some(now),
        last_heartbeat_at: Some(now),
        ..Node::default()
    };
    ds.create_node(&n1).await.expect("failed to create node");
    let n2 = ds
        .get_node_by_id(n1.id.as_ref().unwrap())
        .await
        .expect("failed to get node");
    assert_eq!(n2.id, n1.id);
    ds.update_node(
        n1.id.as_ref().unwrap(),
        Box::new(move |mut u| {
            u.last_heartbeat_at = Some(now);
            u.task_count = Some(2);
            Ok(u)
        }),
    )
    .await
    .expect("failed to update node");
    let n2_updated = ds
        .get_node_by_id(n1.id.as_ref().unwrap())
        .await
        .expect("failed to get node");
    assert_eq!(n2_updated.task_count, Some(2));

    // 7. Active nodes
    let n_active = Node {
        id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
        status: Some(NodeStatus::UP),
        last_heartbeat_at: Some(OffsetDateTime::now_utc() - Duration::seconds(20)),
        name: Some("n_active".to_string()),
        queue: Some("q".to_string()),
        hostname: Some("h_active".to_string()),
        started_at: Some(now),
        version: Some("1.0.0".to_string()),
        ..Node::default()
    };
    ds.create_node(&n_active)
        .await
        .expect("failed to create n_active");
    let ns = ds
        .get_active_nodes()
        .await
        .expect("failed to get active nodes");
    assert!(ns.iter().any(|n| n.id == n_active.id));

    // 8. Pagination
    for i in 0..15 {
        let j = Job {
            id: Some(new_job_id()),
            name: Some(format!("Job {i}")),
            created_by: Some(guest.clone()),
            tags: Some(vec![]),
            created_at: Some(now),
            tasks: Some(vec![]),
            inputs: Some(HashMap::new()),
            context: Some(JobContext::default()),
            ..Job::default()
        };
        ds.create_job(&j).await.expect("failed to create pag job");
    }
    let p1 = ds.get_jobs("", "", 1, 10).await.expect("failed to get p1");
    assert_eq!(p1.items.len(), 10);
    assert!(p1.total_items >= 15);

    // 9. Task logs
    let part1 = TaskLogPart {
        id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
        task_id: t1.id.clone(),
        number: 1,
        contents: Some("line 1".to_string()),
        created_at: Some(OffsetDateTime::now_utc()),
    };
    ds.create_task_log_part(&part1)
        .await
        .expect("failed to create log part");
    let logs = ds
        .get_task_log_parts(t1.id.as_ref().unwrap(), "", 1, 10)
        .await
        .expect("failed to get logs");
    assert_eq!(logs.items.len(), 1);

    // 10. Health check and Metrics
    ds.health_check().await.expect("health check failed");
    let metrics = ds.get_metrics().await.expect("failed to get metrics");
    assert!(metrics.jobs.running >= 0);

    // 11. Search
    let j_search = Job {
        id: Some(new_job_id()),
        name: Some("Searchable Job".to_string()),
        description: Some("This is a searchable description".to_string()),
        created_by: Some(guest.clone()),
        tags: Some(vec!["search-tag".to_string()]),
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };
    ds.create_job(&j_search)
        .await
        .expect("failed to create search job");

    let p_search = ds
        .get_jobs("", "Searchable", 1, 10)
        .await
        .expect("failed to search by name");
    assert!(p_search.items.iter().any(|j| j.id == j_search.id));

    let p_tag = ds
        .get_jobs("", "tag:search-tag", 1, 10)
        .await
        .expect("failed to search by tag");
    assert!(p_tag.items.iter().any(|j| j.id == j_search.id));

    // 12. Scheduled Jobs
    let sj = twerk_core::job::ScheduledJob {
        id: Some(Uuid::new_v4().to_string().replace('-', "").into()),
        name: Some("Test Scheduled Job".to_string()),
        description: Some("Test description".to_string()),
        cron: Some("* * * * *".to_string()),
        created_by: Some(guest.clone()),
        state: twerk_core::job::ScheduledJobState::Active,
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        tags: Some(vec![]),
        output: Some(String::new()),
        ..Default::default()
    };
    ds.create_scheduled_job(&sj)
        .await
        .expect("failed to create scheduled job");

    let sj2 = ds
        .get_scheduled_job_by_id(sj.id.as_ref().unwrap())
        .await
        .expect("failed to get scheduled job");
    assert_eq!(sj2.name, sj.name);

    let active_sjs = ds
        .get_active_scheduled_jobs()
        .await
        .expect("failed to get active scheduled jobs");
    assert!(active_sjs.iter().any(|s| s.id == sj.id));

    // 13. Retention/Cleanup
    let job_expired_id = twerk_core::uuid::new_short_uuid();
    let j_expired = Job {
        id: Some(job_id(job_expired_id.clone())),
        created_by: Some(guest.clone()),
        state: twerk_core::job::JobState::Completed,
        created_at: Some(now - Duration::days(400)),
        completed_at: Some(now - Duration::days(400)),
        tags: Some(vec![]),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };
    ds.create_job(&j_expired)
        .await
        .expect("failed to create expired job");

    // Verify it exists before cleanup
    ds.get_job_by_id(&job_expired_id)
        .await
        .expect("job should exist before cleanup");

    ds.cleanup().await.expect("cleanup failed");

    let res = ds.get_job_by_id(&job_expired_id).await;
    assert!(res.is_err()); // Should be deleted

    // 14. Transactions
    let job_tx_id = twerk_core::uuid::new_short_uuid();
    let j_tx = Job {
        id: Some(job_id(job_tx_id.clone())),
        created_by: Some(guest.clone()),
        tags: Some(vec![]),
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };

    let ds_clone = ds.clone();
    let res = ds
        .with_tx(Box::new(move |tx| {
            let j_tx_clone = j_tx.clone();
            async move {
                tx.create_job(&j_tx_clone).await?;
                // Intentionally fail to trigger rollback
                Err(twerk_infrastructure::datastore::Error::InvalidInput(
                    "forced failure".to_string(),
                ))
            }
            .boxed()
        }))
        .await;

    assert!(res.is_err());
    let res_get = ds_clone.get_job_by_id(&job_tx_id).await;
    assert!(res_get.is_err()); // Should NOT exist due to rollback

    // 15. Concurrency
    let job_conc_id = twerk_core::uuid::new_short_uuid();
    let j_conc = Job {
        id: Some(job_id(job_conc_id.clone())),
        created_by: Some(guest.clone()),
        tags: Some(vec![]),
        created_at: Some(now),
        tasks: Some(vec![]),
        inputs: Some(HashMap::new()),
        context: Some(JobContext::default()),
        ..Job::default()
    };
    ds.create_job(&j_conc)
        .await
        .expect("failed to create job_conc");

    let mut handles = vec![];
    for _ in 0..5 {
        let ds_c = ds.clone();
        let job_conc_id_c = job_conc_id.clone();
        handles.push(tokio::spawn(async move {
            ds_c.update_job(
                &job_conc_id_c,
                Box::new(move |mut j| {
                    j.position += 1;
                    Ok(j)
                }),
            )
            .await
            .expect("failed to update job concurrently");
        }));
    }
    for h in handles {
        h.await.expect("task panicked");
    }
    let j_conc2 = ds
        .get_job_by_id(&job_conc_id)
        .await
        .expect("failed to get job_conc");
    assert_eq!(j_conc2.position, 5);
}
