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

use anyhow::Result;
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, debug};
use twerk_infrastructure::broker::queue::QUEUE_PENDING;
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
            .finish()
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
    name: String,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    datastore: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    stop_token: CancellationToken,
    task_tracker: TaskTracker,
}

impl CoordinatorImpl {
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
            ds.create_job(&job).await.map_err(|e| anyhow::anyhow!("failed to create job: {}", e))?;
            broker.publish_job(&job).await?;
            Ok(job)
        })
    }
}

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
