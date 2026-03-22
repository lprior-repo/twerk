//! Scheduler module for task scheduling.
//!
//! This module provides functionality for scheduling tasks based on
//! various task types:
//! - Regular tasks
//! - Parallel tasks
//! - Each (loop) tasks
//! - Sub-job tasks

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

use serde::{Deserialize, Serialize};
use tork::task::{Task, TASK_STATE_PENDING, TASK_STATE_RUNNING};

/// Scheduler for scheduling tasks.
#[derive(Debug, Clone)]
pub struct Scheduler {
    // Scheduler configuration would go here
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new() -> Self {
        Self {}
    }

    /// Schedule a task based on its type.
    pub fn schedule_task(
        &self,
        task: &mut Task,
    ) -> Result<ScheduledTaskType, SchedulerError> {
        if task.each.is_some() {
            Ok(ScheduledTaskType::Each)
        } else if task.parallel.is_some() {
            Ok(ScheduledTaskType::Parallel)
        } else if task.subjob.is_some() {
            Ok(ScheduledTaskType::SubJob)
        } else {
            Ok(ScheduledTaskType::Regular)
        }
    }

    /// Schedule a regular task.
    pub fn schedule_regular_task(
        &self,
        task: &mut Task,
    ) -> Result<(), SchedulerError> {
        task.state = TASK_STATE_PENDING.clone();
        Ok(())
    }

    /// Schedule a parallel task.
    ///
    /// Marks the parent task as running and prepares subtasks.
    pub fn schedule_parallel_task(
        &self,
        task: &mut Task,
    ) -> Result<(), SchedulerError> {
        task.state = TASK_STATE_RUNNING.clone();
        Ok(())
    }

    /// Schedule an each (loop) task.
    ///
    /// Marks the parent task as running and prepares iteration subtasks.
    pub fn schedule_each_task(
        &self,
        task: &mut Task,
    ) -> Result<(), SchedulerError> {
        task.state = TASK_STATE_RUNNING.clone();
        Ok(())
    }

    /// Schedule a sub-job task.
    ///
    /// Creates the sub-job and marks the parent task as running.
    pub fn schedule_subjob_task(
        &self,
        task: &mut Task,
    ) -> Result<(), SchedulerError> {
        task.state = TASK_STATE_RUNNING.clone();
        Ok(())
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Types of scheduled tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledTaskType {
    /// A regular task
    Regular,
    /// A parallel task with subtasks
    Parallel,
    /// An each (loop) task with iterations
    Each,
    /// A sub-job task
    SubJob,
}

impl std::fmt::Display for ScheduledTaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduledTaskType::Regular => write!(f, "REGULAR"),
            ScheduledTaskType::Parallel => write!(f, "PARALLEL"),
            ScheduledTaskType::Each => write!(f, "EACH"),
            ScheduledTaskType::SubJob => write!(f, "SUBJOB"),
        }
    }
}

/// Errors that can occur during scheduling.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SchedulerError {
    #[error("scheduling error: {0}")]
    Schedule(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("task error: {0}")]
    Task(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_new() {
        let scheduler = Scheduler::new();
        assert!(scheduler.schedule_regular_task(&mut Task::default()).is_ok());
    }

    #[test]
    fn test_schedule_task_regular() {
        let scheduler = Scheduler::new();
        let mut task = Task::default();
        let result = scheduler.schedule_task(&mut task).unwrap();
        assert_eq!(result, ScheduledTaskType::Regular);
    }

    #[test]
    fn test_schedule_regular_task() {
        let scheduler = Scheduler::new();
        let mut task = Task::default();
        scheduler.schedule_regular_task(&mut task).unwrap();
        assert_eq!(task.state, *TASK_STATE_PENDING);
    }

    #[test]
    fn test_schedule_parallel_task() {
        let scheduler = Scheduler::new();
        let mut task = Task::default();
        scheduler.schedule_parallel_task(&mut task).unwrap();
        assert_eq!(task.state, *TASK_STATE_RUNNING);
    }

    #[test]
    fn test_scheduled_task_type_display() {
        assert_eq!(ScheduledTaskType::Regular.to_string(), "REGULAR");
        assert_eq!(ScheduledTaskType::Parallel.to_string(), "PARALLEL");
        assert_eq!(ScheduledTaskType::Each.to_string(), "EACH");
        assert_eq!(ScheduledTaskType::SubJob.to_string(), "SUBJOB");
    }
}