use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use twerk_app::engine::coordinator::{create_coordinator, Coordinator};
use twerk_app::engine::{BrokerProxy, DatastoreProxy};
use twerk_core::job::{Job, JOB_STATE_PENDING};
use twerk_core::task::Task;
use twerk_infrastructure::broker::{
    BoxedFuture, Broker, EventHandler, HeartbeatHandler, JobHandler, QueueInfo,
    TaskHandler, TaskLogPartHandler, TaskProgressHandler,
};
use twerk_infrastructure::broker::inmemory::InMemoryBroker;
use twerk_infrastructure::datastore::{
    inmemory::InMemoryDatastore, Datastore, Error as DatastoreError, Page,
    Result as DatastoreResult,
};

#[derive(Clone, Default, Debug)]
struct FailConfig {
    fail_create_job: bool,
    fail_publish_job: bool,
    fail_create_task: bool,
    fail_update_job: bool,
    fail_publish_task: bool,
}

struct FailableDatastore {
    inner: InMemoryDatastore,
    config: Arc<RwLock<FailConfig>>,
}

#[async_trait]
impl Datastore for FailableDatastore {
    async fn create_task(&self, task: &Task) -> DatastoreResult<()> {
        if self.config.read().await.fail_create_task {
            return Err(DatastoreError::Database("simulated create_task failure".into()));
        }
        self.inner.create_task(task).await
    }
    async fn update_task(&self, id: &str, modify: Box<dyn FnOnce(Task) -> DatastoreResult<Task> + Send>) -> DatastoreResult<()> {
        self.inner.update_task(id, modify).await
    }
    async fn get_task_by_id(&self, id: &str) -> DatastoreResult<Task> {
        self.inner.get_task_by_id(id).await
    }
    async fn get_active_tasks(&self, job_id: &str) -> DatastoreResult<Vec<Task>> {
        self.inner.get_active_tasks(job_id).await
    }
    async fn get_next_task(&self, parent_task_id: &str) -> DatastoreResult<Task> {
        self.inner.get_next_task(parent_task_id).await
    }
    async fn create_task_log_part(&self, part: &twerk_core::task::TaskLogPart) -> DatastoreResult<()> {
        self.inner.create_task_log_part(part).await
    }
    async fn get_task_log_parts(&self, task_id: &str, q: &str, page: i64, size: i64) -> DatastoreResult<Page<twerk_core::task::TaskLogPart>> {
        self.inner.get_task_log_parts(task_id, q, page, size).await
    }
    async fn create_node(&self, node: &twerk_core::node::Node) -> DatastoreResult<()> {
        self.inner.create_node(node).await
    }
    async fn update_node(&self, id: &str, modify: Box<dyn FnOnce(twerk_core::node::Node) -> DatastoreResult<twerk_core::node::Node> + Send>) -> DatastoreResult<()> {
        self.inner.update_node(id, modify).await
    }
    async fn get_node_by_id(&self, id: &str) -> DatastoreResult<twerk_core::node::Node> {
        self.inner.get_node_by_id(id).await
    }
    async fn get_active_nodes(&self) -> DatastoreResult<Vec<twerk_core::node::Node>> {
        self.inner.get_active_nodes().await
    }
    async fn create_job(&self, job: &Job) -> DatastoreResult<()> {
        if self.config.read().await.fail_create_job {
            return Err(DatastoreError::Database("simulated create_job failure".into()));
        }
        self.inner.create_job(job).await
    }
    async fn update_job(&self, id: &str, modify: Box<dyn FnOnce(Job) -> DatastoreResult<Job> + Send>) -> DatastoreResult<()> {
        if self.config.read().await.fail_update_job {
            return Err(DatastoreError::Database("simulated update_job failure".into()));
        }
        self.inner.update_job(id, modify).await
    }
    async fn get_job_by_id(&self, id: &str) -> DatastoreResult<Job> {
        self.inner.get_job_by_id(id).await
    }
    async fn get_job_log_parts(&self, job_id: &str, q: &str, page: i64, size: i64) -> DatastoreResult<Page<twerk_core::task::TaskLogPart>> {
        self.inner.get_job_log_parts(job_id, q, page, size).await
    }
    async fn get_jobs(&self, current_user: &str, q: &str, page: i64, size: i64) -> DatastoreResult<Page<twerk_core::job::JobSummary>> {
        self.inner.get_jobs(current_user, q, page, size).await
    }
    async fn create_scheduled_job(&self, sj: &twerk_core::job::ScheduledJob) -> DatastoreResult<()> {
        self.inner.create_scheduled_job(sj).await
    }
    async fn get_active_scheduled_jobs(&self) -> DatastoreResult<Vec<twerk_core::job::ScheduledJob>> {
        self.inner.get_active_scheduled_jobs().await
    }
    async fn get_scheduled_jobs(&self, current_user: &str, page: i64, size: i64) -> DatastoreResult<Page<twerk_core::job::ScheduledJobSummary>> {
        self.inner.get_scheduled_jobs(current_user, page, size).await
    }
    async fn get_scheduled_job_by_id(&self, id: &str) -> DatastoreResult<twerk_core::job::ScheduledJob> {
        self.inner.get_scheduled_job_by_id(id).await
    }
    async fn update_scheduled_job(&self, id: &str, modify: Box<dyn FnOnce(twerk_core::job::ScheduledJob) -> DatastoreResult<twerk_core::job::ScheduledJob> + Send>) -> DatastoreResult<()> {
        self.inner.update_scheduled_job(id, modify).await
    }
    async fn delete_scheduled_job(&self, id: &str) -> DatastoreResult<()> {
        self.inner.delete_scheduled_job(id).await
    }
    async fn create_user(&self, user: &twerk_core::user::User) -> DatastoreResult<()> {
        self.inner.create_user(user).await
    }
    async fn get_user(&self, username: &str) -> DatastoreResult<twerk_core::user::User> {
        self.inner.get_user(username).await
    }
    async fn create_role(&self, role: &twerk_core::role::Role) -> DatastoreResult<()> {
        self.inner.create_role(role).await
    }
    async fn get_role(&self, id: &str) -> DatastoreResult<twerk_core::role::Role> {
        self.inner.get_role(id).await
    }
    async fn get_roles(&self) -> DatastoreResult<Vec<twerk_core::role::Role>> {
        self.inner.get_roles().await
    }
    async fn get_user_roles(&self, user_id: &str) -> DatastoreResult<Vec<twerk_core::role::Role>> {
        self.inner.get_user_roles(user_id).await
    }
    async fn assign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        self.inner.assign_role(user_id, role_id).await
    }
    async fn unassign_role(&self, user_id: &str, role_id: &str) -> DatastoreResult<()> {
        self.inner.unassign_role(user_id, role_id).await
    }
    async fn get_metrics(&self) -> DatastoreResult<twerk_core::stats::Metrics> {
        self.inner.get_metrics().await
    }
    async fn with_tx(&self, f: Box<dyn for<'a> FnOnce(&'a dyn Datastore) -> futures_util::future::BoxFuture<'a, DatastoreResult<()>> + Send>) -> DatastoreResult<()> {
        self.inner.with_tx(f).await
    }
    async fn health_check(&self) -> DatastoreResult<()> {
        self.inner.health_check().await
    }
}

struct FailableBroker {
    inner: InMemoryBroker,
    config: Arc<RwLock<FailConfig>>,
}

impl Broker for FailableBroker {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let config = Arc::clone(&self.config);
        let inner_fut = self.inner.publish_task(qname, task);
        Box::pin(async move {
            if config.read().await.fail_publish_task {
                return Err(anyhow!("simulated publish_task failure"));
            }
            inner_fut.await
        })
    }
    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
        self.inner.subscribe_for_tasks(qname, handler)
    }
    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()> {
        self.inner.publish_task_progress(task)
    }
    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()> {
        self.inner.subscribe_for_task_progress(handler)
    }
    fn publish_heartbeat(&self, node: twerk_core::node::Node) -> BoxedFuture<()> {
        self.inner.publish_heartbeat(node)
    }
    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()> {
        self.inner.subscribe_for_heartbeats(handler)
    }
    fn publish_job(&self, job: &Job) -> BoxedFuture<()> {
        let config = Arc::clone(&self.config);
        let inner_fut = self.inner.publish_job(job);
        Box::pin(async move {
            if config.read().await.fail_publish_job {
                return Err(anyhow!("simulated publish_job failure"));
            }
            inner_fut.await
        })
    }
    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()> {
        self.inner.subscribe_for_jobs(handler)
    }
    fn publish_event(&self, topic: String, event: serde_json::Value) -> BoxedFuture<()> {
        self.inner.publish_event(topic, event)
    }
    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()> {
        self.inner.subscribe_for_events(pattern, handler)
    }
    fn publish_task_log_part(&self, part: &twerk_core::task::TaskLogPart) -> BoxedFuture<()> {
        self.inner.publish_task_log_part(part)
    }
    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()> {
        self.inner.subscribe_for_task_log_part(handler)
    }
    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        self.inner.queues()
    }
    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        self.inner.queue_info(qname)
    }
    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        self.inner.delete_queue(qname)
    }
    fn health_check(&self) -> BoxedFuture<()> {
        self.inner.health_check()
    }
    fn shutdown(&self) -> BoxedFuture<()> {
        self.inner.shutdown()
    }
}

#[tokio::test]
async fn submit_job_returns_error_when_datastore_create_fails() -> Result<()> {
    let fail_config = Arc::new(RwLock::new(FailConfig {
        fail_create_job: true,
        ..Default::default()
    }));

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.set_broker(Box::new(FailableBroker {
        inner: InMemoryBroker::new(),
        config: fail_config.clone(),
    })).await;

    datastore.set_datastore(Box::new(FailableDatastore {
        inner: InMemoryDatastore::new(),
        config: fail_config.clone(),
    })).await;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("fail-job-1".into()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };

    let result = coordinator.submit_job(job).await;
    let err = result.unwrap_err();
    assert!(err.to_string().contains("failed to create job"));

    Ok(())
}

#[tokio::test]
async fn submit_job_returns_error_when_broker_publish_fails() -> Result<()> {
    let fail_config = Arc::new(RwLock::new(FailConfig {
        fail_publish_job: true,
        ..Default::default()
    }));

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    broker.set_broker(Box::new(FailableBroker {
        inner: InMemoryBroker::new(),
        config: fail_config.clone(),
    })).await;

    datastore.set_datastore(Box::new(FailableDatastore {
        inner: InMemoryDatastore::new(),
        config: fail_config.clone(),
    })).await;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("fail-job-2".into()),
        state: JOB_STATE_PENDING.to_string(),
        ..Default::default()
    };

    let result = coordinator.submit_job(job).await;
    let err = result.unwrap_err();
    assert!(err.to_string().contains("simulated publish_job failure"));

    // Verify job WAS created in datastore (mid-state transition failure)
    let persisted = datastore.get_job_by_id("fail-job-2").await?;
    assert_eq!(persisted.id.as_deref(), Some("fail-job-2"));

    Ok(())
}

#[tokio::test]
async fn start_job_returns_scheduled_state_when_broker_fails_to_publish_task() -> Result<()> {
    // This tests start_job in handlers.rs which is called when a job event is received.
    let fail_config = Arc::new(RwLock::new(FailConfig::default()));

    let broker = BrokerProxy::new();
    let datastore = DatastoreProxy::new();

    let failable_broker = FailableBroker {
        inner: InMemoryBroker::new(),
        config: fail_config.clone(),
    };

    broker.set_broker(Box::new(failable_broker)).await;

    datastore.set_datastore(Box::new(FailableDatastore {
        inner: InMemoryDatastore::new(),
        config: fail_config.clone(),
    })).await;

    let coordinator = create_coordinator(broker.clone(), datastore.clone()).await?;
    coordinator.start().await?;

    let job = Job {
        id: Some("job-3".into()),
        state: JOB_STATE_PENDING.to_string(),
        tasks: Some(vec![Task {
            name: Some("task 1".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    };

    // 1. Create job in datastore (coordinator's start_job handler reads it from there)
    datastore.create_job(&job).await?;

    // 2. Set fail_publish_task to true so the task dispatch fails
    fail_config.write().await.fail_publish_task = true;

    // 3. Publish the job event to trigger the coordinator's start_job handler
    broker.publish_job(&job).await?;

    // 3. Poll with channel-based timeout until job state transitions
    let (tx, rx) = oneshot::channel();
    let ds = datastore.clone();
    tokio::spawn(async move {
        let mut attempts = 0;
        loop {
            match ds.get_job_by_id("job-3").await {
                Ok(persisted_job) if persisted_job.state != JOB_STATE_PENDING => {
                    let _ = tx.send(Ok(persisted_job));
                    return;
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
                _ => {
                    attempts += 1;
                    if attempts >= 50 {
                        let _ = tx.send(Err(DatastoreError::Database("timeout waiting for state transition".into())));
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
    });

    let persisted_job = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
        .await
        .expect("polling timed out")
        .expect("channel dropped")?;

    assert_eq!(persisted_job.state, "SCHEDULED");
    
    // Verify a task was created
    let tasks = datastore.get_active_tasks("job-3").await?;
    assert_eq!(tasks.len(), 1);

    Ok(())
}
