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

        // Get job_id as str reference to avoid clone
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| SchedulerError::JobIdRequired {
                scheduler: "regular".to_string(),
            })?;

        let job = self.ds.get_job_by_id(job_id).await?;

        // Apply defaults if present
        if let Some(defaults) = job.defaults.as_ref() {
            if task.queue.is_none() {
                task.queue.clone_from(&defaults.queue);
            }
            if task.limits.is_none() {
                task.limits.clone_from(&defaults.limits);
            }
            if task.timeout.is_none() {
                task.timeout.clone_from(&defaults.timeout);
            }
            if task.retry.is_none() {
                task.retry.clone_from(&defaults.retry);
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

        // Take ownership of fields we need to move into closure
        let q = task.queue.clone().unwrap_or_default();
        let queue = task.queue.take();
        let limits = task.limits.take();
        let timeout = task.timeout.take();
        let retry = task.retry.take();
        let priority = task.priority;

        self.ds
            .update_task(
                &task_id,
                Box::new(move |mut u| {
                    u.state = twerk_core::task::TaskState::Scheduled;
                    u.scheduled_at = Some(now);
                    u.queue = queue;
                    u.limits = limits;
                    u.timeout = timeout;
                    u.retry = retry;
                    u.priority = priority;
                    Ok(u)
                }),
            )
            .await?;

        // Restore task fields for publish (closure moved them)
        task.queue = Some(q.clone());
        task.state = twerk_core::task::TaskState::Scheduled;
        task.scheduled_at = Some(now);

        self.broker.publish_task(q, &task).await?;

        Ok(())
    }
}
