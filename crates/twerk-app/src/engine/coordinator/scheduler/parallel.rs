//! Parallel task scheduling logic.

use super::Scheduler;
use anyhow::Result;
use twerk_core::eval::evaluate_task;
use twerk_core::uuid::new_short_uuid;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

impl Scheduler {
    /// Schedules parallel tasks from a parallel task definition.
    /// # Errors
    /// Returns error if job retrieval or task creation fails.
    pub async fn schedule_parallel_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();

        let job = self.ds.get_job_by_id(&job_id).await?;
        let job_ctx = job
            .context
            .as_ref()
            .map(twerk_core::job::JobContext::as_map)
            .unwrap_or_default();

        self.ds
            .update_task(
                &task_id,
                Box::new(move |mut u| {
                    u.state = twerk_core::task::TASK_STATE_RUNNING.to_string();
                    u.started_at = Some(now);
                    Ok(u)
                }),
            )
            .await?;

        let parallel = task
            .parallel
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing parallel config"))?;
        let tasks = parallel
            .tasks
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing parallel tasks"))?;

        for t in tasks {
            let mut pt = t.clone();
            pt = evaluate_task(&pt, &job_ctx)
                .map_err(|e| anyhow::anyhow!("failed to evaluate parallel task: {e}"))?;

            pt.id = Some(new_short_uuid().into());
            pt.job_id = Some(job_id.clone());
            pt.parent_id = Some(task_id.to_string().into());
            pt.state = twerk_core::task::TASK_STATE_PENDING.to_string();
            pt.created_at = Some(now);

            self.ds.create_task(&pt).await?;
            self.broker
                .publish_task(QUEUE_PENDING.to_string(), &pt)
                .await?;
        }

        Ok(())
    }
}
