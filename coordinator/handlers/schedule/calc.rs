//! Pure calculation functions for schedule handling.

use tork::job::{
    Job, JobContext, JobSchedule, ScheduledJob, JOB_STATE_PENDING, SCHEDULED_JOB_STATE_ACTIVE,
    SCHEDULED_JOB_STATE_PAUSED,
};

use crate::handlers::HandlerError;

// Minimum time a scheduled job lock should be held.
// Go: `minScheduledJobLockTTL = 10 * time.Second`
#[allow(dead_code)]
pub const MIN_SCHEDULED_JOB_LOCK_TTL: std::time::Duration = std::time::Duration::from_secs(10);

/// Result of analyzing a scheduled job state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleAction {
    /// Scheduled job should be activated (add cron entry).
    Activate,
    /// Scheduled job should be paused (remove cron entry).
    Pause,
}

/// Determines what action to take based on the scheduled job state.
/// Go: `switch s.State { case tork.ScheduledJobStateActive: ... case tork.ScheduledJobStatePaused: ... }`
#[must_use]
pub fn calculate_schedule_action(state: &str) -> Result<ScheduleAction, HandlerError> {
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
#[must_use]
pub fn create_job_from_scheduled(scheduled_job: &ScheduledJob) -> Job {
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
