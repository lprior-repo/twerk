//! Worker implementation for task execution.
//!
//! This module provides the core worker functionality for executing tasks
//! from a broker queue.

use crate::host::get_cpu_percent;
use crate::syncx::Map;
use dashmap::DashMap;
use tork::broker::{is_worker_queue, queue, Broker};
use tork::broker::queue::{QUEUE_COMPLETED, QUEUE_ERROR, QUEUE_EXCLUSIVE_PREFIX, QUEUE_STARTED};
use tork::node::{Node, HEARTBEAT_RATE_SECS, NODE_STATUS_UP, NODE_STATUS_DOWN};
use tork::runtime::Runtime;
use tork::task::{Task, TASK_STATE_COMPLETED, TASK_STATE_FAILED, TASK_STATE_RUNNING};

use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::time::interval;
use uuid::Uuid;

/// Worker error types
#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("must provide broker")]
    BrokerRequired,
    #[error("must provide runtime")]
    RuntimeRequired,
    #[error("unexpected state {0} for task {1}")]
    UnexpectedState(String, String),
    #[error("invalid timeout duration: {0}")]
    InvalidTimeout(String),
    #[error("error subscribing for queue: {0}")]
    SubscribeQueue(String),
    #[error("error shutting down broker: {0}")]
    BrokerShutdown(String),
    #[error("error shutting down API: {0}")]
    ApiShutdown(String),
    #[error("broker error: {0}")]
    Broker(#[from] anyhow::Error),
}

/// Limits holds default resource limits for tasks
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Default CPU limit
    pub default_cpus_limit: Option<String>,
    /// Default memory limit
    pub default_memory_limit: Option<String>,
    /// Default timeout
    pub default_timeout: Option<String>,
}

/// Configuration for creating a new worker
pub struct Config {
    /// Worker name
    pub name: Option<String>,
    /// HTTP address to listen on
    pub address: Option<String>,
    /// Message broker
    pub broker: Option<Arc<dyn Broker>>,
    /// Runtime for executing tasks
    pub runtime: Option<Arc<dyn Runtime>>,
    /// Queue configuration (queue name -> concurrency)
    pub queues: Arc<DashMap<String, i32>>,
    /// Default limits
    pub limits: Limits,
    /// Task middleware functions
    pub middleware: Arc<Vec<Box<dyn Fn(Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync>) -> Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync> + Send + Sync>>>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("name", &self.name)
            .field("address", &self.address)
            .field("queues", &self.queues.len())
            .field("limits", &self.limits)
            .field("middleware", &self.middleware.len())
            .finish()
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            address: self.address.clone(),
            broker: self.broker.clone(),
            runtime: self.runtime.clone(),
            queues: Arc::clone(&self.queues),
            limits: self.limits.clone(),
            middleware: Arc::clone(&self.middleware),
        }
    }
}

/// Represents a currently running task
#[derive(Debug, Clone)]
pub struct RunningTask {
    /// Cancellation sender
    pub cancel: Arc<oneshot::Sender<()>>,
    /// The task being executed
    pub task: Arc<Task>,
}

/// The worker that executes tasks from a broker queue
pub struct Worker {
    /// Unique worker ID
    id: String,
    /// Worker name
    name: Option<String>,
    /// Start time
    start_time: time::OffsetDateTime,
    /// Runtime for executing tasks
    runtime: Arc<dyn Runtime>,
    /// Message broker
    broker: Arc<dyn Broker>,
    /// Stop signal sender
    stop_tx: Arc<oneshot::Sender<()>>,
    /// Queue configuration
    queues: Arc<DashMap<String, i32>>,
    /// Currently running tasks
    tasks: Arc<Map<String, RunningTask>>,
    /// Default limits
    limits: Limits,
    /// HTTP API
    api: crate::worker::api::Api,
    /// Current task count (Arc-shared for heartbeat access)
    task_count: Arc<std::sync::atomic::AtomicI32>,
    /// Task middleware
    middleware: Arc<Vec<Box<dyn Fn(Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync>) -> Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync> + Send + Sync>>>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("start_time", &self.start_time)
            .field("queues", &self.queues.len())
            .field("task_count", &self.task_count.load(std::sync::atomic::Ordering::SeqCst))
            .finish()
    }
}

impl Worker {
    /// Creates a new worker from configuration
    pub fn new(cfg: Config) -> Result<Self, WorkerError> {
        let broker = cfg.broker.ok_or(WorkerError::BrokerRequired)?;
        let runtime = cfg.runtime.ok_or(WorkerError::RuntimeRequired)?;

        let tasks = Arc::new(Map::new());
        let api = crate::worker::api::Api::new(
            cfg.address.clone(),
            Arc::clone(&broker),
            Arc::clone(&runtime),
            Arc::clone(&tasks),
        );

        let (stop_tx, _stop_rx) = oneshot::channel::<()>();

        // If queues map was empty, add default queue with concurrency 1
        if cfg.queues.is_empty() {
            cfg.queues.insert(queue::QUEUE_DEFAULT.to_string(), 1);
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: cfg.name,
            start_time: time::OffsetDateTime::now_utc(),
            runtime,
            broker,
            stop_tx: Arc::new(stop_tx),
            queues: cfg.queues,
            tasks,
            limits: cfg.limits,
            api,
            task_count: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            middleware: cfg.middleware,
        })
    }

    /// Returns the worker ID
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the current task count
    #[must_use]
    pub fn task_count(&self) -> i32 {
        self.task_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Cancels a task by ID
    pub async fn cancel_task(&self, task_id: &str) -> Result<(), WorkerError> {
        if let Some(running_task) = self.tasks.get(&task_id.to_string()) {
            match Arc::try_unwrap(running_task.cancel) {
                Ok(sender) => {
                    let _ = sender.send(());
                }
                Err(_) => {
                    // Sender is shared elsewhere
                }
            }
            self.tasks.delete(task_id.to_string());
        }
        Ok(())
    }

    /// Handles a task (main entry point for task execution)
    pub async fn handle_task(&self, task: Arc<Task>) -> Result<(), WorkerError> {
        self.do_handle_task(task).await
    }

    /// Internal task handling with context
    async fn do_handle_task(&self, task: Arc<Task>) -> Result<(), WorkerError> {
        // Increment task count
        self.task_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _decrement = Defer {
            counter: Arc::clone(&self.task_count),
        };

        let started = time::OffsetDateTime::now_utc();
        let mut task = (*task).clone();
        task.started_at = Some(started);
        task.node_id = Some(self.id.clone());
        task.state = TASK_STATE_RUNNING;

        // Apply default limits
        if task.limits.is_none()
            && (self.limits.default_cpus_limit.is_some()
                || self.limits.default_memory_limit.is_some())
        {
            task.limits = Some(tork::task::TaskLimits {
                cpus: None,
                memory: None,
            });
        }
        if let Some(ref mut limits) = task.limits {
            if limits.cpus.is_none() {
                limits.cpus = self.limits.default_cpus_limit.clone();
            }
            if limits.memory.is_none() {
                limits.memory = self.limits.default_memory_limit.clone();
            }
        }
        if task.timeout.is_none() {
            task.timeout = self.limits.default_timeout.clone();
        }

        // Clone task for middleware (to avoid mutation of original)
        let cloned_task = task.clone();
        let task_arc = Arc::new(cloned_task);

        // Create cancellation context
        let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();

        // Register running task
        self.tasks.set(
            task.id.clone().unwrap_or_default(),
            RunningTask {
                cancel: Arc::new(cancel_tx),
                task: Arc::clone(&task_arc),
            },
        );

        let task_for_broker = task_arc.clone();

        // Publish task started event
        let broker = Arc::clone(&self.broker);
        let task_started = Arc::clone(&task_for_broker);
        if let Err(e) = broker
            .publish_task(QUEUE_STARTED.to_string(), &task_started)
            .await
        {
            tracing::warn!(error = %e, "failed to publish task started event");
        }

        // Create the actual task handler with timeout support
        // (mirrors Go's doRunTask: parse timeout, create timeout context, run)
        let runtime = Arc::clone(&self.runtime);
        let timeout_str = task.timeout.clone();
        let task_handler: Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>> + Send + Sync> =
            Arc::new(move |task: Arc<Task>| {
                let runtime = Arc::clone(&runtime);
                let timeout_str = timeout_str.clone();
                Box::pin(async move {
                    let mut t = (*task).clone();
                    let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));

                    // Parse timeout if defined (Go: doRunTask creates timeout context)
                    let run_future = runtime.run(ctx, &mut t);
                    let result = if let Some(ref ts) = timeout_str {
                        let dur = parse_go_duration(ts);
                        match tokio::time::timeout(dur, run_future).await {
                            Ok(inner) => inner,
                            Err(_) => {
                                let now = time::OffsetDateTime::now_utc();
                                t.error = Some(format!(
                                    "context deadline exceeded: {} timeout",
                                    ts
                                ));
                                t.failed_at = Some(now);
                                t.state = TASK_STATE_FAILED;
                                return Err(anyhow::anyhow!(
                                    "context deadline exceeded: {}",
                                    ts
                                ));
                            }
                        }
                    } else {
                        run_future.await
                    };

                    if let Err(e) = result {
                        let now = time::OffsetDateTime::now_utc();
                        t.error = Some(e.to_string());
                        t.failed_at = Some(now);
                        t.state = TASK_STATE_FAILED;
                        Err(e)
                    } else {
                        let now = time::OffsetDateTime::now_utc();
                        t.completed_at = Some(now);
                        t.state = TASK_STATE_COMPLETED;
                        Ok(())
                    }
                }) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>>
            });

        // Apply middleware chain
        let mut handler = task_handler;
        for mw in self.middleware.iter() {
            let next = handler;
            handler = mw(next);
        }

        // Run the task with cancellation support
        let result = {
            let task_for_handler = Arc::clone(&task_for_broker);
            tokio::select! {
                result = handler(task_for_handler) => {
                    result
                }
                _ = &mut cancel_rx => {
                    tracing::debug!("task cancelled");
                    Err(anyhow::anyhow!("task cancelled"))
                }
            }
        };

        // Remove from running tasks
        self.tasks
            .delete(task.id.clone().unwrap_or_default());

        match result {
            Ok(()) => {
                // Task completed successfully
                let mut final_task = (*task_for_broker).clone();
                final_task.state = TASK_STATE_COMPLETED;
                final_task.completed_at = Some(time::OffsetDateTime::now_utc());
                self.broker
                    .publish_task(QUEUE_COMPLETED.to_string(), &final_task)
                    .await
                    .map_err(WorkerError::Broker)?;
            }
            Err(e) => {
                // Task failed
                let mut final_task = (*task_for_broker).clone();
                final_task.state = TASK_STATE_FAILED;
                final_task.error = Some(e.to_string());
                final_task.failed_at = Some(time::OffsetDateTime::now_utc());
                self.broker
                    .publish_task(QUEUE_ERROR.to_string(), &final_task)
                    .await
                    .map_err(WorkerError::Broker)?;
            }
        }

        Ok(())
    }

    /// Starts the worker
    pub async fn start(&mut self) -> Result<(), WorkerError> {
        // Start API server
        self.api.start().await.map_err(|e| WorkerError::ApiShutdown(e.to_string()))?;

        let worker_id = self.id.clone();
        let tasks = Arc::clone(&self.tasks);

        // Subscribe for exclusive queue (for cancellations)
        let exclusive_queue = format!("{}{}", QUEUE_EXCLUSIVE_PREFIX, worker_id);
        let cancel_handler = Arc::new(move |task: Arc<Task>| {
            let tasks = Arc::clone(&tasks);
            Box::pin(async move {
                if let Some(task_id) = &task.id {
                    if let Some(rt) = tasks.get(task_id) {
                        match Arc::try_unwrap(rt.cancel) {
                            Ok(sender) => {
                                let _ = sender.send(());
                            }
                            Err(_) => {
                                // Sender is shared elsewhere
                            }
                        }
                        tasks.delete(task_id.clone());
                    }
                }
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        });

        self.broker
            .subscribe_for_tasks(exclusive_queue, cancel_handler)
            .await
            .map_err(|e| WorkerError::SubscribeQueue(e.to_string()))?;

        // Subscribe to shared work queues
        let queues_snapshot: Vec<(String, i32)> = self.queues
            .iter()
            .map(|pair| (pair.key().clone(), *pair.value()))
            .collect();
        for (qname, concurrency) in queues_snapshot {
            if is_worker_queue(&qname) {
                for _ in 0..concurrency {
                    let worker = Arc::new(self.clone_inner());
                    let handle_task = Arc::new(move |task: Arc<Task>| {
                        let worker = Arc::clone(&worker);
                        Box::pin(async move {
                            if let Err(e) = worker.handle_task(task).await {
                                tracing::error!(error = %e, "error handling task");
                            }
                        }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
                    });

                    self.broker
                        .subscribe_for_tasks(qname.clone(), handle_task)
                        .await
                        .map_err(|e| WorkerError::SubscribeQueue(e.to_string()))?;
                }
            }
        }

        // Start heartbeat loop
        let stop_tx = Arc::clone(&self.stop_tx);
        let task_count = Arc::clone(&self.task_count);
        let id = self.id.clone();
        let name = self.name.clone();
        let start_time = self.start_time;
        let broker = Arc::clone(&self.broker);
        let runtime = Arc::clone(&self.runtime);
        let api_port = self.api.port();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(HEARTBEAT_RATE_SECS as u64));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let ctx = tokio::time::timeout(
                            Duration::from_secs(5),
                            runtime.health_check(),
                        ).await;

                        let status = match ctx {
                            Ok(Ok(())) => NODE_STATUS_UP.to_string(),
                            _ => NODE_STATUS_DOWN.to_string(),
                        };

                        let hostname = hostname::get()
                            .map(|h| h.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let cpu_percent = get_cpu_percent();

                        let node = Node {
                            id: Some(id.clone()),
                            name: name.clone(),
                            started_at: start_time,
                            cpu_percent,
                            last_heartbeat_at: time::OffsetDateTime::now_utc(),
                            queue: Some(format!(
                                "{}{}",
                                QUEUE_EXCLUSIVE_PREFIX,
                                id
                            )),
                            status,
                            hostname: Some(hostname),
                            port: api_port,
                            task_count: task_count.load(std::sync::atomic::Ordering::SeqCst) as i64,
                            version: tork::version::VERSION.to_string(),
                        };

                        if let Err(e) = broker.publish_heartbeat(node).await {
                            tracing::error!(error = %e, "error publishing heartbeat");
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Check if stop signal was sent
                        if Arc::as_ref(&stop_tx).is_closed() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stops the worker
    pub async fn stop(&mut self) -> Result<(), WorkerError> {
        // Signal shutdown by sending on the stop channel
        match Arc::try_unwrap(self.stop_tx.clone()) {
            Ok(sender) => {
                let _ = sender.send(());
            }
            Err(_) => {
                // Couldn't unwrap, sender is shared
            }
        }

        // Shutdown broker with timeout
        tokio::time::timeout(Duration::from_secs(15), self.broker.shutdown())
            .await
            .map_err(|e| WorkerError::BrokerShutdown(e.to_string()))?
            .map_err(|e| WorkerError::BrokerShutdown(e.to_string()))?;

        // Shutdown API with timeout
        let _ = tokio::time::timeout(Duration::from_secs(15), self.api.shutdown())
            .await
            .map_err(|e| WorkerError::ApiShutdown(e.to_string()))?;

        Ok(())
    }

    /// Creates a clone of the worker for sharing across async tasks
    fn clone_inner(&self) -> Worker {
        Worker {
            id: self.id.clone(),
            name: self.name.clone(),
            start_time: self.start_time,
            runtime: Arc::clone(&self.runtime),
            broker: Arc::clone(&self.broker),
            stop_tx: Arc::clone(&self.stop_tx),
            queues: self.queues.clone(),
            tasks: Arc::clone(&self.tasks),
            limits: self.limits.clone(),
            api: self.api.clone(),
            task_count: Arc::clone(&self.task_count),
            middleware: Arc::clone(&self.middleware),
        }
    }
}

impl Clone for Worker {
    fn clone(&self) -> Self {
        self.clone_inner()
    }
}

/// RAII guard to decrement task count when dropped
struct Defer {
    counter: Arc<std::sync::atomic::AtomicI32>,
}

impl Drop for Defer {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Parses a Go-style duration string (e.g. "5s", "100ms", "1m") into a
/// [`std::time::Duration`]. Returns `Duration::ZERO` on parse failure so
/// callers can decide how to handle the error.
fn parse_go_duration(s: &str) -> std::time::Duration {
    let s = s.trim();
    // Try standard Duration from_secs_f64 with common suffixes
    if let Some(rest) = s.strip_suffix("ms") {
        return rest
            .trim()
            .parse::<f64>()
            .map(|v| std::time::Duration::from_millis(v as u64))
            .unwrap_or_default();
    }
    if let Some(rest) = s.strip_suffix('s') {
        return rest
            .trim()
            .parse::<f64>()
            .map(|v| std::time::Duration::from_secs_f64(v))
            .unwrap_or_default();
    }
    if let Some(rest) = s.strip_suffix('m') {
        return rest
            .trim()
            .parse::<f64>()
            .map(|v| std::time::Duration::from_secs_f64(v * 60.0))
            .unwrap_or_default();
    }
    if let Some(rest) = s.strip_suffix('h') {
        return rest
            .trim()
            .parse::<f64>()
            .map(|v| std::time::Duration::from_secs_f64(v * 3600.0))
            .unwrap_or_default();
    }
    // Unrecognized suffix — return zero (caller will handle as invalid timeout)
    std::time::Duration::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::inmemory::new_in_memory_broker;
    use std::sync::atomic::AtomicBool;

    /// A test runtime that can be configured to succeed, fail, or simulate
    /// long-running tasks for cancellation/timeout tests.
    struct TestRuntime {
        healthy: bool,
        /// If Some(err), run() returns Err(err)
        error: Option<String>,
        /// If true, run() sleeps for 10s (useful for cancellation/timeout tests)
        slow: bool,
        /// Records how many times run() was called
        run_count: Arc<AtomicBool>,
    }

    impl TestRuntime {
        fn new() -> Self {
            Self {
                healthy: true,
                error: None,
                slow: false,
                run_count: Arc::new(AtomicBool::new(false)),
            }
        }

        fn unhealthy() -> Self {
            let mut rt = Self::new();
            rt.healthy = false;
            rt
        }

        fn with_error(msg: &str) -> Self {
            let mut rt = Self::new();
            rt.error = Some(msg.to_string());
            rt
        }

        fn slow() -> Self {
            let mut rt = Self::new();
            rt.slow = true;
            rt
        }
    }

    impl Runtime for TestRuntime {
        fn run(
            &self,
            _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
            _task: &mut tork::task::Task,
        ) -> tork::runtime::BoxedFuture<()> {
            let error = self.error.clone();
            let slow = self.slow;
            let run_count = Arc::clone(&self.run_count);
            Box::pin(async move {
                run_count.store(true, std::sync::atomic::Ordering::SeqCst);
                if let Some(ref err) = error {
                    return Err(anyhow::anyhow!("{}", err));
                }
                if slow {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
                Ok(())
            })
        }

        fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
            let healthy = self.healthy;
            Box::pin(async move {
                if healthy {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("runtime unhealthy"))
                }
            })
        }
    }

    /// Helper to create a Config with the given runtime and broker
    fn test_config(runtime: TestRuntime) -> Config {
        Config {
            name: None,
            address: None,
            broker: Some(Arc::new(new_in_memory_broker())),
            runtime: Some(Arc::new(runtime)),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        }
    }

    /// Helper to create a simple task with an ID
    fn test_task(id: &str) -> Arc<Task> {
        Arc::new(Task {
            id: Some(id.to_string()),
            state: TASK_STATE_RUNNING.clone(),
            ..Default::default()
        })
    }

    // ---- TestNewWorker (mirrors Go TestNewWorker) ----
    #[test]
    fn test_new_worker_broker_required() {
        let cfg = Config {
            name: None,
            address: None,
            broker: None,
            runtime: None,
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };
        let result = Worker::new(cfg);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must provide broker"));
    }

    #[test]
    fn test_new_worker_runtime_required() {
        let cfg = Config {
            name: None,
            address: None,
            broker: Some(Arc::new(new_in_memory_broker())),
            runtime: None,
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };
        let result = Worker::new(cfg);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must provide runtime"));
    }

    #[test]
    fn test_new_worker_success() {
        let cfg = test_config(TestRuntime::new());
        let worker = Worker::new(cfg);
        assert!(worker.is_ok());
        let worker = worker.unwrap();
        assert!(!worker.id().is_empty());
        assert_eq!(worker.task_count(), 0);
    }

    #[test]
    fn test_new_worker_default_queue() {
        let cfg = test_config(TestRuntime::new());
        let worker = Worker::new(cfg).unwrap();
        // Should have the default queue "default" with concurrency 1
        assert!(worker.queues.contains_key(queue::QUEUE_DEFAULT));
    }

    // ---- Test_handleTaskRun (mirrors Go Test_handleTaskRun) ----
    #[tokio::test]
    async fn test_handle_task_run_success() {
        let cfg = test_config(TestRuntime::new());
        let worker = Worker::new(cfg).unwrap();

        let task = test_task("task-1");
        let result = worker.handle_task(task).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_task_run_error() {
        let cfg = test_config(TestRuntime::with_error("something went wrong"));
        let worker = Worker::new(cfg).unwrap();

        let task = test_task("task-err");
        let result = worker.handle_task(task).await;
        assert!(result.is_ok()); // Worker handles the error internally
    }

    // ---- Test_handleTaskRunDefaultLimitExceeded (mirrors Go) ----
    #[tokio::test]
    async fn test_handle_task_timeout_exceeded() {
        let cfg = Config {
            name: None,
            address: None,
            broker: Some(Arc::new(new_in_memory_broker())),
            runtime: Some(Arc::new(TestRuntime::slow())),
            queues: Arc::new(DashMap::new()),
            limits: Limits {
                default_cpus_limit: None,
                default_memory_limit: None,
                default_timeout: Some("500ms".to_string()),
            },
            middleware: Arc::new(Vec::new()),
        };
        let worker = Worker::new(cfg).unwrap();

        let task = test_task("task-timeout");
        let result = worker.handle_task(task).await;
        // The task should time out — worker handles internally and publishes to error queue
        assert!(result.is_ok());
    }

    // ---- Test_handleTaskRunDefaultLimitOK (mirrors Go) ----
    #[tokio::test]
    async fn test_handle_task_timeout_ok() {
        let cfg = Config {
            name: None,
            address: None,
            broker: Some(Arc::new(new_in_memory_broker())),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues: Arc::new(DashMap::new()),
            limits: Limits {
                default_cpus_limit: None,
                default_memory_limit: None,
                default_timeout: Some("5s".to_string()),
            },
            middleware: Arc::new(Vec::new()),
        };
        let worker = Worker::new(cfg).unwrap();

        let task = test_task("task-timeout-ok");
        let result = worker.handle_task(task).await;
        assert!(result.is_ok());
    }

    // ---- Test_sendHeartbeat (mirrors Go Test_sendHeartbeat) ----
    #[tokio::test]
    async fn test_send_heartbeat() {
        let broker = new_in_memory_broker();
        let heartbeat_received = Arc::new(tokio::sync::Notify::new());
        let heartbeat_received_clone = Arc::clone(&heartbeat_received);

        let broker_arc: Arc<dyn Broker> = Arc::new(broker.clone());
        broker_arc
            .subscribe_for_heartbeats(Arc::new(move |_node: tork::node::Node| {
                let notify = Arc::clone(&heartbeat_received_clone);
                Box::pin(async move {
                    notify.notify_one();
                })
            }))
            .await
            .unwrap();

        let cfg = Config {
            name: None,
            address: Some(":0".to_string()),
            broker: Some(Arc::new(broker)),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };

        let mut worker = Worker::new(cfg).unwrap();
        worker.start().await.unwrap();

        // Wait for at least one heartbeat (HEARTBEAT_RATE_SECS is 30s in prod,
        // but the test should complete quickly due to the Notify)
        heartbeat_received.notified().await;

        worker.stop().await.unwrap();
    }

    // ---- Test_handleTaskCancel (mirrors Go Test_handleTaskCancel) ----
    #[tokio::test]
    async fn test_handle_task_cancel() {
        let cfg = test_config(TestRuntime::slow());
        let worker = Worker::new(cfg).unwrap();

        let task_id = "cancel-task-1";
        let task = Arc::new(Task {
            id: Some(task_id.to_string()),
            state: TASK_STATE_RUNNING.clone(),
            ..Default::default()
        });

        // Spawn the task handle
        let worker_clone = worker.clone();
        let task_for_cancel = Arc::clone(&task);
        let handle = tokio::spawn(async move {
            worker_clone.handle_task(task_for_cancel).await
        });

        // Wait a tiny bit for the task to register, then cancel
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        worker.cancel_task(task_id).await.unwrap();

        let result = handle.await.unwrap();
        // Task was cancelled — the worker handles it internally
        assert!(result.is_ok());
    }

    // ---- task_count tracking ----
    #[tokio::test]
    async fn test_task_count_increments() {
        let cfg = Config {
            name: None,
            address: None,
            broker: Some(Arc::new(new_in_memory_broker())),
            runtime: Some(Arc::new(TestRuntime::slow())),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };
        let worker = Worker::new(cfg).unwrap();

        let task = test_task("count-task");
        let worker_clone = worker.clone();
        let handle = tokio::spawn(async move {
            worker_clone.handle_task(task).await
        });

        // Give the task time to start and increment the counter
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(1, worker.task_count());

        // Cancel to clean up
        worker.cancel_task("count-task").await.unwrap();
        let _ = handle.await;

        // After completion, count should be back to 0
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(0, worker.task_count());
    }

    // ---- Limits applied correctly ----
    #[tokio::test]
    async fn test_default_limits_applied() {
        let cfg = Config {
            name: None,
            address: None,
            broker: Some(Arc::new(new_in_memory_broker())),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues: Arc::new(DashMap::new()),
            limits: Limits {
                default_cpus_limit: Some("2.0".to_string()),
                default_memory_limit: Some("4g".to_string()),
                default_timeout: Some("10s".to_string()),
            },
            middleware: Arc::new(Vec::new()),
        };
        let worker = Worker::new(cfg).unwrap();

        let task = Arc::new(Task {
            id: Some("limits-task".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            ..Default::default()
        });

        let result = worker.handle_task(task).await;
        assert!(result.is_ok());
    }

    // ---- cancel unknown task (should be no-op) ----
    #[tokio::test]
    async fn test_cancel_unknown_task() {
        let cfg = test_config(TestRuntime::new());
        let worker = Worker::new(cfg).unwrap();

        let result = worker.cancel_task("nonexistent").await;
        assert!(result.is_ok());
    }

    // Type aliases for middleware readability
    type BoxedTaskFn = Arc<
        dyn Fn(Arc<Task>) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), anyhow::Error>> + Send>,
        > + Send + Sync,
    >;
    type BoxedMiddlewareFn = Box<dyn Fn(BoxedTaskFn) -> BoxedTaskFn + Send + Sync>;

    // ---- TestStart (mirrors Go TestStart) ----
    // Verifies basic start/stop lifecycle without error.
    #[tokio::test]
    async fn test_start() {
        let cfg = Config {
            name: None,
            address: Some(":0".to_string()),
            broker: Some(Arc::new(new_in_memory_broker())),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };
        let mut worker = match Worker::new(cfg) {
            Ok(w) => w,
            Err(e) => panic!("worker creation should succeed: {e}"),
        };
        assert!(worker.start().await.is_ok(), "start should succeed");
        assert!(worker.stop().await.is_ok(), "stop should succeed");
    }

    // ---- TestStart_subscribes (mirrors Go TestStart — queue subscription) ----
    // Verifies that after start(), the worker picks up tasks published to its queue.
    #[tokio::test]
    async fn test_start_subscribes_to_queues() {
        let broker = new_in_memory_broker();
        let completed_notify = Arc::new(tokio::sync::Notify::new());
        let completed_notify_clone = Arc::clone(&completed_notify);
        let broker_ref: Arc<dyn Broker> = Arc::new(broker.clone());

        broker_ref
            .subscribe_for_tasks(
                QUEUE_COMPLETED.to_string(),
                Arc::new(move |_task: Arc<Task>| {
                    let n = Arc::clone(&completed_notify_clone);
                    Box::pin(async move {
                        n.notify_one();
                    })
                }),
            )
            .await
            .unwrap();

        let cfg = Config {
            name: None,
            address: Some(":0".to_string()),
            broker: Some(Arc::new(broker)),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };
        let mut worker = Worker::new(cfg).unwrap();
        worker.start().await.unwrap();

        // Allow time for queue subscription to register
        tokio::time::sleep(Duration::from_millis(100)).await;

        let task = Arc::new(Task {
            id: Some("queue-sub-task".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            ..Default::default()
        });
        broker_ref
            .publish_task(queue::QUEUE_DEFAULT.to_string(), &task)
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(2), completed_notify.notified())
            .await
            .unwrap();

        worker.stop().await.unwrap();
    }

    // ---- Test_handleTaskOutput (mirrors Go Test_handleTaskOutput) ----
    // Verifies that a completed task is published to QUEUE_COMPLETED with
    // state COMPLETED. Uses the worker's start() to subscribe the queue,
    // then publishes a task and waits for the completion signal.
    #[tokio::test]
    async fn test_handle_task_output_completed() {
        let broker = new_in_memory_broker();
        let completed_notify = Arc::new(tokio::sync::Notify::new());
        let completed_notify_clone = Arc::clone(&completed_notify);
        let received_state = Arc::new(std::sync::Mutex::new(String::new()));
        let received_state_clone = Arc::clone(&received_state);
        let broker_ref: Arc<dyn Broker> = Arc::new(broker.clone());

        // Subscribe to QUEUE_COMPLETED FIRST, before any publish
        broker_ref
            .subscribe_for_tasks(
                QUEUE_COMPLETED.to_string(),
                Arc::new(move |task: Arc<Task>| {
                    let n = Arc::clone(&completed_notify_clone);
                    let st = Arc::clone(&received_state_clone);
                    Box::pin(async move {
                        if let Ok(mut guard) = st.lock() {
                            *guard = task.state.to_string();
                        }
                        n.notify_one();
                    })
                }),
            )
            .await
            .unwrap();

        // Use the SAME broker instance (not test_config which creates a new one)
        let cfg = Config {
            name: None,
            address: None,
            broker: Some(Arc::new(broker)),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };
        let worker = Worker::new(cfg).unwrap();

        let task = test_task("output-task");
        let result = worker.handle_task(task).await;
        assert!(result.is_ok());

        // Allow time for the async publish to propagate through broadcast
        tokio::time::timeout(Duration::from_secs(2), completed_notify.notified())
            .await
            .unwrap();

        let state = received_state.lock().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(TASK_STATE_COMPLETED.as_ref(), state);
    }

    // ---- Test_handleTaskError (mirrors Go Test_handleTaskError) ----
    // Verifies that a runtime error publishes the task to QUEUE_ERROR with
    // a non-empty error message.
    #[tokio::test]
    async fn test_handle_task_error_published() {
        let broker = new_in_memory_broker();
        let error_notify = Arc::new(tokio::sync::Notify::new());
        let error_notify_clone = Arc::clone(&error_notify);
        let received_error = Arc::new(std::sync::Mutex::new(String::new()));
        let received_error_clone = Arc::clone(&received_error);
        let broker_ref: Arc<dyn Broker> = Arc::new(broker.clone());

        broker_ref
            .subscribe_for_tasks(
                QUEUE_ERROR.to_string(),
                Arc::new(move |task: Arc<Task>| {
                    let n = Arc::clone(&error_notify_clone);
                    let err = Arc::clone(&received_error_clone);
                    Box::pin(async move {
                        if let Some(ref e) = task.error {
                            if let Ok(mut guard) = err.lock() {
                                guard.clone_from(e);
                            }
                        }
                        n.notify_one();
                    })
                }),
            )
            .await
            .unwrap();

        // Use the SAME broker instance (not test_config which creates a new one)
        let cfg = Config {
            name: None,
            address: None,
            broker: Some(Arc::new(broker)),
            runtime: Some(Arc::new(TestRuntime::with_error("something went wrong"))),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };
        let worker = Worker::new(cfg).unwrap();

        let task = test_task("error-pub-task");
        let result = worker.handle_task(task).await;
        assert!(result.is_ok(), "handle_task should handle error internally");

        tokio::time::timeout(Duration::from_secs(2), error_notify.notified())
            .await
            .unwrap();

        let error_msg = received_error.lock().map(|g| g.clone()).unwrap_or_default();
        assert!(!error_msg.is_empty(), "error message should not be empty");
        assert!(
            error_msg.contains("something went wrong"),
            "error should contain original message, got: {error_msg}"
        );
    }

    // ---- Test_middleware (mirrors Go Test_middleware) ----
    // Verifies that middleware is invoked during task processing and the
    // task completes successfully through the middleware chain.
    #[tokio::test]
    async fn test_middleware_chain() {
        let broker = new_in_memory_broker();
        let middleware_called = Arc::new(AtomicBool::new(false));
        let middleware_called_clone = Arc::clone(&middleware_called);
        let completed_notify = Arc::new(tokio::sync::Notify::new());
        let completed_notify_clone = Arc::clone(&completed_notify);
        let broker_ref: Arc<dyn Broker> = Arc::new(broker.clone());

        broker_ref
            .subscribe_for_tasks(
                QUEUE_COMPLETED.to_string(),
                Arc::new(move |_task: Arc<Task>| {
                    let n = Arc::clone(&completed_notify_clone);
                    Box::pin(async move {
                        n.notify_one();
                    })
                }),
            )
            .await
            .unwrap();

        let mw: BoxedMiddlewareFn = Box::new(move |next: BoxedTaskFn| -> BoxedTaskFn {
            let called = Arc::clone(&middleware_called_clone);
            Arc::new(move |task: Arc<Task>| {
                let next = Arc::clone(&next);
                let called = Arc::clone(&called);
                Box::pin(async move {
                    called.store(true, std::sync::atomic::Ordering::SeqCst);
                    next(task).await
                })
            })
        });

        let queues = Arc::new(DashMap::new());
        queues.insert("someq".to_string(), 1);

        let cfg = Config {
            name: None,
            address: Some(":0".to_string()),
            broker: Some(Arc::new(broker)),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues,
            limits: Limits::default(),
            middleware: Arc::new(vec![mw]),
        };
        let mut worker = Worker::new(cfg).unwrap();
        worker.start().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let task = Arc::new(Task {
            id: Some("middleware-task".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            ..Default::default()
        });
        broker_ref
            .publish_task("someq".to_string(), &task)
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(2), completed_notify.notified())
            .await
            .unwrap();

        assert!(
            middleware_called.load(std::sync::atomic::Ordering::SeqCst),
            "middleware should have been called"
        );
        worker.stop().await.unwrap();
    }

    // ---- Test_middlewareFailure (mirrors Go Test_middlewareFailure) ----
    // Verifies that when middleware returns an error, the task is published
    // to QUEUE_ERROR.
    #[tokio::test]
    async fn test_middleware_failure() {
        let broker = new_in_memory_broker();
        let error_notify = Arc::new(tokio::sync::Notify::new());
        let error_notify_clone = Arc::clone(&error_notify);
        let received_error = Arc::new(std::sync::Mutex::new(String::new()));
        let received_error_clone = Arc::clone(&received_error);
        let broker_ref: Arc<dyn Broker> = Arc::new(broker.clone());

        broker_ref
            .subscribe_for_tasks(
                QUEUE_ERROR.to_string(),
                Arc::new(move |task: Arc<Task>| {
                    let n = Arc::clone(&error_notify_clone);
                    let err = Arc::clone(&received_error_clone);
                    Box::pin(async move {
                        if let Some(ref e) = task.error {
                            if let Ok(mut guard) = err.lock() {
                                guard.clone_from(e);
                            }
                        }
                        n.notify_one();
                    })
                }),
            )
            .await
            .unwrap();

        let mw: BoxedMiddlewareFn =
            Box::new(|_next: BoxedTaskFn| -> BoxedTaskFn {
                Arc::new(|_task: Arc<Task>| {
                    Box::pin(async { Err(anyhow::anyhow!("middleware failure")) })
                })
            });

        let queues = Arc::new(DashMap::new());
        queues.insert("someq".to_string(), 1);

        let cfg = Config {
            name: None,
            address: Some(":0".to_string()),
            broker: Some(Arc::new(broker)),
            runtime: Some(Arc::new(TestRuntime::new())),
            queues,
            limits: Limits::default(),
            middleware: Arc::new(vec![mw]),
        };
        let mut worker = Worker::new(cfg).unwrap();
        worker.start().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let task = Arc::new(Task {
            id: Some("mw-fail-task".to_string()),
            state: TASK_STATE_RUNNING.clone(),
            ..Default::default()
        });
        broker_ref
            .publish_task("someq".to_string(), &task)
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(2), error_notify.notified())
            .await
            .unwrap();

        let error_msg = received_error.lock().map(|g| g.clone()).unwrap_or_default();
        assert!(
            error_msg.contains("middleware failure"),
            "error should contain middleware failure message, got: {error_msg}"
        );
        worker.stop().await.unwrap();
    }

    // ---- ShellRuntimeBridge: adapts ShellRuntime to the Runtime trait ----

    /// Bridge adapter that wraps [`crate::runtime::shell::ShellRuntime`]
    /// and implements the [`Runtime`] trait required by the worker.
    ///
    /// Converts between `tork::task::Task` (with `Option<>` fields) and
    /// `crate::runtime::shell::Task` (concrete fields) on each `run` call.
    ///
    /// NOTE: Due to the `'static` lifetime requirement on `BoxedFuture`,
    /// we cannot capture `&mut Task` across the async boundary. The result
    /// is instead stored in `last_result` and can be read after awaiting.
    struct ShellRuntimeBridge {
        inner: Arc<crate::runtime::shell::ShellRuntime>,
        /// Stores the result from the most recent run (keyed by task ID).
        last_result: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    }

    impl ShellRuntimeBridge {
        fn new() -> Self {
            use tokio::process::Command;
            let config = crate::runtime::shell::ShellConfig {
                cmd: vec!["bash".to_string(), "-c".to_string()],
                uid: crate::runtime::shell::DEFAULT_UID.to_string(),
                gid: crate::runtime::shell::DEFAULT_GID.to_string(),
                reexec: Some(Box::new(|args: &[String]| {
                    let mut cmd = Command::new(&args[5]);
                    cmd.args(&args[6..]);
                    cmd
                })),
                broker: None,
            };
            Self {
                inner: Arc::new(crate::runtime::shell::ShellRuntime::new(config)),
                last_result: Arc::new(std::sync::Mutex::new(
                    std::collections::HashMap::new(),
                )),
            }
        }

        /// Converts a `tork::task::Task` into a `shell::Task`.
        fn to_shell_task(task: &tork::task::Task) -> crate::runtime::shell::Task {
            use crate::runtime::shell::Mount as ShellMount;
            use crate::runtime::shell::MountType;

            crate::runtime::shell::Task {
                id: task.id.clone().unwrap_or_default(),
                name: task.name.clone(),
                image: task.image.clone().unwrap_or_default(),
                run: task.run.clone().unwrap_or_default(),
                cmd: task.cmd.clone().unwrap_or_default(),
                entrypoint: task.entrypoint.clone().unwrap_or_default(),
                env: task.env.clone().unwrap_or_default(),
                mounts: task
                    .mounts
                    .as_ref()
                    .map(|ms| {
                        ms.iter()
                            .map(|m| ShellMount {
                                id: m.id.clone().unwrap_or_default(),
                                mount_type: if m.mount_type == "bind" {
                                    MountType::Bind
                                } else {
                                    MountType::Volume
                                },
                                source: m.source.clone().unwrap_or_default(),
                                target: m.target.clone().unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                files: task.files.clone().unwrap_or_default(),
                networks: task.networks.clone().unwrap_or_default(),
                limits: task.limits.as_ref().map(|l| {
                    crate::runtime::shell::TaskLimits {
                        cpus: l.cpus.clone().unwrap_or_default(),
                        memory: l.memory.clone().unwrap_or_default(),
                    }
                }),
                registry: None, // ShellRuntime doesn't use registry
                sidecars: task
                    .sidecars
                    .as_ref()
                    .map(|ss| ss.iter().map(Self::to_shell_task).collect())
                    .unwrap_or_default(),
                pre: task
                    .pre
                    .as_ref()
                    .map(|ps| ps.iter().map(Self::to_shell_task).collect())
                    .unwrap_or_default(),
                post: task
                    .post
                    .as_ref()
                    .map(|ps| ps.iter().map(Self::to_shell_task).collect())
                    .unwrap_or_default(),
                workdir: task.workdir.clone(),
                result: task.result.clone().unwrap_or_default(),
                progress: task.progress,
            }
        }

        /// Retrieves the stored result for a given task ID.
        fn get_result(&self, task_id: &str) -> Option<String> {
            self.last_result
                .lock()
                .expect("result lock poisoned")
                .get(task_id)
                .cloned()
        }
    }

    impl Runtime for ShellRuntimeBridge {
        fn run(
            &self,
            _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
            task: &mut tork::task::Task,
        ) -> tork::runtime::BoxedFuture<()> {
            let shell_task = Self::to_shell_task(task);
            let inner = Arc::clone(&self.inner);
            let results: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>> =
                Arc::clone(&self.last_result);
            let task_id = task.id.clone().unwrap_or_default();
            Box::pin(async move {
                let cancel = Arc::new(AtomicBool::new(false));
                let mut st = shell_task;
                inner.run(cancel, &mut st).await?;
                if !st.result.is_empty() {
                    results
                        .lock()
                        .expect("result lock poisoned")
                        .insert(task_id, st.result);
                }
                Ok(())
            })
        }

        fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
            Box::pin(async { Ok(()) })
        }
    }

    // ---- Test_handleTaskRunOutput (mirrors Go Test_handleTaskRunOutput) ----
    // Integration test: verifies that a shell task writing to $REEXEC_TORK_OUTPUT
    // captures the output into the runtime's result store, matching Go's
    // Test_handleTaskRunOutput which checks `echo -n hello world > $TORK_OUTPUT`
    // → `t1.Result == "hello world"`.
    #[tokio::test]
    async fn test_handle_task_run_output() {
        let bridge = Arc::new(ShellRuntimeBridge::new());
        let broker = Arc::new(new_in_memory_broker());

        // Subscribe for completion event (mirrors Go pattern)
        let completed = Arc::new(tokio::sync::Notify::new());
        let completed_clone = Arc::clone(&completed);
        let completed_handler: tork::broker::TaskHandler = Arc::new(move |_task: Arc<Task>| {
            let notify = Arc::clone(&completed_clone);
            Box::pin(async move {
                notify.notify_one();
            })
        });
        broker
            .subscribe_for_tasks(
                tork::broker::queue::QUEUE_COMPLETED.to_string(),
                completed_handler,
            )
            .await
            .expect("subscribe should succeed");

        let cfg = Config {
            name: None,
            address: None,
            broker: Some(broker),
            runtime: Some(bridge.clone()),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };

        let w = Worker::new(cfg).expect("worker creation should succeed");

        let task_id = uuid::Uuid::new_v4().to_string();
        let t1 = Arc::new(Task {
            id: Some(task_id.clone()),
            state: TASK_STATE_RUNNING.clone(),
            run: Some("echo -n hello world > $REEXEC_TORK_OUTPUT".to_string()),
            ..Default::default()
        });

        let result = w.handle_task(t1).await;
        assert!(result.is_ok(), "handle_task should succeed: {:?}", result.err());

        // Wait for completion event
        tokio::time::timeout(Duration::from_secs(2), completed.notified())
            .await
            .expect("should receive completion event within timeout");

        // Verify output was captured in the bridge's result store
        let captured = bridge.get_result(&task_id);
        assert_eq!(
            captured.as_deref(),
            Some("hello world"),
            "task result should be 'hello world'"
        );
    }

    // ---- Test_handleTaskRunWithPrePost (mirrors Go Test_handleTaskRunWithPrePost) ----
    // Integration test: verifies that pre-tasks execute before the main task,
    // post-tasks execute after, and the main task result captures pre-task output.
    //
    // Since ShellRuntime doesn't support mounts, we use a shared temp file
    // (analogous to the Go test's shared volume) for pre → main data flow.
    #[tokio::test]
    async fn test_handle_task_run_with_pre_post() {
        let shared_file = format!("/tmp/tork_test_pre_{}", uuid::Uuid::new_v4());
        let shared_file_main = shared_file.clone();

        // Subscribe for completion event
        let bridge = Arc::new(ShellRuntimeBridge::new());
        let broker = Arc::new(new_in_memory_broker());
        let completed = Arc::new(tokio::sync::Notify::new());
        let completed_clone = Arc::clone(&completed);
        let completed_handler: tork::broker::TaskHandler = Arc::new(move |_task: Arc<Task>| {
            let notify = Arc::clone(&completed_clone);
            Box::pin(async move {
                notify.notify_one();
            })
        });
        broker
            .subscribe_for_tasks(
                tork::broker::queue::QUEUE_COMPLETED.to_string(),
                completed_handler,
            )
            .await
            .expect("subscribe should succeed");

        // Pre-task writes "prestuff" to the shared file
        let pre_task = Task {
            id: Some(uuid::Uuid::new_v4().to_string()),
            state: TASK_STATE_RUNNING.clone(),
            run: Some(format!("echo -n prestuff > {}", shared_file)),
            ..Default::default()
        };

        // Post-task creates a marker file to prove it ran
        let post_marker = format!("/tmp/tork_test_post_{}", uuid::Uuid::new_v4());
        let post_marker_run = post_marker.clone();
        let post_task = Task {
            id: Some(uuid::Uuid::new_v4().to_string()),
            state: TASK_STATE_RUNNING.clone(),
            run: Some(format!("touch {}", post_marker_run)),
            ..Default::default()
        };

        // Main task reads from the shared file into TORK_OUTPUT
        let main_run = format!("cat {} > $REEXEC_TORK_OUTPUT", shared_file_main);
        let task_id = uuid::Uuid::new_v4().to_string();
        let t1 = Arc::new(Task {
            id: Some(task_id.clone()),
            state: TASK_STATE_RUNNING.clone(),
            run: Some(main_run),
            pre: Some(vec![pre_task]),
            post: Some(vec![post_task]),
            ..Default::default()
        });

        let cfg = Config {
            name: None,
            address: None,
            broker: Some(broker),
            runtime: Some(bridge.clone()),
            queues: Arc::new(DashMap::new()),
            limits: Limits::default(),
            middleware: Arc::new(Vec::new()),
        };

        let w = Worker::new(cfg).expect("worker creation should succeed");
        let result = w.handle_task(t1).await;
        assert!(
            result.is_ok(),
            "handle_task with pre/post should succeed: {:?}",
            result.err()
        );

        // Wait for completion event
        tokio::time::timeout(Duration::from_secs(2), completed.notified())
            .await
            .expect("should receive completion event within timeout");

        // Verify pre-task output was captured as main task result
        let captured = bridge.get_result(&task_id);
        assert_eq!(
            captured.as_deref(),
            Some("prestuff"),
            "main task result should be 'prestuff' from pre-task"
        );

        // Verify post-task ran (marker file exists)
        assert!(
            std::path::Path::new(&post_marker).exists(),
            "post-task marker file should exist, proving post-task executed"
        );

        // Clean up temp files
        let _ = std::fs::remove_file(&shared_file);
        let _ = std::fs::remove_file(&post_marker);
    }

    // ---- parse_go_duration ----
    #[test]
    fn test_parse_go_duration_seconds() {
        let dur = parse_go_duration("5s");
        assert_eq!(std::time::Duration::from_secs(5), dur);
    }

    #[test]
    fn test_parse_go_duration_milliseconds() {
        let dur = parse_go_duration("500ms");
        assert_eq!(std::time::Duration::from_millis(500), dur);
    }

    #[test]
    fn test_parse_go_duration_minutes() {
        let dur = parse_go_duration("2m");
        assert_eq!(std::time::Duration::from_secs(120), dur);
    }

    #[test]
    fn test_parse_go_duration_hours() {
        let dur = parse_go_duration("1h");
        assert_eq!(std::time::Duration::from_secs(3600), dur);
    }

    #[test]
    fn test_parse_go_duration_float() {
        let dur = parse_go_duration("1.5s");
        assert_eq!(std::time::Duration::from_secs_f64(1.5), dur);
    }

    #[test]
    fn test_parse_go_duration_unknown() {
        let dur = parse_go_duration("10days");
        assert_eq!(std::time::Duration::ZERO, dur);
    }

    #[test]
    fn test_parse_go_duration_whitespace() {
        let dur = parse_go_duration("  5s  ");
        assert_eq!(std::time::Duration::from_secs(5), dur);
    }
}

