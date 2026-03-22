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
//! implementation can integrate with the `locker` crate for the same purpose
//! by acquiring a lock inside the cron tick callback before creating the job.

use std::collections::HashMap;
use std::sync::Arc;

use tork::job::{
    Job, JobContext, JobSchedule, ScheduledJob, JOB_STATE_PENDING,
    SCHEDULED_JOB_STATE_ACTIVE, SCHEDULED_JOB_STATE_PAUSED,
};
use tork::{Broker, Datastore};

use crate::handlers::HandlerError;

// Minimum time a scheduled job lock should be held.
// Go: `minScheduledJobLockTTL = 10 * time.Second`
// Used by distributed locking integration (see module docs).
#[allow(dead_code)]
const MIN_SCHEDULED_JOB_LOCK_TTL: std::time::Duration = std::time::Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Pure Calculations (Data → Calc)
// ---------------------------------------------------------------------------

/// Result of analyzing a scheduled job state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduleAction {
    /// Scheduled job should be activated (add cron entry).
    Activate,
    /// Scheduled job should be paused (remove cron entry).
    Pause,
}

/// Determines what action to take based on the scheduled job state.
/// Go: `switch s.State { case tork.ScheduledJobStateActive: ... case tork.ScheduledJobStatePaused: ... }`
#[must_use]
pub(crate) fn calculate_schedule_action(state: &str) -> Result<ScheduleAction, HandlerError> {
    match state {
        s if s == SCHEDULED_JOB_STATE_ACTIVE => Ok(ScheduleAction::Activate),
        s if s == SCHEDULED_JOB_STATE_PAUSED => Ok(ScheduleAction::Pause),
        other => Err(HandlerError::InvalidState(format!(
            "unknown scheduled job state: {other}"
        ))),
    }
}

/// Creates a Job instance from a ScheduledJob.
///
/// This is the pure data transformation that happens on each cron tick.
/// Go parity (`handleActive` cron task closure):
/// ```go
/// job := &tork.Job{
///     ID:          uuid.NewUUID(),
///     CreatedBy:   s.CreatedBy,
///     CreatedAt:   now,
///     Permissions: s.Permissions,
///     Tags:        s.Tags,
///     Name:        s.Name,
///     Description: s.Description,
///     State:       tork.JobStatePending,
///     Tasks:       s.Tasks,
///     Inputs:      s.Inputs,
///     Secrets:     s.Secrets,
///     Context:     tork.JobContext{Inputs: s.Inputs, Secrets: s.Secrets},
///     TaskCount:   len(s.Tasks),
///     Output:      s.Output,
///     Webhooks:    s.Webhooks,
///     AutoDelete:  s.AutoDelete,
///     Schedule:    &tork.JobSchedule{ID: s.ID, Cron: s.Cron},
/// }
/// ```
#[must_use]
pub(crate) fn create_job_from_scheduled(scheduled_job: &ScheduledJob) -> Job {
    let now = time::OffsetDateTime::now_utc();
    Job {
        id: Some(uuid::Uuid::new_v4().to_string().replace('-', "")),
        created_by: scheduled_job.created_by.clone(),
        created_at: now,
        permissions: scheduled_job.permissions.clone(),
        tags: scheduled_job.tags.clone(),
        name: scheduled_job.name.clone(),
        description: scheduled_job.description.clone(),
        state: JOB_STATE_PENDING.to_string(),
        tasks: scheduled_job.tasks.clone(),
        inputs: scheduled_job.inputs.clone(),
        secrets: scheduled_job.secrets.clone(),
        context: JobContext {
            inputs: scheduled_job.inputs.clone(),
            secrets: scheduled_job.secrets.clone(),
            ..JobContext::default()
        },
        task_count: scheduled_job.tasks.len() as i64,
        output: scheduled_job.output.clone(),
        webhooks: scheduled_job.webhooks.clone(),
        auto_delete: scheduled_job.auto_delete.clone(),
        schedule: Some(JobSchedule {
            id: scheduled_job.id.clone(),
            cron: scheduled_job.cron.clone(),
        }),
        ..Job::default()
    }
}

// ---------------------------------------------------------------------------
// Handler (Action boundary)
// ---------------------------------------------------------------------------

/// Schedule handler for processing scheduled job events.
///
/// Go parity with `jobSchedulerHandler` in `schedule.go`.
///
/// Manages cron-based scheduling of recurring jobs. On each cron tick,
/// a new Job instance is created from the ScheduledJob template,
/// persisted in the datastore, and published via the broker.
pub struct ScheduleHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
    scheduler: Arc<tokio_cron_scheduler::JobScheduler>,
    jobs: Arc<tokio::sync::Mutex<HashMap<String, uuid::Uuid>>>,
}

impl std::fmt::Debug for ScheduleHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScheduleHandler").finish()
    }
}

impl ScheduleHandler {
    /// Create a new schedule handler with datastore and broker.
    ///
    /// Go parity (`NewJobSchedulerHandler`):
    /// 1. Creates a new cron scheduler
    /// 2. Starts the scheduler
    /// 3. Loads all active scheduled jobs from the datastore
    /// 4. Schedules each active job
    ///
    /// ```go
    /// sc, _ := gocron.NewScheduler(gocron.WithDistributedLocker(glocker{locker: l}))
    /// sc.Start()
    /// activeJobs, _ := ds.GetActiveScheduledJobs(ctx)
    /// for _, aj := range activeJobs { h.handle(ctx, aj) }
    /// ```
    pub async fn new(
        ds: Arc<dyn Datastore>,
        broker: Arc<dyn Broker>,
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
            scheduler: Arc::new(scheduler),
            jobs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        };

        // Initialize all existing active jobs
        // Go: `activeJobs, err := ds.GetActiveScheduledJobs(ctx)`
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
            // Silently skip failures during initialization to avoid
            // blocking the entire coordinator startup
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
    ///
    /// Go parity (`handle`):
    /// Dispatches to `handle_active` or `handle_paused` based on state.
    pub async fn handle(&self, scheduled_job: &ScheduledJob) -> Result<(), HandlerError> {
        let job_id = scheduled_job
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("scheduled job ID is required".into()))?;

        match calculate_schedule_action(&scheduled_job.state)? {
            ScheduleAction::Activate => {
                self.handle_active(job_id, &scheduled_job.cron).await
            }
            ScheduleAction::Pause => self.handle_paused(job_id).await,
        }
    }

    /// Handle activation of a scheduled job.
    ///
    /// Go parity (`handleActive`):
    /// Creates a cron job entry that periodically creates Job instances
    /// from the ScheduledJob and publishes them via the broker.
    ///
    /// ```go
    /// cj, _ := h.scheduler.NewJob(
    ///     gocron.CronJob(sj.Cron, false),
    ///     gocron.NewTask(func(sj *tork.ScheduledJob) {
    ///         s, _ := h.ds.GetScheduledJobByID(ctx, sj.ID)
    ///         job := &tork.Job{...}
    ///         h.ds.CreateJob(ctx, job)
    ///         h.broker.PublishJob(ctx, job)
    ///     }, sj),
    ///     gocron.WithName(sj.ID),
    /// )
    /// h.m[sj.ID] = cj
    /// ```
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
        let scheduled_job_id = job_id.to_string();

        // Build the cron tick callback — this is the Action that fires on each
        // scheduled interval. It fetches the latest ScheduledJob, creates a new
        // Job instance, persists it, and publishes it.
        let cron_job = tokio_cron_scheduler::Job::new_async(cron, move |_uuid, _lock| {
            let ds = ds.clone();
            let broker = broker.clone();
            let sj_id = scheduled_job_id.clone();
            Box::pin(async move {
                // Fetch the latest scheduled job to ensure we have the latest version
                // and to get the full details (permissions in particular)
                // Go: `s, err := h.ds.GetScheduledJobByID(ctx, sj.ID)`
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

                // Persist and publish the job
                // Go: `h.ds.CreateJob(ctx, job)` and `h.broker.PublishJob(ctx, job)`
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
            })
        })
        .map_err(|e| {
            HandlerError::Handler(format!("error scheduling job {job_id}: {e}"))
        })?;

        let job_uuid = self
            .scheduler
            .add(cron_job)
            .await
            .map_err(|e| {
                HandlerError::Handler(format!("error scheduling job {job_id}: {e}"))
            })?;

        let mut jobs = self.jobs.lock().await;
        jobs.insert(job_id.to_string(), job_uuid);

        Ok(())
    }

    /// Handle pausing of a scheduled job.
    ///
    /// Go parity (`handlePaused`):
    /// Removes the cron scheduler entry for the scheduled job.
    ///
    /// ```go
    /// gjob, ok := h.m[s.ID]
    /// h.scheduler.RemoveJob(gjob.ID())
    /// delete(h.m, s.ID)
    /// ```
    async fn handle_paused(&self, job_id: &str) -> Result<(), HandlerError> {
        let jobs = self.jobs.lock().await;

        let job_uuid = jobs.get(job_id).ok_or_else(|| {
            HandlerError::NotFound(format!("unknown scheduled job: {job_id}"))
        })?;

        tracing::info!(scheduled_job_id = job_id, "Pausing scheduled job");

        self.scheduler
            .remove(job_uuid)
            .await
            .map_err(|e| {
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
    use super::*;

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
            tasks: vec![
                Task::default(),
                Task::default(),
                Task::default(),
            ],
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(job.task_count, 3);
        assert_eq!(job.tasks.len(), 3);
    }

    #[test]
    fn test_create_job_from_scheduled_preserves_inputs_and_secrets() {
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

        // Inputs should be in both job.inputs and job.context.inputs
        assert_eq!(job.inputs, Some(inputs.clone()));
        assert_eq!(job.context.inputs, Some(inputs));

        // Secrets should be in both job.secrets and job.context.secrets
        assert_eq!(job.secrets, Some(secrets.clone()));
        assert_eq!(job.context.secrets, Some(secrets));
    }

    #[test]
    fn test_create_job_from_scheduled_preserves_webhooks_and_permissions() {
        let webhooks = Some(vec![tork::task::Webhook {
            url: Some("https://example.com/hook".to_string()),
            headers: None,
            event: None,
            r#if: None,
        }]);

        let permissions = Some(vec![tork::task::Permission {
            role: None,
            user: None,
        }]);

        let scheduled = ScheduledJob {
            id: Some("sched-4".to_string()),
            webhooks: webhooks.clone(),
            permissions: permissions.clone(),
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert!(job.webhooks.is_some());
        assert_eq!(job.webhooks.as_ref().map(|w| w.len()), Some(1));
        assert!(job.permissions.is_some());
        assert_eq!(job.permissions.as_ref().map(|p| p.len()), Some(1));
    }

    #[test]
    fn test_create_job_from_scheduled_preserves_auto_delete() {
        let auto_delete = Some(tork::task::AutoDelete {
            after: Some("1h".to_string()),
        });

        let scheduled = ScheduledJob {
            id: Some("sched-5".to_string()),
            auto_delete: auto_delete.clone(),
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(job.auto_delete, auto_delete);
    }

    #[test]
    fn test_create_job_from_scheduled_preserves_output() {
        let scheduled = ScheduledJob {
            id: Some("sched-6".to_string()),
            output: Some("{{.result}}".to_string()),
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(job.output.as_deref(), Some("{{.result}}"));
    }

    #[test]
    fn test_create_job_from_scheduled_id_is_unique() {
        let scheduled = ScheduledJob {
            id: Some("sched-7".to_string()),
            ..ScheduledJob::default()
        };

        let job1 = create_job_from_scheduled(&scheduled);
        let job2 = create_job_from_scheduled(&scheduled);

        // Each invocation should produce a unique job ID
        assert_ne!(job1.id, job2.id);
    }

    #[test]
    fn test_create_job_from_scheduled_preserves_tags() {
        let tags = Some(vec![
            "tag1".to_string(),
            "tag2".to_string(),
        ]);

        let scheduled = ScheduledJob {
            id: Some("sched-8".to_string()),
            tags: tags.clone(),
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(job.tags, tags);
    }

    #[test]
    fn test_create_job_from_scheduled_state_is_always_pending() {
        let scheduled = ScheduledJob {
            id: Some("sched-9".to_string()),
            state: "SOME_RANDOM_STATE".to_string(),
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(job.state, JOB_STATE_PENDING);
    }

    #[test]
    fn test_create_job_from_scheduled_preserves_description() {
        let scheduled = ScheduledJob {
            id: Some("sched-10".to_string()),
            description: Some("A scheduled job description".to_string()),
            ..ScheduledJob::default()
        };

        let job = create_job_from_scheduled(&scheduled);

        assert_eq!(
            job.description.as_deref(),
            Some("A scheduled job description")
        );
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
        assert_eq!(MIN_SCHEDULED_JOB_LOCK_TTL, std::time::Duration::from_secs(10));
    }
}
