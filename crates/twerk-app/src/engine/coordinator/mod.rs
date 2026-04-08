//! Coordinator module for Twerk engine
//!
//! Orchestrates the workflow engine: manages job lifecycle, task scheduling,
//! and coordination between brokers and datastores.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument};

use twerk_infrastructure::broker::queue::{
    QUEUE_COMPLETED, QUEUE_FAILED, QUEUE_PENDING, QUEUE_REDELIVERIES, QUEUE_STARTED,
};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;
use twerk_infrastructure::locker::Locker;

pub mod auth;
pub mod handlers;
pub mod hostenv;
pub mod limits;
pub mod middleware;
pub mod schedule;
pub mod scheduler;
pub mod utils;
pub mod webhook;

use schedule::JobSchedulerHandler;

// ── Coordinator Trait ──────────────────────────────────────────

/// [`Coordinator`] orchestrates job lifecycle and task coordination.
#[async_trait]
pub trait Coordinator: Send + Sync {
    /// Returns the component name.
    fn name(&self) -> &'static str;
    /// Starts the coordinator subscriptions.
    async fn start(&self) -> Result<()>;
    /// Stops the coordinator.
    async fn stop(&self) -> Result<()>;
    /// Submits a job to the engine.
    async fn submit_job(&self, job: twerk_core::job::Job) -> Result<twerk_core::job::Job>;
}

// ── Default Coordinator ────────────────────────────────────────

/// Implementation of [`Coordinator`] using standard broker/datastore.
pub struct DefaultCoordinator {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
    scheduler: Arc<JobSchedulerHandler>,
    stop_token: CancellationToken,
}

impl DefaultCoordinator {
    /// Creates a new coordinator instance.
    ///
    /// # Errors
    /// Returns error if internal components fail to initialize.
    pub async fn new(
        ds: Arc<dyn Datastore>,
        broker: Arc<dyn Broker>,
        locker: Arc<dyn Locker>,
    ) -> Result<Self> {
        JobSchedulerHandler::new(ds.clone(), broker.clone(), locker)
            .await
            .map(|scheduler| Self {
                ds,
                broker,
                scheduler: Arc::new(scheduler),
                stop_token: CancellationToken::new(),
            })
    }

    /// Helper to subscribe a handler to a specific queue.
    async fn subscribe_task_handler<F, Fut>(&self, queue: &str, handler: F) -> Result<()>
    where
        F: Fn(Arc<dyn Datastore>, Arc<dyn Broker>, twerk_core::task::Task) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let ds = Arc::clone(&self.ds);
        let broker = Arc::clone(&self.broker);
        let st = self.stop_token.clone();
        let handler = Arc::new(handler);

        self.broker
            .subscribe_for_tasks(
                queue.to_string(),
                Arc::new(move |task| {
                    let (ds, b, st, h) = (ds.clone(), broker.clone(), st.clone(), handler.clone());
                    let task = (*task).clone();
                    Box::pin(async move {
                        if st.is_cancelled() {
                            Ok(())
                        } else {
                            h(ds, b, task).await
                        }
                    })
                }),
            )
            .await
    }
}

#[async_trait]
impl Coordinator for DefaultCoordinator {
    fn name(&self) -> &'static str {
        "Coordinator"
    }

    #[instrument(name = "coordinator_start", skip_all)]
    async fn start(&self) -> Result<()> {
        // 1. Task Subscription Pipeline
        self.subscribe_task_handler(QUEUE_PENDING, handlers::handle_pending_task)
            .await?;
        self.subscribe_task_handler(QUEUE_COMPLETED, handlers::handle_task_completed)
            .await?;
        self.subscribe_task_handler(QUEUE_FAILED, handlers::handle_error)
            .await?;
        self.subscribe_task_handler(QUEUE_STARTED, handlers::handle_started)
            .await?;
        self.subscribe_task_handler(QUEUE_REDELIVERIES, handlers::handle_redelivered)
            .await?;

        // 2. Progress Subscription
        let (ds, b, st) = (
            self.ds.clone(),
            self.broker.clone(),
            self.stop_token.clone(),
        );
        self.broker
            .subscribe_for_task_progress(Arc::new(move |task| {
                let (ds, b, st) = (ds.clone(), b.clone(), st.clone());
                Box::pin(async move {
                    if st.is_cancelled() {
                        Ok(())
                    } else {
                        Box::pin(handlers::handle_task_progress(ds, b, task)).await
                    }
                })
            }))
            .await?;

        // 3. Job Events Subscription
        let (ds, b, st) = (
            self.ds.clone(),
            self.broker.clone(),
            self.stop_token.clone(),
        );
        self.broker
            .subscribe_for_jobs(Arc::new(move |job| {
                let (ds, b, st) = (ds.clone(), b.clone(), st.clone());
                Box::pin(async move {
                    if st.is_cancelled() {
                        Ok(())
                    } else {
                        Box::pin(handlers::handle_job_event(ds, b, job)).await
                    }
                })
            }))
            .await?;

        // 4. Heartbeats
        let (ds, b, st) = (
            self.ds.clone(),
            self.broker.clone(),
            self.stop_token.clone(),
        );
        self.broker
            .subscribe_for_heartbeats(Arc::new(move |node| {
                let (ds, b, st) = (ds.clone(), b.clone(), st.clone());
                Box::pin(async move {
                    if st.is_cancelled() {
                        Ok(())
                    } else {
                        handlers::handle_heartbeat(ds, b, node).await
                    }
                })
            }))
            .await?;

        // 5. Task Log Parts
        let (ds, b, st) = (
            self.ds.clone(),
            self.broker.clone(),
            self.stop_token.clone(),
        );
        self.broker
            .subscribe_for_task_log_part(Arc::new(move |part| {
                let (ds, b, st) = (ds.clone(), b.clone(), st.clone());
                Box::pin(async move {
                    if st.is_cancelled() {
                        Ok(())
                    } else {
                        handlers::handle_log_part(ds, b, part).await
                    }
                })
            }))
            .await?;

        // 6. Scheduled Job Events
        let (scheduler, st) = (self.scheduler.clone(), self.stop_token.clone());
        self.broker
            .subscribe_for_events(
                "scheduled.job".to_string(),
                Arc::new(move |sj_val| {
                    let (scheduler, st) = (scheduler.clone(), st.clone());
                    Box::pin(async move {
                        if st.is_cancelled() {
                            Ok(())
                        } else {
                            let sj =
                                serde_json::from_value::<twerk_core::job::ScheduledJob>(sj_val)?;
                            scheduler.handle_scheduled_job(&sj).await
                        }
                    })
                }),
            )
            .await?;

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("Stopping coordinator");
        self.stop_token.cancel();
        Ok(())
    }

    #[instrument(name = "submit_job", skip_all, fields(job_name = %job.name.as_deref().unwrap_or("unknown")))]
    async fn submit_job(&self, job: twerk_core::job::Job) -> Result<twerk_core::job::Job> {
        let job_id = job
            .id
            .clone()
            .unwrap_or_else(|| twerk_core::uuid::new_short_uuid().into());
        let mut job = job;
        job.id = Some(job_id);

        if job.created_at.is_none() {
            job.created_at = Some(time::OffsetDateTime::now_utc());
        }

        self.ds
            .create_job(&job)
            .await
            .map_err(|e| anyhow!("failed to create job: {e}"))?;
        self.broker
            .publish_job(&job)
            .await
            .map_err(|e| anyhow!("failed to publish job: {e}"))?;
        Ok(job)
    }
}

// ── Coordinator Factory ────────────────────────────────────────

/// Creates a new coordinator using proxies.
///
/// # Errors
/// Returns error if locker or coordinator creation fails.
pub async fn create_coordinator(
    broker: crate::engine::broker::BrokerProxy,
    ds: crate::engine::datastore::DatastoreProxy,
) -> Result<Box<dyn Coordinator>> {
    let locker_type = crate::engine::resolve_locker_type();
    let locker: Box<dyn Locker> = crate::engine::locker::create_locker(&locker_type).await?;
    let locker_arc: Arc<dyn Locker> = Arc::from(locker);

    DefaultCoordinator::new(Arc::new(ds), Arc::new(broker), locker_arc)
        .await
        .map(|coord| Box::new(coord) as Box<dyn Coordinator>)
}
