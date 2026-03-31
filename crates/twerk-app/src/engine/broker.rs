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
//! - `create_broker()` dispatches on type (inmemory / rabbitmq)
//! - delegates RabbitMQ to the infrastructure implementation

use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;
use twerk_core::node::Node;
use twerk_core::task::Task;
use twerk_infrastructure::broker::{
    inmemory::InMemoryBroker, rabbitmq::RabbitMQBroker, BoxedFuture, Broker, EventHandler,
    HeartbeatHandler, JobHandler, QueueInfo, RabbitMQOptions, TaskHandler, TaskLogPartHandler,
    TaskProgressHandler,
};

use super::engine_helpers::ensure_config_loaded;

// ── Broker type enumeration ────────────────────────────────────

/// Broker type enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerType {
    /// In-memory broker
    InMemory,
    /// `RabbitMQ` broker
    RabbitMQ,
}

impl BrokerType {
    /// Parse broker type from string.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
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

// ── Broker trait delegation ────────────────────────────────────

macro_rules! delegate {
    ($method:ident ( $( $arg:ident : $ty:ty ),* ) -> $ret:ty) => {
        fn $method(&self, $($arg: $ty),*) -> $ret {
            let inner = self.inner.clone();
            Box::pin(async move {
                let guard = inner.read().await;
                match guard.as_ref() {
                    Some(b) => b.$method($($arg),*).await,
                    None => Err(anyhow!(
                        "Broker not initialized. You must call engine.Start() first"
                    )),
                }
            })
        }
    };
}

impl Broker for BrokerProxy {
    // Methods with reference args must clone the data to satisfy 'static bound.
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let inner = self.inner.clone();
        let task = task.clone();
        Box::pin(async move {
            let guard = inner.read().await;
            match guard.as_ref() {
                Some(b) => b.publish_task(qname, &task).await,
                None => Err(anyhow!(
                    "Broker not initialized. You must call engine.Start() first"
                )),
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
                None => Err(anyhow!(
                    "Broker not initialized. You must call engine.Start() first"
                )),
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
                None => Err(anyhow!(
                    "Broker not initialized. You must call engine.Start() first"
                )),
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
                None => Err(anyhow!(
                    "Broker not initialized. You must call engine.Start() first"
                )),
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
    delegate!(subscribe_for_task_log_part(handler: TaskLogPartHandler) -> BoxedFuture<()>);
    delegate!(queues() -> BoxedFuture<Vec<QueueInfo>>);
    delegate!(queue_info(qname: String) -> BoxedFuture<QueueInfo>);
    delegate!(delete_queue(qname: String) -> BoxedFuture<()>);
    delegate!(health_check() -> BoxedFuture<()>);
    delegate!(shutdown() -> BoxedFuture<()>);
}

// ── Config helpers ─────────────────────────────────────────────

/// Get a string from environment variables (`TWERK_` prefix, dots → underscores).
fn env_string(key: &str) -> String {
    let env_key = format!("TWERK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| twerk_infrastructure::config::string(key))
}

/// Get a string with default from environment variables.
fn env_string_default(key: &str, default: &str) -> String {
    let value = env_string(key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

/// Get a duration from environment (parsed as milliseconds) with default.
fn env_duration_ms_default(key: &str, default: u64) -> Duration {
    let value = env_string(key);
    if value.is_empty() {
        Duration::from_millis(default)
    } else {
        value
            .parse::<u64>()
            .map(Duration::from_millis)
            .unwrap_or_else(|_| Duration::from_millis(default))
    }
}

/// Get a bool from environment (parsed as "true"/"false") with default.
fn env_bool(key: &str, default: bool) -> bool {
    let value = env_string(key);
    if value.is_empty() {
        default
    } else {
        value == "true" || value == "1"
    }
}

// ── Broker factory ─────────────────────────────────────────────

/// Default `RabbitMQ` URL (matches Go default).
const DEFAULT_RABBITMQ_URL: &str = "amqp://guest:guest@localhost:5672/";

/// Default consumer timeout in milliseconds.
const DEFAULT_CONSUMER_TIMEOUT_MS: u64 = 30_000;

/// Queue type constant — classic.
const QUEUE_TYPE_CLASSIC: &str = "classic";

/// Creates a broker based on the given type string.
///
/// Matches Go `createBroker()`:
/// - `"inmemory"` → [`InMemoryBroker`]
/// - `"rabbitmq"` → [`RabbitMQBroker`] with full config from env
///
/// # Errors
///
/// Returns an error if:
/// - The `RabbitMQ` connection cannot be established
pub async fn create_broker(
    btype: &str,
    engine_id: Option<&str>,
) -> Result<Box<dyn Broker + Send + Sync>> {
    ensure_config_loaded();
    match BrokerType::parse(btype) {
        BrokerType::InMemory => Ok(Box::new(InMemoryBroker::new())),
        BrokerType::RabbitMQ => {
            let url = env_string_default("broker.rabbitmq.url", DEFAULT_RABBITMQ_URL);
            let management_url = {
                let v = env_string("broker.rabbitmq.management.url");
                if v.is_empty() {
                    None
                } else {
                    Some(v)
                }
            };
            let _consumer_timeout = env_duration_ms_default(
                "broker.rabbitmq.consumer.timeout",
                DEFAULT_CONSUMER_TIMEOUT_MS,
            );
            let durable = env_bool("broker.rabbitmq.durable.queues", false);
            let queue_type = env_string_default("broker.rabbitmq.queue.type", QUEUE_TYPE_CLASSIC);

            let broker = RabbitMQBroker::new(
                &url,
                RabbitMQOptions {
                    management_url,
                    durable_queues: durable,
                    queue_type,
                    consumer_timeout: Some(_consumer_timeout),
                },
                engine_id,
            )
            .await
            .map_err(|e| anyhow!("unable to connect to RabbitMQ: {e}"))?;

            Ok(Box::new(broker))
        }
    }
}

// ── In-memory broker (Removed, moved to twerk-infrastructure)
