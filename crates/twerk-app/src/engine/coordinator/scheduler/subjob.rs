//! Subjob task scheduling logic.

use super::Scheduler;
use anyhow::Result;
use twerk_core::uuid::new_short_uuid;

impl Scheduler {
    /// Schedules a subjob task.
    /// # Errors
    /// Returns error if job creation or publish fails.
    pub async fn schedule_subjob_task(&self, task: twerk_core::task::Task) -> Result<()> {
        let task_id = task.id.clone().unwrap_or_default();
        let job_id = task.job_id.clone().unwrap_or_default();
        let now = time::OffsetDateTime::now_utc();

        let job = self.ds.get_job_by_id(&job_id).await?;

        let subjob_task = task
            .subjob
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing subjob config"))?;

        let subjob = twerk_core::job::Job {
            id: Some(new_short_uuid().into()),
            parent_id: Some(task_id.to_string().into()),
            name: subjob_task.name.clone(),
            description: subjob_task.description.clone(),
            state: twerk_core::job::JobState::Pending,
            tasks: subjob_task.tasks.clone(),
            inputs: subjob_task.inputs.clone(),
            secrets: subjob_task.secrets.clone(),
            task_count: subjob_task.tasks.as_ref().map_or(0, |t| t.len() as i64),
            output: subjob_task.output.clone(),
            webhooks: subjob_task.webhooks.clone(),
            auto_delete: subjob_task.auto_delete.clone(),
            created_at: Some(now),
            created_by: job.created_by.clone(),
            ..Default::default()
        };

        let subjob_id = subjob.id.clone().unwrap_or_default();

        self.ds
            .update_task(
                &task_id,
                Box::new(move |mut u| {
                    u.state = twerk_core::task::TaskState::Running;
                    u.started_at = Some(now);
                    if let Some(ref mut sj) = u.subjob {
                        sj.id = Some(subjob_id.clone());
                    }
                    Ok(u)
                }),
            )
            .await?;

        self.ds.create_job(&subjob).await?;
        self.broker.publish_job(&subjob).await?;

        Ok(())
    }
}
