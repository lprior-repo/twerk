//! Cron scheduler handler for scheduled jobs.
//!
//! Handles scheduling and unscheduling of jobs based on their state (Active/Paused).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, instrument};
use twerk_core::job::{Job as TorkJob, JobState, ScheduledJob, ScheduledJobState};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;
use twerk_infrastructure::locker::Locker;
use uuid::Uuid;

// ── Calculations (Pure) ────────────────────────────────────────

fn sj_id_str(sj: &ScheduledJob) -> &str {
    sj.id.as_deref().map_or("unknown", |id| id)
}

// ── Actions ────────────────────────────────────────────────────

pub struct JobSchedulerHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
    locker: Arc<dyn Locker>,
    scheduler: JobScheduler,
    jobs: Mutex<HashMap<String, Uuid>>,
}

impl JobSchedulerHandler {
    /// Creates a new job scheduler handler.
    ///
    /// # Errors
    /// Returns error if scheduler initialization fails.
    pub async fn new(
        ds: Arc<dyn Datastore>,
        broker: Arc<dyn Broker>,
        locker: Arc<dyn Locker>,
    ) -> Result<Self> {
        JobScheduler::new()
            .await
            .map_err(|e| anyhow!("error creating scheduler: {e}"))
            .map(|scheduler| Self {
                ds,
                broker,
                locker,
                scheduler,
                jobs: Mutex::new(HashMap::new()),
            })?
            .pipe(|handler| async move {
                let active_jobs = handler.ds.get_active_scheduled_jobs().await?;
                for sj in active_jobs {
                    handler.handle_active(&sj).await?;
                }
                handler
                    .scheduler
                    .start()
                    .await
                    .map_err(|e| anyhow!("error starting scheduler: {e}"))?;
                Ok(handler)
            })
            .await
    }

    /// Handles a scheduled job event.
    ///
    /// # Errors
    /// Returns error if state transition fails.
    #[instrument(name = "handle_scheduled_job", skip_all, fields(sj_id = %sj_id_str(sj), state = ?sj.state))]
    pub async fn handle_scheduled_job(&self, sj: &ScheduledJob) -> Result<()> {
        match sj.state {
            ScheduledJobState::Active => self.handle_active(sj).await,
            ScheduledJobState::Paused => self.handle_paused(sj).await,
        }
    }

    async fn handle_active(&self, sj: &ScheduledJob) -> Result<()> {
        let sj_id = sj_id_str(sj).to_string();
        info!(sj_id = %sj_id, cron = ?sj.cron, "Scheduling job");

        let cron_expr = sj
            .cron
            .as_deref()
            .ok_or_else(|| anyhow!("scheduled job {sj_id} has no cron expression"))?;

        let ds = self.ds.clone();
        let broker = self.broker.clone();
        let locker = self.locker.clone();
        let sj_id_clone = sj_id.clone();

        let job = Job::new_async(cron_expr, move |_uuid, _lock| {
            let (ds, b, l, id) = (
                ds.clone(),
                broker.clone(),
                locker.clone(),
                sj_id_clone.clone(),
            );
            Box::pin(async move {
                if let Err(e) = trigger_scheduled_job(&ds, &b, &l, &id).await {
                    error!(sj_id = %id, error = %e, "error triggering scheduled job");
                }
            })
        })
        .map_err(|e| anyhow!("error creating job: {e}"))?;

        let job_id = self
            .scheduler
            .add(job)
            .await
            .map_err(|e| anyhow!("error adding job: {e}"))?;

        let mut jobs = self.jobs.lock().await;
        jobs.insert(sj_id, job_id);

        Ok(())
    }

    async fn handle_paused(&self, sj: &ScheduledJob) -> Result<()> {
        let sj_id = sj_id_str(sj);
        let mut jobs = self.jobs.lock().await;

        jobs.remove(sj_id)
            .ok_or_else(|| anyhow!("unknown scheduled job: {sj_id}"))
            .pipe(|res| async move {
                let job_id = res?;
                self.scheduler
                    .remove(&job_id)
                    .await
                    .map_err(|e| anyhow!("error removing job: {e}"))
            })
            .await?;

        info!(sj_id = %sj_id, "Pausing scheduled job");
        Ok(())
    }
}

impl Clone for JobSchedulerHandler {
    fn clone(&self) -> Self {
        Self {
            ds: self.ds.clone(),
            broker: self.broker.clone(),
            locker: self.locker.clone(),
            scheduler: self.scheduler.clone(),
            jobs: Mutex::new(HashMap::new()),
        }
    }
}

#[instrument(name = "trigger_scheduled_job", skip_all, fields(sj_id = %sj_id))]
async fn trigger_scheduled_job(
    ds: &Arc<dyn Datastore>,
    broker: &Arc<dyn Broker>,
    locker: &Arc<dyn Locker>,
    sj_id: &str,
) -> Result<()> {
    let sj = ds.get_scheduled_job_by_id(sj_id).await?;
    let lock_key = format!("scheduled_job:{sj_id}");
    let _lock = locker.acquire_lock(&lock_key).await?;

    let now = time::OffsetDateTime::now_utc();
    let job_id = Uuid::new_v4().to_string();

    let job = TorkJob {
        id: Some(job_id.into()),
        created_by: sj.created_by.clone(),
        created_at: Some(now),
        permissions: sj.permissions.clone(),
        tags: sj.tags.clone(),
        name: sj.name.clone(),
        description: sj.description.clone(),
        state: JobState::Pending,
        tasks: sj.tasks.clone(),
        inputs: sj.inputs.clone(),
        secrets: sj.secrets.clone(),
        context: Some(twerk_core::job::JobContext {
            inputs: sj.inputs.clone(),
            secrets: sj.secrets.clone(),
            ..Default::default()
        }),
        task_count: sj.tasks.as_ref().map_or(0, |t| t.len() as i64),
        output: sj.output.clone(),
        webhooks: sj.webhooks.clone(),
        auto_delete: sj.auto_delete.clone(),
        schedule: Some(twerk_core::job::JobSchedule {
            id: sj.id.clone(),
            cron: sj.cron.clone(),
        }),
        ..Default::default()
    };

    ds.create_job(&job)
        .await
        .map_err(|e| anyhow!("error creating scheduled job instance: {e}"))?;

    broker
        .publish_job(&job)
        .await
        .map_err(|e| anyhow!("error publishing scheduled job instance: {e}"))?;

    debug!(sj_id = %sj_id, "Successfully triggered scheduled job");
    Ok(())
}

/// Extension trait for functional piping.
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R;
}
impl<T> Pipe for T {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}
