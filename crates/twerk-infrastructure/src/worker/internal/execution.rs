//! Task execution module.
//!
//! Pure task execution logic with cancellation and limit application.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use dashmap::DashMap;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{debug, instrument, warn};

use twerk_common::constants::DEFAULT_TASK_NAME;
use twerk_core::id::TaskId;
use twerk_core::task::{Task, TaskLimits, TaskState};

use crate::broker::Broker;
use crate::runtime::Runtime as RuntimeTrait;

use super::types::{Limits, RunningTask};

// ── Typed errors for task execution ────────────────────────────────

#[derive(Debug, thiserror::Error)]
enum TaskExecutionError {
    #[error("task timed out")]
    Timeout,
}

/// Execute a task
#[instrument(skip_all, fields(task_id = ?task.id))]
pub async fn execute_task(
    task: Arc<Task>,
    runtime: Arc<dyn RuntimeTrait>,
    broker: Arc<dyn Broker>,
    limits: Limits,
    active_tasks: Arc<DashMap<TaskId, RunningTask>>,
    tasks_notify: Arc<tokio::sync::Notify>,
) -> Result<()> {
    let tid = task.id.clone();

    // Create cancellation channel
    let (cancel_tx, mut cancel_rx) = broadcast::channel(1);

    // Track running task
    if let Some(ref id) = tid {
        let running = RunningTask { cancel_tx };
        active_tasks.insert(id.clone(), running);
    }

    // Apply default limits
    let mut t = (*task).clone();
    apply_limits(&mut t, &limits);

    // Update task state
    t.state = TaskState::Running;
    t.started_at = Some(OffsetDateTime::now_utc());

    broker.publish_task_progress(&t).await?;

    // Run the task with cancellation support
    let result = run_task_with_cancel(&t, runtime.clone(), &mut cancel_rx).await;

    // Update final state
    match result {
        Ok(()) => {
            t.state = TaskState::Completed;
            t.completed_at = Some(OffsetDateTime::now_utc());
        }
        Err(e) => {
            t.state = TaskState::Failed;
            t.failed_at = Some(OffsetDateTime::now_utc());
            t.error = Some(e.to_string());
        }
    }

    // Remove from active tasks
    if let Some(ref id) = tid {
        active_tasks.remove(id);
        tasks_notify.notify_waiters();
    }

    broker.publish_task_progress(&t).await
}

/// Run a task with support for cancellation
async fn run_task_with_cancel(
    t: &Task,
    runtime: Arc<dyn RuntimeTrait>,
    cancel_rx: &mut broadcast::Receiver<()>,
) -> Result<()> {
    let task_id_str = t.id.as_deref().unwrap_or(DEFAULT_TASK_NAME);
    let timeout = t.timeout.clone();

    if let Some(dur) = timeout.as_ref().and_then(|s| parse_duration(s)) {
        run_with_timeout(t, runtime, cancel_rx, task_id_str, dur).await
    } else {
        run_without_timeout(t, runtime, cancel_rx, task_id_str).await
    }
}

async fn run_with_timeout(
    t: &Task,
    runtime: Arc<dyn RuntimeTrait>,
    cancel_rx: &mut broadcast::Receiver<()>,
    task_id_str: &str,
    dur: Duration,
) -> Result<()> {
    let timeout_str = t.timeout.clone().unwrap_or_default();
    tokio::select! {
        result = runtime.run(t) => result,
        _ = cancel_rx.recv() => {
            debug!("Task {} cancelled", task_id_str);
            Ok(())
        },
        () = sleep(dur) => {
            warn!("Task {} timed out after {}", task_id_str, timeout_str);
            let _ = runtime.stop(t).await;
            Err(TaskExecutionError::Timeout.into())
        }
    }
}

async fn run_without_timeout(
    t: &Task,
    runtime: Arc<dyn RuntimeTrait>,
    cancel_rx: &mut broadcast::Receiver<()>,
    task_id_str: &str,
) -> Result<()> {
    tokio::select! {
        result = runtime.run(t) => result,
        _ = cancel_rx.recv() => {
            debug!("Task {} cancelled", task_id_str);
            Ok(())
        }
    }
}

/// Apply default limits to a task
fn apply_limits(task: &mut Task, limits: &Limits) {
    let has_cpu_limit = !limits.default_cpus_limit.is_empty();
    let has_mem_limit = !limits.default_memory_limit.is_empty();

    if task.limits.is_none() && (has_cpu_limit || has_mem_limit) {
        task.limits = Some(TaskLimits::default());
    }

    if let Some(ref mut task_limits) = task.limits {
        if task_limits.cpus.is_none() && has_cpu_limit {
            task_limits.cpus = Some(limits.default_cpus_limit.clone());
        }
        if task_limits.memory.is_none() && has_mem_limit {
            task_limits.memory = Some(limits.default_memory_limit.clone());
        }
    }

    if task.timeout.is_none() && !limits.default_timeout.is_empty() {
        task.timeout = Some(limits.default_timeout.clone());
    }
}

/// Parse a duration string (e.g., "5m", "1h", "30s")
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let value_str: String = s.chars().take_while(char::is_ascii_digit).collect();
    let unit = s[value_str.len()..].trim();

    let value: u64 = value_str.parse().ok()?;
    if value == 0 {
        return None;
    }

    match unit {
        "s" | "sec" | "second" | "seconds" => Some(Duration::from_secs(value)),
        "m" | "min" | "minute" | "minutes" => Some(Duration::from_secs(value * 60)),
        "h" | "hour" | "hours" => Some(Duration::from_secs(value * 3600)),
        "d" | "day" | "days" => Some(Duration::from_secs(value * 86400)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5s"), Some(Duration::from_secs(5)));
        assert_eq!(parse_duration("10m"), Some(Duration::from_secs(600)));
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("1d"), Some(Duration::from_secs(86400)));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration("0s"), None);
    }

    #[test]
    fn test_apply_limits_empty_task() {
        let mut task = Task::default();
        let limits = Limits {
            default_cpus_limit: "2".to_string(),
            default_memory_limit: "1g".to_string(),
            default_timeout: "10m".to_string(),
        };

        apply_limits(&mut task, &limits);

        assert!(task.limits.is_some());
        let task_limits = task.limits.unwrap();
        assert_eq!(task_limits.cpus, Some("2".to_string()));
        assert_eq!(task_limits.memory, Some("1g".to_string()));
        assert_eq!(task.timeout, Some("10m".to_string()));
    }

    #[test]
    fn test_apply_limits_partial_task() {
        let task = Task {
            limits: Some(TaskLimits {
                cpus: Some("4".to_string()),
                memory: None,
            }),
            ..Default::default()
        };
        let limits = Limits {
            default_cpus_limit: "2".to_string(),
            default_memory_limit: "1g".to_string(),
            default_timeout: "10m".to_string(),
        };

        let mut modified_task = task.clone();
        apply_limits(&mut modified_task, &limits);

        // CPU should remain as set
        let task_limits = modified_task.limits.as_ref().unwrap();
        assert_eq!(task_limits.cpus, Some("4".to_string()));
        // Memory should get default
        assert_eq!(task_limits.memory, Some("1g".to_string()));
        // Timeout should get default
        assert_eq!(modified_task.timeout, Some("10m".to_string()));
    }
}
