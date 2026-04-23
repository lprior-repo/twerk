//! Regular task scheduling logic.

use super::Scheduler;
use super::SchedulerError;
use anyhow::Result;

impl Scheduler {
    /// Schedules a regular (non-parallel, non-each) task.
    /// # Errors
    /// Returns error if task creation or broker publish fails.
    pub async fn schedule_regular_task(&self, mut task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();
        let job_id = task
            .job_id
            .clone()
            .ok_or_else(|| SchedulerError::JobIdRequired {
                scheduler: "regular".to_string(),
            })?;

        let job = self.ds.get_job_by_id(job_id.as_str()).await?;

        if let Some(defaults) = job.defaults.as_ref() {
            if task.queue.is_none() {
                task.queue = defaults.queue.clone();
            }
            if task.limits.is_none() {
                task.limits = defaults.limits.clone();
            }
            if task.timeout.is_none() {
                task.timeout = defaults.timeout.clone();
            }
            if task.retry.is_none() {
                task.retry = defaults.retry.clone();
            }
            if task.priority == 0 {
                task.priority = defaults.priority;
            }
        }

        task.state = twerk_core::task::TaskState::Scheduled;
        task.scheduled_at = Some(now);

        if task.queue.is_none() {
            task.queue = Some("default".to_string());
        }

        let q = task.queue.clone().unwrap_or_default();
        let t_queue = task.queue.clone();
        let t_limits = task.limits.clone();
        let t_timeout = task.timeout.clone();
        let t_retry = task.retry.clone();

        self.ds
            .update_task(
                &task_id,
                Box::new(move |mut u| {
                    u.state = twerk_core::task::TaskState::Scheduled;
                    u.scheduled_at = Some(now);
                    u.queue = t_queue;
                    u.limits = t_limits;
                    u.timeout = t_timeout;
                    u.retry = t_retry;
                    u.priority = task.priority;
                    Ok(u)
                }),
            )
            .await?;

        self.broker.publish_task(q, &task).await?;

        Ok(())
    }
}
