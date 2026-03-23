//! Pure calculation functions for task completion.

use std::collections::HashMap;

use tork::job::Job;
use tork::task::{EachTask, ParallelTask, Task, TASK_STATE_COMPLETED, TASK_STATE_PENDING,
    TASK_STATE_RUNNING, TASK_STATE_SCHEDULED, TASK_STATE_SKIPPED};

pub fn validate_task_can_complete(state: &str) -> bool {
    *state == *TASK_STATE_RUNNING || *state == *TASK_STATE_SCHEDULED || *state == *TASK_STATE_SKIPPED
}

pub fn is_completion_state(state: &str) -> bool {
    *state == *TASK_STATE_COMPLETED || *state == *TASK_STATE_SKIPPED
}

pub fn calculate_progress(position: i64, task_count: i64) -> f64 {
    if task_count == 0 {
        return 0.0;
    }
    let raw = (position as f64) / (task_count as f64) * 100.0;
    (raw * 100.0).round() / 100.0
}

pub fn is_last_each_completion(completions: i64, size: i64) -> bool {
    completions >= size
}

pub fn is_last_parallel_completion(completions: i64, task_count: usize) -> bool {
    completions >= task_count as i64
}

pub fn has_next_task(position: i64, tasks_len: usize) -> bool {
    position <= tasks_len as i64
}

pub fn should_dispatch_next(concurrency: i64, index: i64, size: i64, is_last: bool) -> bool {
    !is_last && concurrency > 0 && index < size
}

pub fn update_context_map(
    existing: Option<HashMap<String, String>>,
    key: String,
    value: String,
) -> HashMap<String, String> {
    existing
        .unwrap_or_default()
        .into_iter()
        .chain(std::iter::once((key, value)))
        .collect()
}

pub fn create_next_task(
    task_def: &Task,
    job: &Job,
    position: i64,
    now: time::OffsetDateTime,
) -> Task {
    let new_id = uuid::Uuid::new_v4().to_string().replace('-', "");
    Task {
        id: Some(new_id),
        job_id: job.id.clone(),
        state: TASK_STATE_PENDING.clone(),
        position,
        created_at: Some(now),
        ..task_def.clone()
    }
}

pub fn increment_each(each: &EachTask) -> EachTask {
    EachTask {
        completions: each.completions + 1,
        index: each.index + 1,
        ..each.clone()
    }
}

pub fn increment_parallel(parallel: &ParallelTask) -> ParallelTask {
    ParallelTask {
        completions: parallel.completions + 1,
        ..parallel.clone()
    }
}
