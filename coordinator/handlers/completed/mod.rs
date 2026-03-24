//! Completed handler for task completion events.

pub mod calc;
pub mod eval;

use std::pin::Pin;
use std::sync::Arc;

use tork::broker::queue;
use tork::broker::TOPIC_JOB_PROGRESS;
use tork::job::{Job, JobContext, JOB_STATE_COMPLETED};
use tork::task::{
    Task, TASK_STATE_COMPLETED as TASK_STATE_COMPLETED_CONST, TASK_STATE_FAILED, TASK_STATE_PENDING,
};
use tork::{Broker, Datastore};

use crate::handlers::HandlerError;

use self::calc::{
    calculate_progress, create_next_task, has_next_task, increment_each, increment_parallel,
    is_completion_state, is_last_each_completion, is_last_parallel_completion,
    should_dispatch_next, update_context_map, validate_task_can_complete,
};
use self::eval::evaluate_task;

pub struct CompletedHandler {
    ds: Arc<dyn Datastore>,
    broker: Arc<dyn Broker>,
}

impl std::fmt::Debug for CompletedHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletedHandler").finish()
    }
}

impl CompletedHandler {
    pub fn new(ds: Arc<dyn Datastore>, broker: Arc<dyn Broker>) -> Self {
        Self { ds, broker }
    }

    pub async fn handle(&self, task: &Task) -> Result<(), HandlerError> {
        if !is_completion_state(&task.state) {
            return Err(HandlerError::InvalidState(format!(
                "invalid completion state: {}",
                task.state
            )));
        }

        let task = Task {
            completed_at: Some(time::OffsetDateTime::now_utc()),
            ..task.clone()
        };

        self.complete_task(&task).await
    }

    fn complete_task<'a>(
        &'a self,
        task: &'a Task,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), HandlerError>> + Send + 'a>> {
        Box::pin(async move {
            match &task.parent_id {
                Some(_) => self.complete_sub_task(task).await,
                None => self.complete_top_level_task(task).await,
            }
        })
    }

    async fn complete_sub_task(&self, task: &Task) -> Result<(), HandlerError> {
        let parent_id = task
            .parent_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("parent_id is required".into()))?;

        let parent = self
            .ds
            .get_task_by_id(parent_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("parent task {parent_id} not found")))?;

        match &parent.parallel {
            Some(_) => self.complete_parallel_task(task).await,
            None => self.complete_each_task(task).await,
        }
    }

    async fn complete_each_task(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let parent_id = task
            .parent_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("parent_id is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        let current = self
            .ds
            .get_task_by_id(task_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("task {task_id} not found")))?;

        if !validate_task_can_complete(&current.state) {
            return Err(HandlerError::InvalidState(format!(
                "can't complete task {task_id} because it's {}",
                current.state
            )));
        }

        let updated_task = Task {
            state: task.state.clone(),
            completed_at: task.completed_at,
            result: task.result.clone(),
            ..current
        };
        self.ds
            .update_task(task_id.to_string(), updated_task)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let parent = self
            .ds
            .get_task_by_id(parent_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("parent task {parent_id} not found")))?;

        let each = parent
            .each
            .as_ref()
            .ok_or_else(|| HandlerError::Validation("parent has no each configuration".into()))?;

        let is_last = is_last_each_completion(each.completions + 1, each.size);
        let dispatch_next = should_dispatch_next(each.concurrency, each.index, each.size, is_last);

        if dispatch_next {
            if let Some(next_task) = self
                .ds
                .get_next_task(parent_id.to_string())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?
            {
                let next_id = next_task
                    .id
                    .clone()
                    .ok_or_else(|| HandlerError::Validation("next task has no ID".into()))?;
                let pending_task = Task {
                    state: TASK_STATE_PENDING.clone(),
                    ..next_task
                };
                self.ds
                    .update_task(next_id, pending_task.clone())
                    .await
                    .map_err(|e| HandlerError::Datastore(e.to_string()))?;
                self.broker
                    .publish_task(queue::QUEUE_PENDING.to_string(), &pending_task)
                    .await
                    .map_err(|e| HandlerError::Broker(e.to_string()))?;
            }
        }

        let updated_each = increment_each(each);
        let updated_parent = Task {
            each: Some(updated_each),
            ..parent
        };
        self.ds
            .update_task(parent_id.to_string(), updated_parent)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let (Some(var), Some(result)) = (&task.var, &task.result) {
            self.update_job_context(job_id, var.clone(), result.clone())
                .await?;
        }

        if is_last {
            let parent = self
                .ds
                .get_task_by_id(parent_id.to_string())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?
                .ok_or_else(|| {
                    HandlerError::NotFound(format!("parent task {parent_id} not found"))
                })?;

            let now = time::OffsetDateTime::now_utc();
            let completed_parent = Task {
                state: TASK_STATE_COMPLETED_CONST.clone(),
                completed_at: Some(now),
                ..parent
            };
            return self.complete_task(&completed_parent).await;
        }

        Ok(())
    }

    async fn complete_parallel_task(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let parent_id = task
            .parent_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("parent_id is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        let current = self
            .ds
            .get_task_by_id(task_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("task {task_id} not found")))?;

        if !validate_task_can_complete(&current.state) {
            return Err(HandlerError::InvalidState(format!(
                "can't complete task {task_id} because it's {}",
                current.state
            )));
        }

        let updated_task = Task {
            state: task.state.clone(),
            completed_at: task.completed_at,
            result: task.result.clone(),
            ..current
        };
        self.ds
            .update_task(task_id.to_string(), updated_task)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let parent = self
            .ds
            .get_task_by_id(parent_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("parent task {parent_id} not found")))?;

        let parallel = parent.parallel.as_ref().ok_or_else(|| {
            HandlerError::Validation("parent has no parallel configuration".into())
        })?;

        let parallel_task_count = parallel.tasks.as_ref().map_or(0, Vec::len);
        let is_last = is_last_parallel_completion(parallel.completions + 1, parallel_task_count);

        let updated_parallel = increment_parallel(parallel);
        let updated_parent = Task {
            parallel: Some(updated_parallel),
            ..parent
        };
        self.ds
            .update_task(parent_id.to_string(), updated_parent)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        if let (Some(var), Some(result)) = (&task.var, &task.result) {
            self.update_job_context(job_id, var.clone(), result.clone())
                .await?;
        }

        if is_last {
            let parent = self
                .ds
                .get_task_by_id(parent_id.to_string())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?
                .ok_or_else(|| {
                    HandlerError::NotFound(format!("parent task {parent_id} not found"))
                })?;

            let now = time::OffsetDateTime::now_utc();
            let completed_parent = Task {
                state: TASK_STATE_COMPLETED_CONST.clone(),
                completed_at: Some(now),
                ..parent
            };
            return self.complete_task(&completed_parent).await;
        }

        Ok(())
    }

    async fn complete_top_level_task(&self, task: &Task) -> Result<(), HandlerError> {
        let task_id = task
            .id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("task ID is required".into()))?;
        let job_id = task
            .job_id
            .as_deref()
            .ok_or_else(|| HandlerError::Validation("job ID is required".into()))?;

        let current = self
            .ds
            .get_task_by_id(task_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("task {task_id} not found")))?;

        if !validate_task_can_complete(&current.state) {
            return Err(HandlerError::InvalidState(format!(
                "can't complete task {task_id} because it's {}",
                current.state
            )));
        }

        let updated_task = Task {
            state: task.state.clone(),
            completed_at: task.completed_at,
            result: task.result.clone(),
            ..current
        };
        self.ds
            .update_task(task_id.to_string(), updated_task)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job {job_id} not found")))?;

        let progress = calculate_progress(job.position, job.task_count);
        let new_position = job.position + 1;

        let updated_context = if let (Some(var), Some(result)) = (&task.var, &task.result) {
            let new_tasks =
                update_context_map(job.context.tasks.clone(), var.clone(), result.clone());
            JobContext {
                tasks: Some(new_tasks),
                ..job.context.clone()
            }
        } else {
            job.context.clone()
        };

        let updated_job = Job {
            progress,
            position: new_position,
            context: updated_context,
            ..job.clone()
        };
        self.ds
            .update_job(job_id.to_string(), updated_job)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        let updated_job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job {job_id} not found")))?;

        // Publish progress event (matches Go's onJob(ctx, job.Progress, j))
        let event_payload = serde_json::to_value(&updated_job)
            .map_err(|e| HandlerError::Handler(format!("failed to serialize job: {e}")))?;
        self.broker
            .publish_event(TOPIC_JOB_PROGRESS.to_string(), event_payload)
            .await
            .map_err(|e| HandlerError::Broker(e.to_string()))?;

        if has_next_task(updated_job.position, updated_job.tasks.len()) {
            let next_idx = usize::try_from(updated_job.position - 1)
                .map_err(|_| HandlerError::Validation("position overflow".into()))?;
            let task_def = updated_job
                .tasks
                .get(next_idx)
                .ok_or_else(|| HandlerError::NotFound("next task definition not found".into()))?;

            let now = time::OffsetDateTime::now_utc();
            let next_task = create_next_task(task_def, &updated_job, new_position, now);

            let ctx_map = updated_job.context.as_map();
            let evaluated_task = match evaluate_task(&next_task, &ctx_map) {
                Ok(t) => t,
                Err(eval_err) => {
                    let mut failed = next_task;
                    failed.error = Some(eval_err);
                    failed.state = TASK_STATE_FAILED.clone();
                    failed.failed_at = Some(now);
                    failed
                }
            };

            self.ds
                .create_task(evaluated_task.clone())
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?;
            self.broker
                .publish_task(queue::QUEUE_PENDING.to_string(), &evaluated_task)
                .await
                .map_err(|e| HandlerError::Broker(e.to_string()))?;
        } else {
            let now = time::OffsetDateTime::now_utc();
            let completed_job = Job {
                state: JOB_STATE_COMPLETED.to_string(),
                completed_at: Some(now),
                ..updated_job.clone()
            };
            self.ds
                .update_job(job_id.to_string(), completed_job)
                .await
                .map_err(|e| HandlerError::Datastore(e.to_string()))?;
        }

        Ok(())
    }

    async fn update_job_context(
        &self,
        job_id: &str,
        var: String,
        result: String,
    ) -> Result<(), HandlerError> {
        let job = self
            .ds
            .get_job_by_id(job_id.to_string())
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?
            .ok_or_else(|| HandlerError::NotFound(format!("job {job_id} not found")))?;

        let new_tasks = update_context_map(job.context.tasks.clone(), var, result);
        let updated_context = JobContext {
            tasks: Some(new_tasks),
            ..job.context.clone()
        };
        let updated_job = Job {
            context: updated_context,
            ..job
        };
        self.ds
            .update_job(job_id.to_string(), updated_job)
            .await
            .map_err(|e| HandlerError::Datastore(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tork::task::{TASK_STATE_RUNNING, TASK_STATE_SCHEDULED, TASK_STATE_SKIPPED};

    #[test]
    fn test_validate_task_can_complete() {
        assert!(validate_task_can_complete(&TASK_STATE_RUNNING));
        assert!(validate_task_can_complete(&TASK_STATE_SCHEDULED));
        assert!(validate_task_can_complete(&TASK_STATE_SKIPPED));
        assert!(!validate_task_can_complete(&TASK_STATE_PENDING));
        assert!(!validate_task_can_complete(&TASK_STATE_COMPLETED_CONST));
    }

    #[test]
    fn test_is_completion_state() {
        assert!(is_completion_state(&TASK_STATE_COMPLETED_CONST));
        assert!(is_completion_state(&TASK_STATE_SKIPPED));
        assert!(!is_completion_state(&TASK_STATE_PENDING));
    }

    #[test]
    fn test_calculate_progress() {
        assert_eq!(calculate_progress(0, 10), 0.0);
        assert_eq!(calculate_progress(5, 10), 50.0);
        assert_eq!(calculate_progress(10, 10), 100.0);
        assert_eq!(calculate_progress(1, 3), 33.33);
    }

    #[test]
    fn test_is_last_each_completion() {
        assert!(is_last_each_completion(5, 5));
        assert!(is_last_each_completion(6, 5));
        assert!(!is_last_each_completion(4, 5));
    }

    #[test]
    fn test_is_last_parallel_completion() {
        assert!(is_last_parallel_completion(3, 3));
        assert!(is_last_parallel_completion(4, 3));
        assert!(!is_last_parallel_completion(2, 3));
    }

    #[test]
    fn test_has_next_task() {
        assert!(has_next_task(0, 5));
        assert!(has_next_task(1, 5));
        assert!(has_next_task(4, 5));
        assert!(!has_next_task(5, 5));
        assert!(!has_next_task(6, 5));
    }

    #[test]
    fn test_should_dispatch_next() {
        assert!(!should_dispatch_next(2, 0, 5, true));
        assert!(!should_dispatch_next(0, 0, 5, false));
        assert!(should_dispatch_next(2, 0, 5, false));
        assert!(!should_dispatch_next(2, 5, 5, false));
    }

    #[test]
    fn test_update_context_map() {
        use std::collections::HashMap;
        let result = update_context_map(None, "key1".to_string(), "val1".to_string());
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("key1"), Some(&"val1".to_string()));

        let existing = Some(HashMap::from([("key0".to_string(), "val0".to_string())]));
        let result = update_context_map(existing, "key1".to_string(), "val1".to_string());
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("key0"), Some(&"val0".to_string()));
        assert_eq!(result.get("key1"), Some(&"val1".to_string()));
    }
}
