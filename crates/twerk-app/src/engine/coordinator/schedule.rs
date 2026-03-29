//! Cron scheduler handler for scheduled jobs.
//!
//! Handles scheduling and unscheduling of jobs based on their state (Active/Paused).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info};
use uuid::Uuid;
use twerk_core::job::{Job as TorkJob, JobState, ScheduledJob, SCHEDULED_JOB_STATE_ACTIVE, SCHEDULED_JOB_STATE_PAUSED};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::datastore::Datastore;
use twerk_infrastructure::locker::Locker;

#[allow(dead_code)] const MIN_SCHEDULED_JOB_LOCK_TTL_SECS: i64 = 10;

#[derive(Clone)]
pub struct JobSchedulerHandle {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
    locker: Arc<dyn Locker>,
}

impl JobSchedulerHandle {
    pub async fn handle_scheduled_job(&self, sj: &ScheduledJob) -> Result<()> {
        match sj.state.as_str() {
            SCHEDULED_JOB_STATE_ACTIVE => self.handle_active(sj).await,
            SCHEDULED_JOB_STATE_PAUSED => self.handle_paused(sj).await,
            _ => Err(anyhow!("unknown scheduled job state: {}", sj.state)),
        }
    }

    async fn handle_active(&self, sj: &ScheduledJob) -> Result<()> {
        let sj_id = sj.id.as_ref().map(|id| id.to_string()).unwrap_or_default();
        info!("Scheduling job {} with cron {:?}", sj_id, sj.cron);

        let cron_expr = sj
            .cron
            .as_ref()
            .ok_or_else(|| anyhow!("scheduled job {} has no cron expression", sj_id))?;

        debug!("Successfully scheduled job {} with cron {}", sj_id, cron_expr);
        Ok(())
    }

    async fn handle_paused(&self, sj: &ScheduledJob) -> Result<()> {
        let sj_id = sj.id.as_ref().map(|id| id.to_string()).unwrap_or_default();
        info!("Pausing scheduled job {}", sj_id);
        Ok(())
    }

    pub async fn trigger_scheduled_job(&self, sj_id: &str) -> Result<()> {
        let sj = self.ds.get_scheduled_job_by_id(sj_id).await?;

        let lock_key = format!("scheduled_job:{}", sj_id);
        let lock = self.locker.acquire_lock(&lock_key).await?;

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
            state: JobState::from(twerk_core::job::JOB_STATE_PENDING),
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

        if let Err(e) = self.ds.create_job(&job).await {
            error!("error creating scheduled job instance: {}", e);
            return Err(anyhow!("error creating scheduled job instance: {}", e));
        }

        if let Err(e) = self.broker.publish_job(&job).await {
            error!("error publishing scheduled job instance: {}", e);
            return Err(anyhow!("error publishing scheduled job instance: {}", e));
        }

        debug!("Successfully triggered scheduled job {}", sj_id);

        let _ = lock;

        Ok(())
    }
}

pub struct JobSchedulerHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
    locker: Arc<dyn Locker>,
    scheduler: JobScheduler,
    jobs: Mutex<HashMap<String, Uuid>>,
}

impl JobSchedulerHandler {
    pub async fn new(
        ds: Arc<dyn Datastore>,
        broker: Arc<dyn Broker>,
        locker: Arc<dyn Locker>,
    ) -> Result<Self> {
        let scheduler = JobScheduler::new().await.map_err(|e| anyhow!("error creating scheduler: {}", e))?;

        let handler = Self {
            ds,
            broker,
            locker,
            scheduler,
            jobs: Mutex::new(HashMap::new()),
        };

        let active_jobs = handler.ds.get_active_scheduled_jobs().await?;
        for sj in active_jobs {
            if let Err(e) = handler.handle_scheduled_job(&sj).await {
                error!("error handling active scheduled job: {}", e);
            }
        }

        Ok(handler)
    }

    pub async fn handle_scheduled_job(&self, sj: &ScheduledJob) -> Result<()> {
        match sj.state.as_str() {
            SCHEDULED_JOB_STATE_ACTIVE => self.handle_active(sj).await,
            SCHEDULED_JOB_STATE_PAUSED => self.handle_paused(sj).await,
            _ => Err(anyhow!("unknown scheduled job state: {}", sj.state)),
        }
    }

    async fn handle_active(&self, sj: &ScheduledJob) -> Result<()> {
        let sj_id = sj.id.as_ref().map(|id| id.to_string()).unwrap_or_default();
        info!("Scheduling job {} with cron {:?}", sj_id, sj.cron);

        let cron_expr = sj
            .cron
            .as_ref()
            .ok_or_else(|| anyhow!("scheduled job {} has no cron expression", sj_id))?;

        let ds = self.ds.clone();
        let broker = self.broker.clone();
        let locker = self.locker.clone();
        let sj_id_clone = sj_id.clone();

        let job = Job::new_async(cron_expr, move |_uuid, _lock| {
            let ds = ds.clone();
            let broker = broker.clone();
            let locker = locker.clone();
            let sj_id = sj_id_clone.clone();
            Box::pin(async move {
                if let Err(e) = trigger_scheduled_job(&ds, &broker, &locker, &sj_id).await {
                    error!("error triggering scheduled job {}: {}", sj_id, e);
                }
            })
        })
        .map_err(|e| anyhow!("error creating job: {}", e))?;

        let job_id = self.scheduler.add(job).await.map_err(|e| anyhow!("error adding job: {}", e))?;

        let mut jobs = self.jobs.lock().await;
        jobs.insert(sj_id.clone(), job_id);

        debug!("Successfully scheduled job {} with cron {}", sj_id, cron_expr);
        Ok(())
    }

    #[allow(dead_code)] fn locker(&self) -> Arc<dyn Locker> {
        self.locker.clone()
    }

    async fn handle_paused(&self, sj: &ScheduledJob) -> Result<()> {
        let sj_id = sj.id.as_ref().map(|id| id.to_string()).unwrap_or_default();

        let mut jobs = self.jobs.lock().await;
        let job_id = jobs.remove(&sj_id).ok_or_else(|| {
            anyhow!("unknown scheduled job: {}", sj_id)
        })?;
        drop(jobs);

        self.scheduler.remove(&job_id).await.map_err(|e| anyhow!("error removing job: {}", e))?;

        info!("Pausing scheduled job {}", sj_id);
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

async fn trigger_scheduled_job(
    ds: &Arc<dyn Datastore>,
    broker: &Arc<dyn Broker>,
    locker: &Arc<dyn Locker>,
    sj_id: &str,
) -> Result<()> {
    let sj = ds.get_scheduled_job_by_id(sj_id).await?;

    let lock_key = format!("scheduled_job:{}", sj_id);
    let lock = locker.acquire_lock(&lock_key).await?;

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
        state: JobState::from(twerk_core::job::JOB_STATE_PENDING),
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

    if let Err(e) = ds.create_job(&job).await {
        error!("error creating scheduled job instance: {}", e);
        return Err(anyhow!("error creating scheduled job instance: {}", e));
    }

    if let Err(e) = broker.publish_job(&job).await {
        error!("error publishing scheduled job instance: {}", e);
        return Err(anyhow!("error publishing scheduled job instance: {}", e));
    }

    debug!("Successfully triggered scheduled job {}", sj_id);

    let _ = lock;

    Ok(())
}
