//! Regular task scheduling logic.

use super::Scheduler;
use anyhow::Result;

impl Scheduler {
    /// Schedules a regular (non-parallel, non-each) task.
    /// # Errors
    /// Returns error if task creation or broker publish fails.
    pub async fn schedule_regular_task(&self, mut task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();

        task.state = twerk_core::task::TaskState::Scheduled;
        task.scheduled_at = Some(now);

        if task.queue.is_none() {
            task.queue = Some("default".to_string());
        }

        let q = task.queue.clone().unwrap_or_default();
        let t_queue = task.queue.clone();

        self.ds
            .update_task(
                &task_id,
                Box::new(move |mut u| {
                    u.state = twerk_core::task::TaskState::Scheduled;
                    u.scheduled_at = Some(now);
                    u.queue = t_queue;
                    Ok(u)
                }),
            )
            .await?;

        self.broker.publish_task(q, &task).await?;

        Ok(())
    }
}
