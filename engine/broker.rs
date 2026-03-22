//! Broker proxy module
//!
//! This module provides a proxy wrapper around the Broker interface
//! that adds initialization checks.

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tork::broker::{
    Broker, EventHandler, HeartbeatHandler, JobHandler, QueueInfo, TaskHandler, TaskLogPartHandler,
    TaskProgressHandler,
};
use tork::node::Node;
use tork::task::Task;

/// Broker type enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerType {
    /// In-memory broker
    InMemory,
    /// RabbitMQ broker
    RabbitMQ,
    /// Unknown broker type
    Unknown(String),
}

impl BrokerType {
    /// Parse broker type from string
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "inmemory" => Self::InMemory,
            "rabbitmq" => Self::RabbitMQ,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// BrokerProxy wraps a Broker and adds initialization checks
#[derive(Clone)]
pub struct BrokerProxy {
    inner: Arc<RwLock<Option<Box<dyn Broker + Send + Sync>>>>,
}

impl BrokerProxy {
    /// Creates a new empty broker proxy
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Initializes the broker based on the given type string
    pub async fn init(&self, broker_type: &str) -> Result<()> {
        let broker: Box<dyn Broker + Send + Sync> = match BrokerType::from_str(broker_type) {
            BrokerType::InMemory => Box::new(InMemoryBroker::new()),
            BrokerType::RabbitMQ => {
                // For RabbitMQ, we would read config and create a real broker
                // For now, fall back to in-memory
                tracing::warn!("RabbitMQ broker not yet implemented, using in-memory");
                Box::new(InMemoryBroker::new())
            }
            BrokerType::Unknown(t) => {
                tracing::warn!("Unknown broker type '{}', using in-memory", t);
                Box::new(InMemoryBroker::new())
            }
        };
        *self.inner.write().await = Some(broker);
        Ok(())
    }

    /// Sets a custom broker implementation
    pub async fn set_broker(&self, broker: Box<dyn Broker + Send + Sync>) {
        *self.inner.write().await = Some(broker);
    }

    /// Clones the inner Arc for sharing
    pub fn clone_inner(&self) -> BrokerProxy {
        BrokerProxy {
            inner: self.inner.clone(),
        }
    }

    /// Checks if the broker is initialized
    pub async fn check_init(&self) -> Result<()> {
        if self.inner.read().await.is_none() {
            return Err(anyhow!(
                "Broker not initialized. You must call engine.Start() first"
            ));
        }
        Ok(())
    }
}

impl Default for BrokerProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl Broker for BrokerProxy {
    fn publish_task(&self, qname: String, task: &Task) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        let task = task.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.publish_task(qname, &task).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn subscribe_for_tasks(
        &self,
        qname: String,
        handler: TaskHandler,
    ) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.subscribe_for_tasks(qname, handler).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn publish_task_progress(&self, task: &Task) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        let task = task.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.publish_task_progress(&task).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn subscribe_for_task_progress(
        &self,
        handler: TaskProgressHandler,
    ) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.subscribe_for_task_progress(handler).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn publish_heartbeat(&self, node: Node) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.publish_heartbeat(node).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.subscribe_for_heartbeats(handler).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn publish_job(&self, job: &tork::job::Job) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        let job = job.deep_clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.publish_job(&job).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn subscribe_for_jobs(&self, handler: JobHandler) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.subscribe_for_jobs(handler).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn publish_event(
        &self,
        topic: String,
        event: serde_json::Value,
    ) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.publish_event(topic, event).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn subscribe_for_events(
        &self,
        pattern: String,
        handler: EventHandler,
    ) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.subscribe_for_events(pattern, handler).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn publish_task_log_part(
        &self,
        part: &tork::task::TaskLogPart,
    ) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        let part = part.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.publish_task_log_part(&part).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn subscribe_for_task_log_part(
        &self,
        handler: TaskLogPartHandler,
    ) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.subscribe_for_task_log_part(handler).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn queues(&self) -> tork::broker::BoxedFuture<Vec<QueueInfo>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.queues().await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn queue_info(&self, qname: String) -> tork::broker::BoxedFuture<QueueInfo> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.queue_info(qname).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn delete_queue(&self, qname: String) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.delete_queue(qname).await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn health_check(&self) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.health_check().await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }

    fn shutdown(&self) -> tork::broker::BoxedFuture<()> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            if let Some(broker) = guard.as_ref() {
                broker.shutdown().await
            } else {
                Err(anyhow!("Broker not initialized").into())
            }
        })
    }
}

/// In-memory broker implementation for testing
#[derive(Debug)]
pub struct InMemoryBroker {
    queues: std::collections::HashMap<String, Vec<serde_json::Value>>,
}

impl InMemoryBroker {
    pub fn new() -> Self {
        Self {
            queues: std::collections::HashMap::new(),
        }
    }
}

impl Broker for InMemoryBroker {
    fn publish_task(&self, _qname: String, _task: &Task) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_tasks(
        &self,
        _qname: String,
        _handler: TaskHandler,
    ) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_task_progress(&self, _task: &Task) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_task_progress(
        &self,
        _handler: TaskProgressHandler,
    ) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_heartbeat(&self, _node: Node) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_heartbeats(
        &self,
        _handler: HeartbeatHandler,
    ) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_job(&self, _job: &tork::job::Job) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_jobs(&self, _handler: JobHandler) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_event(
        &self,
        _topic: String,
        _event: serde_json::Value,
    ) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_events(
        &self,
        _pattern: String,
        _handler: EventHandler,
    ) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_task_log_part(
        &self,
        _part: &tork::task::TaskLogPart,
    ) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_task_log_part(
        &self,
        _handler: TaskLogPartHandler,
    ) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn queues(&self) -> tork::broker::BoxedFuture<Vec<QueueInfo>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn queue_info(&self, _qname: String) -> tork::broker::BoxedFuture<QueueInfo> {
        Box::pin(async {
            Ok(QueueInfo {
                name: _qname,
                size: 0,
                subscribers: 0,
                unacked: 0,
            })
        })
    }

    fn delete_queue(&self, _qname: String) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn health_check(&self) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> tork::broker::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}
