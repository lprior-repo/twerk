//! Broker proxy module
//!
//! This module provides a proxy wrapper around the Broker interface
//! that adds initialization checks, plus factory functions for creating
//! concrete broker implementations.
//!
//! # Go Parity
//!
//! Matches `engine/broker.go`:
//! - [`BrokerProxy`] delegates every `Broker` method with init-check
//! - `create_broker()` dispatches on type (inmemory only)

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use twerk_core::job::JobEvent;
use twerk_core::node::Node;
use twerk_core::task::Task;
use twerk_infrastructure::broker::{
    config::RabbitMQOptions, inmemory::InMemoryBroker, rabbitmq::RabbitMQBroker, BoxedFuture,
    Broker, EventHandler, HeartbeatHandler, JobHandler, QueueInfo, TaskHandler,
    TaskLogPartHandler, TaskProgressHandler,
};

use super::engine_helpers::ensure_config_loaded;

// ── Typed error for broker proxy ───────────────────────────────────

#[derive(Debug, thiserror::Error)]
#[error("Broker not initialized. You must call engine.Start() first")]
struct BrokerNotInitialized;

// ── Broker type enumeration ────────────────────────────────────

/// Broker type enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerType {
    /// In-memory broker
    InMemory,
    /// RabbitMQ broker
    RabbitMQ,
}

impl BrokerType {
    /// Parse broker type from string.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "rabbitmq" => Self::RabbitMQ,
            _ => Self::InMemory,
        }
    }
}

// ── Broker proxy ───────────────────────────────────────────────

/// [`BrokerProxy`] wraps a [`Broker`] and adds initialization checks.
///
/// Every method reads the inner `Option`, delegates to the real broker if
/// present, or returns a "not initialized" error — matching Go's
/// `brokerProxy.checkInit()` pattern exactly.
#[derive(Clone)]
pub struct BrokerProxy {
    inner: Arc<RwLock<Option<Box<dyn Broker + Send + Sync>>>>,
}

impl std::fmt::Debug for BrokerProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrokerProxy").finish()
    }
}

impl BrokerProxy {
    /// Creates a new empty broker proxy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// Initializes the broker based on the given type string.
    ///
    /// Matches Go `initBroker()`:
    /// - Reads config for the given type
    /// - Delegates to `create_broker()`
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying broker creation fails.
    pub async fn init(&self, broker_type: &str, engine_id: Option<&str>) -> Result<()> {
        let broker = create_broker(broker_type, engine_id).await?;
        *self.inner.write().await = Some(broker);
        Ok(())
    }

    /// Sets a custom broker implementation.
    pub async fn set_broker(&self, broker: Box<dyn Broker + Send + Sync>) {
        *self.inner.write().await = Some(broker);
    }

    /// Clones the inner Arc for sharing.
    #[must_use]
    pub fn clone_inner(&self) -> BrokerProxy {
        BrokerProxy {
            inner: self.inner.clone(),
        }
    }

    /// Checks if the broker is initialized (matches Go `checkInit`).
    ///
    /// # Errors
    ///
    /// Returns an error if the broker has not been initialized.
    pub async fn check_init(&self) -> Result<()> {
        if self.inner.read().await.is_none() {
            return Err(BrokerNotInitialized.into());
        }
        Ok(())
    }
}

impl Default for BrokerProxy {
    fn default() -> Self {
        Self::new()
    }
}

// ── Broker trait delegation ────────────────────────────────────

macro_rules! delegate {
    ($method:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty) => {
        fn $method(&self, $($arg: $ty),*) -> $ret {
            let inner = self.inner.clone();
            Box::pin(async move {
                let guard = inner.read().await;
                match guard.as_ref() {
                    Some(b) => b.$method($($arg),*).await,
                    None => Err(BrokerNotInitialized.into()),
                }
            })
        }
    };
}

impl Broker for BrokerProxy {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let inner = self.inner.clone();
        let task = task.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            match guard.as_ref() {
                Some(b) => b.publish_task(qname, &task).await,
                None => Err(BrokerNotInitialized.into()),
            }
        })
    }

    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()> {
        let inner = self.inner.clone();
        let task = task.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            match guard.as_ref() {
                Some(b) => b.publish_task_progress(&task).await,
                None => Err(BrokerNotInitialized.into()),
            }
        })
    }

    fn publish_job(&self, job: &twerk_core::job::Job) -> BoxedFuture<()> {
        let inner = self.inner.clone();
        let job = job.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            match guard.as_ref() {
                Some(b) => b.publish_job(&job).await,
                None => Err(BrokerNotInitialized.into()),
            }
        })
    }

    fn publish_task_log_part(&self, part: &twerk_core::task::TaskLogPart) -> BoxedFuture<()> {
        let inner = self.inner.clone();
        let part = part.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            match guard.as_ref() {
                Some(b) => b.publish_task_log_part(&part).await,
                None => Err(BrokerNotInitialized.into()),
            }
        })
    }

    delegate!(subscribe_for_tasks(qname: String, handler: TaskHandler) -> BoxedFuture<()>);
    delegate!(subscribe_for_task_progress(handler: TaskProgressHandler) -> BoxedFuture<()>);
    delegate!(publish_heartbeat(node: Node) -> BoxedFuture<()>);
    delegate!(subscribe_for_heartbeats(handler: HeartbeatHandler) -> BoxedFuture<()>);
    delegate!(subscribe_for_jobs(handler: JobHandler) -> BoxedFuture<()>);
    delegate!(publish_event(topic: String, event: serde_json::Value) -> BoxedFuture<()>);
    delegate!(subscribe_for_events(pattern: String, handler: EventHandler) -> BoxedFuture<()>);
    delegate!(subscribe(pattern: String) -> BoxedFuture<tokio::sync::broadcast::Receiver<JobEvent>>);
    delegate!(subscribe_for_task_log_part(handler: TaskLogPartHandler) -> BoxedFuture<()>);
    delegate!(queues() -> BoxedFuture<Vec<QueueInfo>>);
    delegate!(queue_info(qname: String) -> BoxedFuture<QueueInfo>);
    delegate!(delete_queue(qname: String) -> BoxedFuture<()>);
    delegate!(health_check() -> BoxedFuture<()>);
    delegate!(shutdown() -> BoxedFuture<()>);
}

// ── Broker factory ─────────────────────────────────────────────

/// Creates a broker based on the given type string.
///
/// Matches Go `createBroker()`:
/// - `"inmemory"` → [`InMemoryBroker`]
/// - `"rabbitmq"` → [`RabbitMQBroker`]
///
/// # Errors
///
/// Returns an error if the RabbitMQ connection cannot be established.
pub async fn create_broker(
    btype: &str,
    engine_id: Option<&str>,
) -> Result<Box<dyn Broker + Send + Sync>> {
    ensure_config_loaded();
    match BrokerType::parse(btype) {
        BrokerType::InMemory => Ok(Box::new(InMemoryBroker::new())),
        BrokerType::RabbitMQ => {
            let broker = RabbitMQBroker::new(
                "amqp://guest:guest@localhost:5672/%2f",
                RabbitMQOptions::default(),
                engine_id,
            )
            .await?;
            Ok(Box::new(broker))
        }
    }
}
