//! Parallel task scheduling logic.

use super::Scheduler;
use super::SchedulerError;
use anyhow::Result;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use twerk_core::eval::evaluate_task;
use twerk_core::task::Task;
use twerk_core::uuid::new_short_uuid;
use twerk_infrastructure::broker::queue::QUEUE_PENDING;

impl Scheduler {
    /// Schedules parallel tasks from a parallel task definition.
    /// # Errors
    /// Returns error if job retrieval or task creation fails.
    pub async fn schedule_parallel_task(&self, task: Task) -> Result<()> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| SchedulerError::TaskIdRequired {
                scheduler: "parallel".to_string(),
            })?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| SchedulerError::JobIdRequired {
                scheduler: "parallel".to_string(),
            })?;
        let now = time::OffsetDateTime::now_utc();

        let job = self.ds.get_job_by_id(job_id).await?;
        let job_ctx = job
            .context
            .map_or_else(std::collections::HashMap::new, |c| c.as_map());

        self.ds
            .update_task(
                task_id,
                Box::new(move |u| {
                    Ok(Task {
                        state: twerk_core::task::TaskState::Running,
                        started_at: Some(now),
                        ..u
                    })
                }),
            )
            .await?;

        let parallel = task
            .parallel
            .as_ref()
            .ok_or_else(|| SchedulerError::MissingConfig {
                scheduler: "parallel".to_string(),
            })?;
        let tasks = parallel
            .tasks
            .as_ref()
            .ok_or(SchedulerError::MissingParallelTasks)?;

        let subtasks: Vec<_> = tasks
            .iter()
            .par_bridge()
            .map(|t| {
                let evaluated =
                    evaluate_task(t, &job_ctx).map_err(|e| SchedulerError::Evaluation {
                        context: "parallel task".to_string(),
                        error: e.to_string(),
                    })?;
                Ok(Task {
                    id: Some(new_short_uuid().into()),
                    job_id: Some(twerk_core::id::JobId::new(job_id.to_string())?),
                    parent_id: Some(task_id.to_string().into()),
                    state: twerk_core::task::TaskState::Pending,
                    created_at: Some(now),
                    ..evaluated
                })
            })
            .collect::<Result<Vec<_>>>()?;

        if !subtasks.is_empty() {
            self.ds.create_tasks(&subtasks).await?;
            if let Err(e) = self
                .broker
                .publish_tasks(QUEUE_PENDING.to_string(), &subtasks)
                .await
            {
                // Compensating rollback: tasks persisted but broker publish failed.
                // Mark all orphaned tasks as FAILED concurrently to prevent zombie state.
                let error_msg = format!("broker publish failed: {e}");
                let compensating: Vec<_> = subtasks
                    .iter()
                    .filter_map(|s| s.id.as_deref())
                    .map(|id| {
                        let msg = error_msg.clone();
                        self.ds.update_task(
                            id,
                            Box::new(move |t| {
                                Ok(Task {
                                    state: twerk_core::task::TaskState::Failed,
                                    error: Some(msg),
                                    ..t
                                })
                            }),
                        )
                    })
                    .collect();
                let _ = futures_util::future::join_all(compensating).await;
                return Err(e);
            }
        }

        Ok(())
    }
}
