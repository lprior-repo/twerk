//! Worker implementation module.
//!
//! Provides the core Worker struct that handles:
//! - Task execution via runtime
//! - Queue subscription via broker
//! - Heartbeat management

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use dashmap::DashMap;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use twerk_core::id::{NodeId, TaskId};
use twerk_core::node::{Node, NodeStatus};
use twerk_core::task::{Task, TaskLimits, TASK_STATE_COMPLETED, TASK_STATE_FAILED, TASK_STATE_RUNNING};
use twerk_core::uuid::new_short_uuid;

use crate::broker::is_worker_queue;
use crate::broker::{Broker, TaskHandler};
use crate::runtime::Runtime as RuntimeTrait;
use crate::worker::api::WorkerApi;

/// Worker configuration
#[derive(Clone)]
pub struct Config {
    /// Worker name
    pub name: String,
    /// API address (empty for dynamic)
    pub address: String,
    /// Broker for task queue
    pub broker: Arc<dyn Broker>,
    /// Runtime for task execution
    pub runtime: Arc<dyn RuntimeTrait>,
    /// Queue subscriptions (queue name -> concurrency)
    pub queues: HashMap<String, i32>,
    /// Default resource limits
    pub limits: Limits,
}

/// Default resource limits for tasks
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Default CPU limit (e.g., "1", "2")
    pub default_cpus_limit: String,
    /// Default memory limit (e.g., "512m", "1g")
    pub default_memory_limit: String,
    /// Default timeout duration (e.g., "5m", "1h")
    pub default_timeout: String,
}



/// Errors that can occur during worker operations
#[derive(Debug, Error)]
pub enum NewWorkerError {
    #[error("no queues configured")]
    NoQueuesConfigured,

    #[error("broker is required")]
    BrokerRequired,

    #[error("runtime is required")]
    RuntimeRequired,
}

/// Running task tracking
#[derive(Debug, Clone)]
struct RunningTask {
    cancel_tx: broadcast::Sender<()>,
}

/// Worker handles task execution and queue subscription
pub struct Worker {
    /// Unique worker ID
    id: String,
    /// Worker name
    name: String,
    /// Runtime for task execution
    runtime: Arc<dyn RuntimeTrait>,
    /// Broker for task queue
    broker: Arc<dyn Broker>,
    /// Shutdown signal sender
    stop_tx: broadcast::Sender<()>,
    /// Queue subscriptions
    queues: HashMap<String, i32>,
    /// Active tasks
    active_tasks: Arc<DashMap<TaskId, RunningTask>>,
    /// Default resource limits
    limits: Limits,
    /// Worker API
    api: WorkerApi,
}

impl Worker {
    /// Create a new Worker from configuration
    pub fn new(config: Config) -> Result<Self, NewWorkerError> {
        // Validate configuration
        if config.queues.is_empty() {
            return Err(NewWorkerError::NoQueuesConfigured);
        }

        // Create API
        let api = WorkerApi::new(
            config.broker.clone(),
            Arc::new(crate::datastore::inmemory::InMemoryDatastore::default()),
            config.runtime.clone(),
        );

        let (stop_tx, _) = broadcast::channel(1);

        Ok(Self {
            id: new_short_uuid(),
            name: config.name,
            runtime: config.runtime,
            broker: config.broker,
            stop_tx,
            queues: config.queues,
            active_tasks: Arc::new(DashMap::new()),
            limits: config.limits,
            api,
        })
    }

    /// Get the worker ID
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the worker name
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the worker API port
    #[must_use]
    pub fn port(&self) -> u16 {
        self.api.port()
    }

    /// Start the worker
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting worker {} ({})", self.name, self.id);

        // Start the API server
        self.api.start().await?;

        // Subscribe to exclusive cancel queue
        let cancel_queue = format!("cancel.{}", self.id);
        let cancel_runtime = self.runtime.clone();
        let cancel_active_tasks = self.active_tasks.clone();
        let cancel_broker = self.broker.clone();

        let _cancel_queue = cancel_queue.clone();
        tokio::spawn(async move {
            let handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                let runtime = cancel_runtime.clone();
                let active_tasks = cancel_active_tasks.clone();
                Box::pin(async move {
                    if let Some(tid) = &task.id {
                        if let Some((_, running)) = active_tasks.remove(tid) {
                            debug!("Cancelling task {}", tid);
                            let _ = running.cancel_tx.send(());
                        }
                    }
                    let _ = runtime.health_check().await.map_err(|e| {
                        warn!("Runtime health check failed during cancel: {}", e)
                    });
                    Ok(())
                })
            });

            let _ = cancel_broker.subscribe_for_tasks(cancel_queue, handler).await;
        });

        // Subscribe to task queues
        for (qname, concurrency) in &self.queues {
            if !is_worker_queue(qname) {
                continue;
            }

            for _ in 0..*concurrency {
                let broker = self.broker.clone();
                let runtime = self.runtime.clone();
                let limits = self.limits.clone();
                let active_tasks = self.active_tasks.clone();
                let qname_clone = qname.clone();

                let handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let runtime = runtime.clone();
                    let broker = broker.clone();
                    let limits = limits.clone();
                    let active_tasks = active_tasks.clone();
                    Box::pin(async move {
                        execute_task(task, runtime, broker, limits, active_tasks).await
                    })
                });

                // Subscribe and run - the subscription is blocking
                let broker_for_subscribe = self.broker.clone();
                tokio::spawn(async move {
                    let _ = broker_for_subscribe.subscribe_for_tasks(qname_clone, handler).await;
                });
            }
        }

        // Start heartbeat sender
        let broker = self.broker.clone();
        let runtime = self.runtime.clone();
        let id = self.id.clone();
        let name = self.name.clone();
        let port = self.api.port();
        let mut stop_rx = self.stop_tx.subscribe();

        tokio::spawn(async move {
            send_heartbeats(broker, runtime, id, name, port, &mut stop_rx).await;
        });

        Ok(())
    }

    /// Stop the worker gracefully
    pub async fn stop(&self) -> Result<()> {
        debug!("Shutting down worker {}", self.id);

        // Send stop signal
        let _ = self.stop_tx.send(());

        // Wait for active tasks to complete (with timeout)
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(15);

        while !self.active_tasks.is_empty() && start.elapsed() < timeout {
            sleep(Duration::from_millis(100)).await;
        }

        if !self.active_tasks.is_empty() {
            warn!(
                "Worker stopping with {} tasks still active",
                self.active_tasks.len()
            );
        }

        // Shutdown broker
        self.broker.shutdown().await?;

        Ok(())
    }
}

/// Execute a task
async fn execute_task(
    task: Arc<Task>,
    runtime: Arc<dyn RuntimeTrait>,
    broker: Arc<dyn Broker>,
    limits: Limits,
    active_tasks: Arc<DashMap<TaskId, RunningTask>>,
) -> Result<()> {
    let mut t = (*task).clone();
    let tid = match &t.id {
        Some(id) => id.clone(),
        None => return Ok(()),
    };

    // Create cancellation channel
    let (cancel_tx, mut cancel_rx) = broadcast::channel(1);

    // Track running task
    let running = RunningTask { cancel_tx };
    active_tasks.insert(tid.clone(), running);

    // Apply default limits
    apply_limits(&mut t, &limits);

    // Update task state
    t.state = TASK_STATE_RUNNING.to_string();
    t.started_at = Some(OffsetDateTime::now_utc());

    // Publish task started
    let _ = broker.publish_task_progress(&t).await;

    // Run the task with cancellation support
    let result = run_task_with_cancel(&t, runtime.clone(), &mut cancel_rx).await;

    // Update final state
    match result {
        Ok(()) => {
            t.state = TASK_STATE_COMPLETED.to_string();
            t.completed_at = Some(OffsetDateTime::now_utc());
        }
        Err(e) => {
            t.state = TASK_STATE_FAILED.to_string();
            t.failed_at = Some(OffsetDateTime::now_utc());
            t.error = Some(e.to_string());
        }
    }

    // Remove from active tasks
    active_tasks.remove(&tid);

    // Publish final state
    let _ = broker.publish_task_progress(&t).await;

    Ok(())
}

/// Run a task with support for cancellation
async fn run_task_with_cancel(
    t: &Task,
    runtime: Arc<dyn RuntimeTrait>,
    cancel_rx: &mut broadcast::Receiver<()>,
) -> Result<()> {
    // Create timeout context if needed
    let timeout = t.timeout.clone();

    if let Some(timeout_str) = timeout {
        if let Some(dur) = parse_duration(&timeout_str) {
            return tokio::select! {
                result = runtime.run(t) => result,
                _ = cancel_rx.recv() => {
                    debug!("Task {} cancelled", t.id.as_deref().unwrap_or("unknown"));
                    Ok(())
                },
                _ = sleep(dur) => {
                    warn!("Task {} timed out after {}", t.id.as_deref().unwrap_or("unknown"), timeout_str);
                    let _ = runtime.stop(t).await;
                    Err(anyhow::anyhow!("task timed out"))
                }
            };
        }
    }

    // No timeout
    tokio::select! {
        result = runtime.run(t) => result,
        _ = cancel_rx.recv() => {
            debug!("Task {} cancelled", t.id.as_deref().unwrap_or("unknown"));
            Ok(())
        }
    }
}

/// Apply default limits to a task
fn apply_limits(task: &mut Task, limits: &Limits) {
    if task.limits.is_none()
        && (!limits.default_cpus_limit.is_empty() || !limits.default_memory_limit.is_empty())
    {
        task.limits = Some(TaskLimits::default());
    }

    if let Some(ref mut task_limits) = task.limits {
        if task_limits.cpus.is_none() && !limits.default_cpus_limit.is_empty() {
            task_limits.cpus = Some(limits.default_cpus_limit.clone());
        }
        if task_limits.memory.is_none() && !limits.default_memory_limit.is_empty() {
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

    let value_str: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
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

/// Send heartbeats to the broker
async fn send_heartbeats(
    broker: Arc<dyn Broker>,
    runtime: Arc<dyn RuntimeTrait>,
    id: String,
    name: String,
    port: u16,
    stop_rx: &mut broadcast::Receiver<()>,
) {
    let heartbeat_interval = Duration::from_secs(30);

    loop {
        tokio::select! {
            _ = stop_rx.recv() => break,
            _ = sleep(heartbeat_interval) => {}
        }

        // Check runtime health
        let status = match runtime.health_check().await {
            Ok(()) => NodeStatus::UP,
            Err(e) => {
                warn!("Runtime health check failed: {}", e);
                NodeStatus::DOWN
            }
        };

        // Get hostname
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".to_string());

        // Get CPU usage
        let cpu_percent = twerk_core::host::get_cpu_percent().into();

        // Create node for heartbeat
        let node = Node {
            id: Some(NodeId::from(id.clone())),
            name: Some(name.clone()),
            hostname: Some(hostname),
            cpu_percent,
            status: Some(status),
            port: Some(port as i64),
            last_heartbeat_at: Some(OffsetDateTime::now_utc()),
            ..Default::default()
        };

        if let Err(e) = broker.publish_heartbeat(node).await {
            error!("Failed to publish heartbeat: {}", e);
        }
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
        let mut task = Task::default();
        task.limits = Some(TaskLimits {
            cpus: Some("4".to_string()),
            memory: None,
        });
        let limits = Limits {
            default_cpus_limit: "2".to_string(),
            default_memory_limit: "1g".to_string(),
            default_timeout: "10m".to_string(),
        };

        apply_limits(&mut task, &limits);

        // CPU should remain as set
        let task_limits = task.limits.as_ref().unwrap();
        assert_eq!(task_limits.cpus, Some("4".to_string()));
        // Memory should get default
        assert_eq!(task_limits.memory, Some("1g".to_string()));
        // Timeout should get default
        assert_eq!(task.timeout, Some("10m".to_string()));
    }
}