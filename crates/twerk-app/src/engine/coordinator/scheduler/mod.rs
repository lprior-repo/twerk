//! Task scheduler for the coordinator
//!
//! Schedules tasks based on their type: regular, parallel, each, or subjob.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use anyhow::Result;
use std::sync::Arc;

use tracing::instrument;

mod each;
mod parallel;
mod regular;
mod shared;
mod subjob;

#[cfg(test)]
mod dag;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// ── Typed errors for scheduler operations ──────────────────────────

#[derive(Debug, thiserror::Error)]
pub(super) enum SchedulerError {
    #[error("task ID required for {scheduler} scheduling")]
    TaskIdRequired { scheduler: String },
    #[error("job ID required for {scheduler} scheduling")]
    JobIdRequired { scheduler: String },
    #[error("missing {scheduler} config")]
    MissingConfig { scheduler: String },
    #[error("missing parallel tasks")]
    MissingParallelTasks,
    #[error("each list must be an array")]
    EachListMustBeArray,
    #[error("missing each task template")]
    MissingEachTemplate,
    #[error("failed to evaluate {context}: {error}")]
    Evaluation { context: String, error: String },
    #[error("circular dependency detected involving task {task_id}")]
    CircularDependency { task_id: String },
    #[error("task not found: {task_id}")]
    TaskNotFound { task_id: String },
}

/// Scheduler handles task scheduling based on task type.
pub struct Scheduler {
    ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
    broker: Arc<dyn twerk_infrastructure::broker::Broker>,
}

impl Scheduler {
    /// Creates a new scheduler instance.
    #[must_use]
    pub fn new(
        ds: Arc<dyn twerk_infrastructure::datastore::Datastore>,
        broker: Arc<dyn twerk_infrastructure::broker::Broker>,
    ) -> Self {
        Self { ds, broker }
    }

    /// Schedules a task based on its type (regular, parallel, each, or subjob).
    /// # Errors
    /// Returns error if task scheduling fails.
    #[instrument(name = "schedule_task", skip_all)]
    pub async fn schedule_task(&self, task: twerk_core::task::Task) -> Result<()> {
        if task.parallel.is_some() {
            self.schedule_parallel_task(task).await
        } else if task.each.is_some() {
            self.schedule_each_task(task).await
        } else if task.subjob.is_some() {
            self.schedule_subjob_task(task).await
        } else {
            self.schedule_regular_task(task).await
        }
    }

    #[cfg(test)]
    pub(super) async fn submit_dag(&self, tasks: Vec<twerk_core::task::Task>) -> Result<()> {
        use twerk_core::id::TaskId;
        use twerk_core::task::TaskState;
        use std::collections::{HashMap, HashSet};

        if tasks.is_empty() {
            return Ok(());
        }

        let task_ids: HashSet<TaskId> = tasks.iter().filter_map(|t| t.id.clone()).collect();
        let mut adjacency: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();

        for task in &tasks {
            let id = task.id.as_ref().ok_or_else(|| {
                anyhow::anyhow!("task ID required")
            })?;
            in_degree.insert(id.clone(), 0);
            adjacency.insert(id.clone(), vec![]);
        }

        for task in &tasks {
            let id = task.id.as_ref().ok_or_else(|| anyhow::anyhow!("task ID required"))?;
            if let Some(deps) = &task.depends_on {
                for dep_id in deps {
                    if !task_ids.contains(dep_id) {
                        return Err(anyhow::anyhow!("dependency {} not found in submitted tasks", dep_id)).into();
                    }
                    adjacency.get_mut(dep_id).unwrap().push(id.clone());
                    *in_degree.get_mut(id).unwrap() += 1;
                }
            }
        }

        let mut queue: Vec<TaskId> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(id, _)| id.clone()).collect();
        let mut sorted: Vec<TaskId> = Vec::new();

        while let Some(node) = queue.pop() {
            sorted.push(node.clone());
            if let Some(neighbors) = adjacency.get(&node) {
                for neighbor in neighbors {
                    *in_degree.get_mut(neighbor).unwrap() -= 1;
                    if in_degree.get(neighbor) == Some(&0) {
                        queue.push(neighbor.clone());
                    }
                }
            }
        }

        if sorted.len() != tasks.len() {
            let remaining: Vec<TaskId> = in_degree.iter().filter(|(_, &d)| d > 0).map(|(id, _)| id.clone()).collect();
            if let Some(first) = remaining.first() {
                return Err(anyhow::anyhow!("circular dependency detected involving task {}", first)).into();
            }
        }

        for task_id in sorted {
            let task = tasks.iter().find(|t| t.id.as_ref() == Some(&task_id)).unwrap();
            if task.depends_on.as_ref().map_or(false, |deps| !deps.is_empty()) {
                self.ds.update_task(&task_id.to_string(), Box::new(|mut t| {
                    t.state = TaskState::Pending;
                    Ok(t)
                })).await.map_err(|e| anyhow::anyhow!(e))?;
            } else {
                self.schedule_task(task.clone()).await?;
            }
        }

        Ok(())
    }

    #[cfg(test)]
    pub(super) async fn mark_task_failed(&self, task_id: &twerk_core::id::TaskId) -> Result<()> {
        use twerk_core::task::TaskState;

        let task = self.ds.get_task_by_id(&task_id.to_string()).await
            .map_err(|_| SchedulerError::TaskNotFound { task_id: task_id.to_string() })?;

        if task.state != TaskState::Failed {
            self.ds.update_task(&task_id.to_string(), Box::new(|mut t| {
                t.state = TaskState::Failed;
                Ok(t)
            })).await.map_err(|e| anyhow::anyhow!(e))?;
        }

        self.propagate_cancellation(task_id).await?;
        Ok(())
    }

    #[cfg(test)]
    async fn propagate_cancellation(&self, failed_task_id: &twerk_core::id::TaskId) -> Result<()> {
        use twerk_core::task::TaskState;

        let failed_task = self.ds.get_task_by_id(&failed_task_id.to_string()).await
            .map_err(|e| anyhow::anyhow!(e))?;
        let job_id = failed_task.job_id.clone().ok_or_else(|| anyhow::anyhow!("task has no job_id"))?;

        let mut queue: Vec<twerk_core::id::TaskId> = vec![failed_task_id.clone()];

        while let Some(task_id) = queue.pop() {
            let all_tasks = self.ds.get_all_tasks_for_job(&job_id.to_string()).await
                .map_err(|e| anyhow::anyhow!(e))?;

            let dependents: Vec<twerk_core::id::TaskId> = all_tasks.iter()
                .filter(|t| t.depends_on.as_ref().map_or(false, |deps| deps.contains(&task_id)))
                .filter(|t| t.state.is_active())
                .map(|t| t.id.clone().unwrap())
                .collect();

            for dependent_id in dependents {
                self.ds.update_task(&dependent_id.to_string(), Box::new(|mut t| {
                    t.state = TaskState::Cancelled;
                    Ok(t)
                })).await.map_err(|e| anyhow::anyhow!(e))?;

                queue.push(dependent_id);
            }
        }

        Ok(())
    }
}
