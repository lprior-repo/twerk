//! Worker implementation module.
//!
//! Provides the core Worker struct that handles task execution.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use dashmap::DashMap;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use twerk_core::id::TaskId;
use twerk_core::task::Task;
use twerk_core::uuid::new_short_uuid;

use crate::broker::is_worker_queue;
use crate::broker::{Broker, TaskHandler};
use crate::runtime::Runtime as RuntimeTrait;

use super::super::api::WorkerApi;
use super::execution::execute_task;
use super::heartbeat::send_heartbeats;
use super::types::{Config, Limits, NewWorkerError, RunningTask};

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
    /// Notification for task completion
    tasks_notify: Arc<tokio::sync::Notify>,
    /// Default resource limits
    limits: Limits,
    /// Worker API
    api: WorkerApi,
}

impl Worker {
    /// Create a new Worker from configuration
    ///
    /// # Errors
    ///
    /// Returns `NewWorkerError` if the worker cannot be created.
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
            tasks_notify: Arc::new(tokio::sync::Notify::new()),
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
    ///
    /// # Errors
    ///
    /// Returns an error if the worker fails to start.
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting worker {} ({})", self.name, self.id);

        // Start the API server
        self.api
            .start()
            .await
            .context("Failed to start API server")?;

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
                    let _ = runtime
                        .health_check()
                        .await
                        .map_err(|e| warn!("Runtime health check failed during cancel: {}", e));
                    Ok(())
                })
            });

            let _ = cancel_broker
                .subscribe_for_tasks(cancel_queue, handler)
                .await;
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
                let tasks_notify = self.tasks_notify.clone();
                let qname_clone = qname.clone();

                let handler: TaskHandler = Arc::new(move |task: Arc<Task>| {
                    let runtime = runtime.clone();
                    let broker = broker.clone();
                    let limits = limits.clone();
                    let active_tasks = active_tasks.clone();
                    let tasks_notify = tasks_notify.clone();
                    Box::pin(async move {
                        execute_task(task, runtime, broker, limits, active_tasks, tasks_notify).await
                    })
                });

                // Subscribe and run - the subscription is blocking
                let broker_for_subscribe = self.broker.clone();
                tokio::spawn(async move {
                    let _ = broker_for_subscribe
                        .subscribe_for_tasks(qname_clone, handler)
                        .await;
                });
            }
        }

        // Start heartbeat sender
        let broker = self.broker.clone();
        let runtime = self.runtime.clone();
        let id = self.id.clone();
        let name = self.name.clone();
        let port = self.api.port();
        let stop_rx = self.stop_tx.subscribe();

        tokio::spawn(async move {
            send_heartbeats(broker, runtime, id, name, port, stop_rx).await;
        });

        Ok(())
    }

    /// Stop the worker gracefully
    ///
    /// # Errors
    ///
    /// Returns an error if the worker fails to stop gracefully.
    pub async fn stop(&self) -> Result<()> {
        debug!("Shutting down worker {}", self.id);

        // Send stop signal
        let _ = self.stop_tx.send(());

        // Wait for active tasks to complete (with timeout)
        let _ = tokio::time::timeout(
            Duration::from_secs(15),
            async {
                while !self.active_tasks.is_empty() {
                    self.tasks_notify.notified().await;
                }
            }
        ).await;

        if !self.active_tasks.is_empty() {
            warn!(
                "Worker stopping with {} tasks still active",
                self.active_tasks.len()
            );
        }

        // Shutdown broker
        self.broker
            .shutdown()
            .await
            .context("Broker shutdown failed")?;

        Ok(())
    }
}
