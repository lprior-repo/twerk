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
    pub middleware: Arc<Vec<Box<dyn Fn(Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>) -> Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync> + Send + Sync>>>,
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
    /// Current task count
    task_count: std::sync::atomic::AtomicI32,
    /// Task middleware
    middleware: Arc<Vec<Box<dyn Fn(Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>) -> Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync> + Send + Sync>>>,
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
            task_count: std::sync::atomic::AtomicI32::new(0),
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
            counter: &self.task_count,
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

        // Create the actual task handler
        let runtime = Arc::clone(&self.runtime);
        let _task_clone = Arc::clone(&task_for_broker);
        let task_handler: Arc<dyn Fn(Arc<Task>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync> =
            Arc::new(move |task: Arc<Task>| {
                let runtime = Arc::clone(&runtime);
                Box::pin(async move {
                    let mut t = (*task).clone();
                    let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
                    if let Err(e) = runtime.run(ctx, &mut t).await {
                        let now = time::OffsetDateTime::now_utc();
                        t.error = Some(e.to_string());
                        t.failed_at = Some(now);
                        t.state = TASK_STATE_FAILED;
                    } else {
                        let now = time::OffsetDateTime::now_utc();
                        t.completed_at = Some(now);
                        t.state = TASK_STATE_COMPLETED;
                    }
                }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
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
                    Ok(result)
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
        tokio::spawn({
            let id = self.id.clone();
            let name = self.name.clone();
            let start_time = self.start_time;
            let broker = Arc::clone(&self.broker);
            let runtime = Arc::clone(&self.runtime);
            let api_port = self.api.port();

            async move {
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
                                task_count: 0,
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
            task_count: std::sync::atomic::AtomicI32::new(
                self.task_count.load(std::sync::atomic::Ordering::SeqCst),
            ),
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
struct Defer<'a> {
    counter: &'a std::sync::atomic::AtomicI32,
}

impl Drop for Defer<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

use crate::worker::api;
