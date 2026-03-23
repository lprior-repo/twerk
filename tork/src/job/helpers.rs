//! Helper functions for job cloning and construction.

use std::collections::HashMap;

use super::schedule::{Job, JobSummary, ScheduledJob, ScheduledJobSummary};

#[must_use]
pub fn new_job_summary(j: &Job) -> JobSummary {
    JobSummary {
        id: j.id.clone(),
        created_by: j.created_by.as_ref().map(|u| Box::new((**u).clone())),
        parent_id: j.parent_id.clone(),
        name: j.name.clone(),
        description: j.description.clone(),
        tags: j.tags.clone(),
        inputs: j.inputs.as_ref().map(HashMap::clone),
        state: j.state.clone(),
        created_at: j.created_at,
        started_at: j.started_at,
        completed_at: j.completed_at,
        failed_at: j.failed_at,
        position: j.position,
        task_count: j.task_count,
        result: j.result.clone(),
        error: j.error.clone(),
        progress: j.progress,
        schedule: j.schedule.as_ref().map(|s| Box::new((**s).clone())),
    }
}

#[must_use]
pub fn new_scheduled_job_summary(sj: &ScheduledJob) -> ScheduledJobSummary {
    ScheduledJobSummary {
        id: sj.id.clone(),
        created_by: sj.created_by.as_ref().map(|u| Box::new((**u).clone())),
        name: sj.name.clone(),
        state: sj.state.clone(),
        description: sj.description.clone(),
        tags: sj.tags.clone(),
        inputs: sj.inputs.as_ref().map(HashMap::clone),
        cron: sj.cron.clone(),
        created_at: sj.created_at,
    }
}
