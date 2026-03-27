//! Coordinator module for twerk
//!
//! Handles job coordination, distributed locking, and HTTP middleware.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

pub mod auth;
pub mod handlers;
pub mod limits;
pub mod middleware;
pub mod scheduler;
pub mod utils;
pub mod webhook;

use anyhow::Result;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, debug};
use twerk_infrastructure::broker::queue::{QUEUE_COMPLETED, QUEUE_FAILED, QUEUE_PENDING, QUEUE_REDELIVERIES, QUEUE_STARTED};
use twerk_infrastructure::config;

use crate::engine::BrokerProxy;
use crate::engine::DatastoreProxy;

pub use twerk_core::user::USERNAME;
pub use utils::wildcard_match;

/// Boxed future type for coordinator operations
pub type BoxedFuture<T> = Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>;

/// Coordinator trait for job coordination
pub trait Coordinator: Send + Sync {
    fn start(&self) -> BoxedFuture<()>;
    fn stop(&self) -> BoxedFuture<()>;
    fn submit_job(&self, job: twerk_core::job::Job) -> BoxedFuture<twerk_core::job::Job>;
}

/// Locker trait for distributed locking
pub trait Locker: Send + Sync {
    fn acquire_lock(&self, key: &str) -> BoxedFuture<()>;
}

/// Simple in-memory locker implementation
pub struct InMemoryLocker;

impl Locker for InMemoryLocker {
    fn acquire_lock(&self, _key: &str) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

/// Configuration for the coordinator
pub struct Config {
    pub name: String,
    pub broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    pub datastore: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    pub locker: Arc<dyn Locker>,
    pub queues: std::collections::HashMap<String, i64>,
    pub address: String,
    pub enabled: std::collections::HashMap<String, bool>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("name", &self.name)
            .field("queues", &self.queues)
            .field("address", &self.address)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Coordinator".to_string(),
            broker: Arc::new(BrokerProxy::new()),
            datastore: Arc::new(DatastoreProxy::new()),
            locker: Arc::new(InMemoryLocker),
            queues: std::collections::HashMap::new(),
            address: "0.0.0.0:8000".to_string(),
            enabled: std::collections::HashMap::new(),
        }
    }
}

impl Config {
    /// Creates a new coordinator config from infrastructure config (TOML/ENV)
    #[must_use] 
    pub fn from_infra() -> Self {
        let name = config::string_default("coordinator.name", "Coordinator");
        let address = config::string_default("coordinator.address", "0.0.0.0:8000");
        let queues = config::int_map("coordinator.queues");
        let enabled = config::bool_map("coordinator.api.endpoints");

        Self {
            name,
            broker: Arc::new(BrokerProxy::new()),
            datastore: Arc::new(DatastoreProxy::new()),
            locker: Arc::new(InMemoryLocker),
            queues,
            address,
            enabled,
        }
    }
}

pub struct CoordinatorImpl {
    #[allow(dead_code)]
    name: String,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    datastore: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    stop_token: CancellationToken,
    task_tracker: TaskTracker,
}

impl CoordinatorImpl {
    #[must_use] 
    pub fn new(config: Config) -> Self {
        Self {
            name: config.name,
            broker: config.broker,
            datastore: config.datastore,
            stop_token: CancellationToken::new(),
            task_tracker: TaskTracker::new(),
        }
    }
}

impl Coordinator for CoordinatorImpl {
    #[allow(clippy::too_many_lines)]
    fn start(&self) -> BoxedFuture<()> {
        let broker = self.broker.clone();
        let ds = self.datastore.clone();
        let stop_token = self.stop_token.clone();
        let tracker = self.task_tracker.clone();

        Box::pin(async move {
            info!("Starting coordinator");
            
            // Subscribe to task progress
            let ds_p = ds.clone();
            let b_p = broker.clone();
            let st_p = stop_token.clone();
            let tr_p = tracker.clone();
            broker.subscribe_for_task_progress(Arc::new(move |task| {
                let ds = ds_p.clone();
                let b = b_p.clone();
                let st = st_p.clone();
                let tr = tr_p.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_task_progress(ds, b, task));
                    Ok(())
                })
            })).await?;

            // Subscribe to job events
            let ds_j = ds.clone();
            let b_j = broker.clone();
            let st_j = stop_token.clone();
            let tr_j = tracker.clone();
            broker.subscribe_for_jobs(Arc::new(move |job| {
                let ds = ds_j.clone();
                let b = b_j.clone();
                let st = st_j.clone();
                let tr = tr_j.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_job_event(ds, b, job));
                    Ok(())
                })
            })).await?;

            // Subscribe to pending tasks
            let ds_t = ds.clone();
            let b_t = broker.clone();
            let st_t = stop_token.clone();
            let tr_t = tracker.clone();
            broker.subscribe_for_tasks(QUEUE_PENDING.to_string(), Arc::new(move |task| {
                let ds = ds_t.clone();
                let b = b_t.clone();
                let task = (*task).clone();
                let st = st_t.clone();
                let tr = tr_t.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_pending_task(ds, b, task));
                    Ok(())
                })
            })).await?;

            // Subscribe to completed tasks
            let ds_c = ds.clone();
            let b_c = broker.clone();
            let st_c = stop_token.clone();
            let tr_c = tracker.clone();
            broker.subscribe_for_tasks(QUEUE_COMPLETED.to_string(), Arc::new(move |task| {
                let ds = ds_c.clone();
                let b = b_c.clone();
                let task = (*task).clone();
                let st = st_c.clone();
                let tr = tr_c.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_task_completed(ds, b, task));
                    Ok(())
                })
            })).await?;

            // Subscribe to failed tasks
            let ds_f = ds.clone();
            let b_f = broker.clone();
            let st_f = stop_token.clone();
            let tr_f = tracker.clone();
            broker.subscribe_for_tasks(QUEUE_FAILED.to_string(), Arc::new(move |task| {
                let ds = ds_f.clone();
                let b = b_f.clone();
                let task = (*task).clone();
                let st = st_f.clone();
                let tr = tr_f.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_error(ds, b, task));
                    Ok(())
                })
            })).await?;

            // Subscribe to started tasks
            let ds_s = ds.clone();
            let b_s = broker.clone();
            let st_s = stop_token.clone();
            let tr_s = tracker.clone();
            broker.subscribe_for_tasks(QUEUE_STARTED.to_string(), Arc::new(move |task| {
                let ds = ds_s.clone();
                let b = b_s.clone();
                let task = (*task).clone();
                let st = st_s.clone();
                let tr = tr_s.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_started(ds, b, task));
                    Ok(())
                })
            })).await?;

            // Subscribe to redelivered tasks
            let ds_r = ds.clone();
            let b_r = broker.clone();
            let st_r = stop_token.clone();
            let tr_r = tracker.clone();
            broker.subscribe_for_tasks(QUEUE_REDELIVERIES.to_string(), Arc::new(move |task| {
                let ds = ds_r.clone();
                let b = b_r.clone();
                let task = (*task).clone();
                let st = st_r.clone();
                let tr = tr_r.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_redelivered(ds, b, task));
                    Ok(())
                })
            })).await?;

            // Subscribe to heartbeats
            let ds_h = ds.clone();
            let b_h = broker.clone();
            let st_h = stop_token.clone();
            let tr_h = tracker.clone();
            broker.subscribe_for_heartbeats(Arc::new(move |node| {
                let ds = ds_h.clone();
                let b = b_h.clone();
                let st = st_h.clone();
                let tr = tr_h.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_heartbeat(ds, b, node));
                    Ok(())
                })
            })).await?;

            // Subscribe to task log parts
            let ds_l = ds.clone();
            let b_l = broker.clone();
            let st_l = stop_token.clone();
            let tr_l = tracker.clone();
            broker.subscribe_for_task_log_part(Arc::new(move |part| {
                let ds = ds_l.clone();
                let b = b_l.clone();
                let st = st_l.clone();
                let tr = tr_l.clone();
                Box::pin(async move {
                    if st.is_cancelled() { return Ok(()); }
                    tr.spawn(handlers::handle_log_part(ds, b, part));
                    Ok(())
                })
            })).await?;

            Ok(())
        })
    }

    fn stop(&self) -> BoxedFuture<()> {
        let stop_token = self.stop_token.clone();
        let tracker = self.task_tracker.clone();
        Box::pin(async move {
            info!("Stopping coordinator: cancelling handlers and waiting for tasks to settle");
            stop_token.cancel();
            tracker.close();
            tracker.wait().await;
            debug!("Coordinator stopped");
            Ok(())
        })
    }

    fn submit_job(&self, mut job: twerk_core::job::Job) -> BoxedFuture<twerk_core::job::Job> {
        let broker = self.broker.clone();
        let ds = self.datastore.clone();
        Box::pin(async move {
            if job.id.is_none() {
                job.id = Some(uuid::Uuid::new_v4().to_string().into());
            }
            job.state = twerk_core::job::JOB_STATE_PENDING.to_string();
            job.created_at = Some(time::OffsetDateTime::now_utc());
            ds.create_job(&job).await.map_err(|e| anyhow::anyhow!("failed to create job: {e}"))?;
            broker.publish_job(&job).await?;
            Ok(job)
        })
    }
}

/// Creates a new coordinator instance.
/// # Errors
/// Returns error if locker creation fails.
pub async fn create_coordinator(
    broker: BrokerProxy,
    datastore: DatastoreProxy,
) -> Result<Box<dyn Coordinator + Send + Sync>> {
    let config = Config::from_infra();
    let b: Arc<dyn twerk_infrastructure::broker::Broker> = Arc::new(broker);
    let ds: Arc<dyn twerk_infrastructure::datastore::Datastore> = Arc::new(datastore);

    let coordinator = CoordinatorImpl::new(Config {
        name: config.name,
        broker: b,
        datastore: ds,
        locker: config.locker,
        queues: config.queues,
        address: config.address,
        enabled: config.enabled,
    });

    Ok(Box::new(coordinator) as Box<dyn Coordinator + Send + Sync>)
}
