//! Schedule handler for scheduled job events.
//!
//! Port of Go `internal/coordinator/handlers/schedule.go` with 100% parity.
//!
//! # Go Parity
//!
//! 1. Receives scheduled job events (ACTIVE, PAUSED)
//! 2. `handleActive`: creates cron scheduler entries for active scheduled jobs
//! 3. `handlePaused`: removes cron scheduler entries for paused jobs
//! 4. Each cron tick: fetches latest ScheduledJob from datastore, creates
//!    a Job instance, persists it, and publishes via broker
//! 5. Initializes all existing active scheduled jobs on startup
//!
//! # Distributed Locking
//!
//! The Go implementation uses gocron's `WithDistributedLocker` to prevent
//! duplicate scheduling across multiple coordinator instances. The Rust
//! implementation integrates with the `locker` crate by acquiring a
//! distributed lock inside the cron tick callback before creating the job.

pub mod calc;

use std::collections::HashMap;
use std::sync::Arc;

use locker::Locker;
use tork::job::ScheduledJob;
use tork::{Broker, Datastore};

use crate::handlers::HandlerError;

// Re-export from calc module
pub use calc::{calculate_schedule_action, create_job_from_scheduled, ScheduleAction};

// Minimum time a scheduled job lock should be held.
// Go: `minScheduledJobLockTTL = 10 * time.Second`
const MIN_SCHEDULED_JOB_LOCK_TTL: std::time::Duration = std::time::Duration::from_secs(10);

/// Schedule handler for processing scheduled job events.
///
/// Manages cron-based scheduling of recurring jobs. On each cron tick,
/// a new Job instance is created from the ScheduledJob template,
/// persisted in the datastore, and published via the broker.
///
/// Uses distributed locking via the `locker` crate to prevent duplicate
/// job creation when multiple coordinators are running.
pub struct ScheduleHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
    locker: Arc<dyn Locker>,
    scheduler: Arc<tokio_cron_scheduler::JobScheduler>,
    jobs: Arc<tokio::sync::Mutex<HashMap<String, uuid::Uuid>>>,
}

impl std::fmt::Debug for ScheduleHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduleHandler")
            .field("ds", &"<dyn Datastore>")
            .field("broker", &"<dyn Broker>")
            .field("locker", &"<dyn Locker>")
            .field("scheduler", &"<tokio_cron_scheduler::JobScheduler>")
            .field("jobs", &"<Mutex<HashMap>>")
            .finish()
    }
}

impl ScheduleHandler {
    /// Create a new schedule handler with datastore, broker, and distributed locker.
    pub async fn new(
        ds: Arc<dyn Datastore>,
        broker: Arc<dyn Broker>,
        locker: Arc<dyn Locker>,
    ) -> Result<Self, HandlerError> {
        let scheduler = tokio_cron_scheduler::JobScheduler::new()
            .await
            .map_err(|e| HandlerError::Handler(format!("error creating scheduler: {e}")))?;

        scheduler
            .start()
            .await
            .map_err(|e| HandlerError::Handler(format!("error starting scheduler: {e}")))?;

        let handler = Self {
            ds: ds.clone(),
            broker,
            locker,
            scheduler: Arc::new(scheduler),
            jobs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        };

        // Initialize all existing active jobs
        let active_jobs = handler
            .ds
            .get_active_scheduled_jobs()
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        for active_job in &active_jobs {
            let job_id = match &active_job.id {
                Some(id) => id.clone(),
                None => continue,
            };
            if let Err(e) = handler.handle_active(&job_id, &active_job.cron).await {
                tracing::warn!(
                    error = %e,
                    scheduled_job_id = %job_id,
                    "failed to initialize scheduled job during startup"
                );
            }
        }

        Ok(handler)
    }

    /// Handle a scheduled job event.
    pub async fn handle(&self, scheduled_job: &ScheduledJob) -> Result<(), HandlerError> {
        let job_id = scheduled_job
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("scheduled job ID is required".into()))?;

        match calculate_schedule_action(&scheduled_job.state)? {
            ScheduleAction::Activate => self.handle_active(job_id, &scheduled_job.cron).await,
            ScheduleAction::Pause => self.handle_paused(job_id).await,
        }
    }

    /// Handle activation of a scheduled job.
    async fn handle_active(
        &self,
        job_id: &str,
        cron_expr: &Option<String>,
    ) -> Result<(), HandlerError> {
        let cron = cron_expr.as_deref().ok_or_else(|| {
            HandlerError::Validation(format!("cron expression required for {job_id}"))
        })?;

        tracing::info!(scheduled_job_id = job_id, cron = cron, "Scheduling job");

        let ds = self.ds.clone();
        let broker = self.broker.clone();
        let locker = self.locker.clone();
        let scheduled_job_id = job_id.to_string();

        let cron_job = tokio_cron_scheduler::Job::new_async(cron, move |_uuid, _lock| {
            let ds = ds.clone();
            let broker = broker.clone();
            let locker = locker.clone();
            let sj_id = scheduled_job_id.clone();
            Box::pin(async move {
                let lock_key = format!("schedule:{}", sj_id);
                let lock_start = std::time::Instant::now();
                let lock = match locker.acquire_lock(&lock_key).await {
                    Ok(lock) => lock,
                    Err(e) => {
                        tracing::warn!(
                            scheduled_job_id = %sj_id,
                            error = %e,
                            "failed to acquire distributed lock, skipping this tick"
                        );
                        return;
                    }
                };

                let scheduled = match ds.get_scheduled_job_by_id(sj_id.clone()).await {
                    Ok(Some(sj)) => sj,
                    Ok(None) => {
                        tracing::error!(
                            scheduled_job_id = %sj_id,
                            "scheduled job not found before creating instance"
                        );
                        return;
                    }
                    Err(e) => {
                        tracing::error!(
                            scheduled_job_id = %sj_id,
                            error = %e,
                            "error fetching scheduled job before creating instance"
                        );
                        return;
                    }
                };

                let job = create_job_from_scheduled(&scheduled);
                let job_id_str = match &job.id {
                    Some(id) => id.clone(),
                    None => return,
                };

                if let Err(e) = ds.create_job(job.clone()).await {
                    tracing::error!(
                        scheduled_job_id = %sj_id,
                        job_id = %job_id_str,
                        error = %e,
                        "error creating scheduled job instance"
                    );
                    return;
                }
                if let Err(e) = broker.publish_job(&job).await {
                    tracing::error!(
                        scheduled_job_id = %sj_id,
                        job_id = %job_id_str,
                        error = %e,
                        "error publishing scheduled job instance"
                    );
                }

                // Go parity: ensure minimum lock hold time to prevent coordinator drift
                // if elapsed < minScheduledJobLockTTL { time.Sleep(minScheduledJobLockTTL - elapsed) }
                let elapsed = lock_start.elapsed();
                if elapsed < MIN_SCHEDULED_JOB_LOCK_TTL {
                    tokio::time::sleep(MIN_SCHEDULED_JOB_LOCK_TTL - elapsed).await;
                }
                lock.release_lock().await.ok();
            })
        })
        .map_err(|e| HandlerError::Handler(format!("error scheduling job {job_id}: {e}")))?;

        let job_uuid =
            self.scheduler.add(cron_job).await.map_err(|e| {
                HandlerError::Handler(format!("error scheduling job {job_id}: {e}"))
            })?;

        let mut jobs = self.jobs.lock().await;
        jobs.insert(job_id.to_string(), job_uuid);

        Ok(())
    }

    /// Handle pausing of a scheduled job.
    async fn handle_paused(&self, job_id: &str) -> Result<(), HandlerError> {
        let jobs = self.jobs.lock().await;

        let job_uuid = jobs
            .get(job_id)
            .ok_or_else(|| HandlerError::NotFound(format!("unknown scheduled job: {job_id}")))?;

        tracing::info!(scheduled_job_id = job_id, "Pausing scheduled job");

        self.scheduler.remove(job_uuid).await.map_err(|e| {
            HandlerError::Handler(format!("error pausing scheduled job {job_id}: {e}"))
        })?;

        drop(jobs);
        self.jobs.lock().await.remove(job_id);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use self::calc::MIN_SCHEDULED_JOB_LOCK_TTL;
    use super::*;
    use tork::{JOB_STATE_PENDING, SCHEDULED_JOB_STATE_ACTIVE, SCHEDULED_JOB_STATE_PAUSED};

    #[test]
    fn test_calculate_schedule_action_active() {
        let result = calculate_schedule_action(SCHEDULED_JOB_STATE_ACTIVE);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ScheduleAction::Activate);
    }

    #[test]
    fn test_calculate_schedule_action_paused() {
        let result = calculate_schedule_action(SCHEDULED_JOB_STATE_PAUSED);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ScheduleAction::Pause);
    }

    #[test]
    fn test_calculate_schedule_action_unknown() {
        let result = calculate_schedule_action("UNKNOWN");
        assert!(result.is_err());
        match result {
            Err(HandlerError::InvalidState(msg)) => assert!(msg.contains("UNKNOWN")),
            other => panic!("expected InvalidState, got: {other:?}"),
        }
    }

    #[test]
    fn test_calculate_schedule_action_empty() {
        let result = calculate_schedule_action("");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_job_from_scheduled_basic() {
        let scheduled = ScheduledJob {
            id: Some("sched-1".to_string()),
            name: Some("Test Scheduled Job".to_string()),
            cron: Some("* * * * *".to_string()),
            state: SCHEDULED_JOB_STATE_ACTIVE.to_string(),
            tasks: vec![],
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert!(job.id.is_some());
        assert_eq!(job.state, JOB_STATE_PENDING);
        assert_eq!(job.name.as_deref(), Some("Test Scheduled Job"));
        assert_eq!(job.task_count, 0);
        assert!(job.schedule.is_some());
        assert_eq!(
            job.schedule.as_ref().and_then(|s| s.id.as_deref()),
            Some("sched-1")
        );
        assert_eq!(
            job.schedule.as_ref().and_then(|s| s.cron.as_deref()),
            Some("* * * * *")
        );
    }

    #[test]
    fn test_create_job_from_scheduled_with_tasks() {
        use tork::task::Task;

        let scheduled = ScheduledJob {
            id: Some("sched-2".to_string()),
            tasks: vec![Task::default(), Task::default(), Task::default()],
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(job.task_count, 3);
        assert_eq!(job.tasks.len(), 3);
    }

    #[test]
    fn test_create_job_from_scheduled_preserves_inputs_and_secrets() {
        use std::collections::HashMap;

        let mut inputs = HashMap::new();
        inputs.insert("key1".to_string(), "val1".to_string());

        let mut secrets = HashMap::new();
        secrets.insert("secret1".to_string(), "token1".to_string());

        let scheduled = ScheduledJob {
            id: Some("sched-3".to_string()),
            inputs: Some(inputs.clone()),
            secrets: Some(secrets.clone()),
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(job.inputs, Some(inputs.clone()));
        assert_eq!(job.context.inputs, Some(inputs));
        assert_eq!(job.secrets, Some(secrets.clone()));
        assert_eq!(job.context.secrets, Some(secrets));
    }

    #[test]
    fn test_debug_impl() {
        let debug_str = format!("{:?}", ScheduleAction::Activate);
        assert!(debug_str.contains("Activate"));
        let debug_str = format!("{:?}", ScheduleAction::Pause);
        assert!(debug_str.contains("Pause"));
    }

    #[test]
    fn test_min_lock_ttl() {
        assert_eq!(
            MIN_SCHEDULED_JOB_LOCK_TTL,
            std::time::Duration::from_secs(10)
        );
    }
}
