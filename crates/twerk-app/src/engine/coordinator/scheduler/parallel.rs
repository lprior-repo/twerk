//! Parallel task scheduling logic.

use super::shared::{
    create_and_publish_subtasks, job_context_map, mark_task_running, scheduler_ids,
};
use super::Scheduler;
use super::SchedulerError;
use anyhow::Result;
use rayon::prelude::{ParallelBridge, ParallelIterator};
use twerk_core::eval::evaluate_task;
use twerk_core::task::Task;
use twerk_core::uuid::new_short_uuid;

impl Scheduler {
    /// Schedules parallel tasks from a parallel task definition.
    /// # Errors
    /// Returns error if job retrieval or task creation fails.
    pub async fn schedule_parallel_task(&self, task: Task) -> Result<()> {
        let ids = scheduler_ids(&task, "parallel")?;
        let now = time::OffsetDateTime::now_utc();

        let job = self.ds.get_job_by_id(ids.job_id).await?;
        let job_ctx = job_context_map(&job);

        mark_task_running(self, ids.task_id, now).await?;

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

        let subtasks = build_parallel_subtasks(tasks, &job_ctx, ids.job_id, ids.task_id, now)?;
        create_and_publish_subtasks(self, &subtasks).await
    }
}

fn build_parallel_subtasks(
    tasks: &[Task],
    job_ctx: &std::collections::HashMap<String, serde_json::Value>,
    job_id: &str,
    parent_task_id: &str,
    now: time::OffsetDateTime,
) -> Result<Vec<Task>> {
    tasks
        .iter()
        .par_bridge()
        .map(|task| build_parallel_subtask(task, job_ctx, job_id, parent_task_id, now))
        .collect::<Result<Vec<_>>>()
}

fn build_parallel_subtask(
    task: &Task,
    job_ctx: &std::collections::HashMap<String, serde_json::Value>,
    job_id: &str,
    parent_task_id: &str,
    now: time::OffsetDateTime,
) -> Result<Task> {
    let evaluated = evaluate_task(task, job_ctx).map_err(|error| SchedulerError::Evaluation {
        context: "parallel task".to_string(),
        error: error.to_string(),
    })?;

    Ok(Task {
        id: Some(new_short_uuid().into()),
        job_id: Some(twerk_core::id::JobId::new(job_id.to_string())?),
        parent_id: Some(parent_task_id.to_string().into()),
        state: twerk_core::task::TaskState::Pending,
        created_at: Some(now),
        ..evaluated
    })
}
