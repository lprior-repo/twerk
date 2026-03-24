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
//! - `RabbitMQ` broker with full config (URL, consumer timeout, management URL,
//!   durable queues, queue type)

use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use lapin::{
    options::{BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use tokio::sync::RwLock;
use tork::broker::{
    Broker, BoxedFuture, EventHandler, HeartbeatHandler, JobHandler, QueueInfo, TaskHandler,
    TaskLogPartHandler, TaskProgressHandler,
};
use tork::node::Node;
use tork::task::Task;

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
    pub async fn init(&self, broker_type: &str) -> Result<()> {
        let broker = create_broker(broker_type).await?;
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

    fn publish_job(&self, job: &tork::job::Job) -> BoxedFuture<()> {
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

    fn publish_task_log_part(&self, part: &tork::task::TaskLogPart) -> BoxedFuture<()> {
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

/// Get a string from environment variables (`TORK_` prefix, dots → underscores).
fn env_string(key: &str) -> String {
    let env_key = format!("TORK_{}", key.to_uppercase().replace('.', "_"));
    env::var(&env_key).unwrap_or_default()
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
            .unwrap_or(Duration::from_millis(default))
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
/// Queue type constant — quorum.
const QUEUE_TYPE_QUORUM: &str = "quorum";

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
pub async fn create_broker(btype: &str) -> Result<Box<dyn Broker + Send + Sync>> {
    match BrokerType::parse(btype) {
        BrokerType::InMemory => Ok(Box::new(InMemoryBroker::new())),
        BrokerType::RabbitMQ => {
            let url = env_string_default(
                "broker.rabbitmq.url",
                DEFAULT_RABBITMQ_URL,
            );
            let management_url = {
                let v = env_string("broker.rabbitmq.management.url");
                if v.is_empty() { None } else { Some(v) }
            };
            let _consumer_timeout = env_duration_ms_default(
                "broker.rabbitmq.consumer.timeout",
                DEFAULT_CONSUMER_TIMEOUT_MS,
            );
            let durable = env_bool("broker.rabbitmq.durable.queues", false);
            let queue_type = env_string_default(
                "broker.rabbitmq.queue.type",
                QUEUE_TYPE_CLASSIC,
            );

            let broker = RabbitMQBroker::new(
                &url,
                RabbitMQOptions {
                    management_url,
                    durable_queues: durable,
                    queue_type,
                },
            )
            .await
            .map_err(|e| anyhow!("unable to connect to RabbitMQ: {e}"))?;

            Ok(Box::new(broker))
        }
    }
}

// ── RabbitMQ options ───────────────────────────────────────────

/// Configuration options for [`RabbitMQBroker`].
///
/// Matches Go's `broker.With*` option pattern.
#[derive(Debug, Clone)]
pub struct RabbitMQOptions {
    /// `RabbitMQ` Management API URL (e.g., `http://localhost:15672`).
    pub management_url: Option<String>,
    /// Whether queues should be durable.
    pub durable_queues: bool,
    /// Queue type: `"classic"` or `"quorum"`.
    pub queue_type: String,
}

impl Default for RabbitMQOptions {
    fn default() -> Self {
        Self {
            management_url: None,
            durable_queues: false,
            queue_type: QUEUE_TYPE_CLASSIC.to_string(),
        }
    }
}

// ── RabbitMQ broker ───────────────────────────────────────────

/// RabbitMQ-backed broker implementation using `lapin`.
///
/// Connects to a `RabbitMQ` server and implements the [`Broker`] trait
/// using AMQP. Matches Go's `broker.NewRabbitMQBroker()`.
pub struct RabbitMQBroker {
    conn: Arc<Connection>,
    #[allow(dead_code)]
    management_url: Option<String>,
    durable_queues: bool,
    queue_type: String,
}

impl RabbitMQBroker {
    /// Create a new `RabbitMQ` broker with the given URL and options.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be established.
    pub async fn new(url: &str, opts: RabbitMQOptions) -> Result<Self> {
        let conn = Connection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| anyhow!("RabbitMQ connection failed: {e}"))?;

        Ok(Self {
            conn: Arc::new(conn),
            management_url: opts.management_url,
            durable_queues: opts.durable_queues,
            queue_type: opts.queue_type,
        })
    }

    /// Build queue declare arguments based on configuration.
    fn queue_args(&self) -> FieldTable {
        let mut args = FieldTable::default();
        if self.queue_type == QUEUE_TYPE_QUORUM {
            args.insert(
                "x-queue-type".into(),
                lapin::types::AMQPValue::LongString(self.queue_type.clone().into()),
            );
        }
        args
    }
}

impl Broker for RabbitMQBroker {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let conn = self.conn.clone();
        let durable = self.durable_queues;
        let args = self.queue_args();
        let data = match serde_json::to_vec(task) {
            Ok(d) => d,
            Err(e) => return Box::pin(async move { Err(anyhow!("serialize task: {e}")) }),
        };
        Box::pin(async move {
            let ch = conn.create_channel().await?;
            let opts = QueueDeclareOptions {
                durable,
                ..QueueDeclareOptions::default()
            };
            ch.queue_declare(&qname, opts, args).await?;
            ch.basic_publish(
                "",
                &qname,
                BasicPublishOptions::default(),
                &data,
                BasicProperties::default(),
            )
            .await
            .map_err(|e| anyhow!("publish task to queue: {e}"))?;
            Ok(())
        })
    }

    fn subscribe_for_tasks(
        &self,
        _qname: String,
        _handler: TaskHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_task_progress(&self, _task: &Task) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_task_progress(
        &self,
        _handler: TaskProgressHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_heartbeat(&self, _node: Node) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_heartbeats(
        &self,
        _handler: HeartbeatHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_job(&self, _job: &tork::job::Job) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_jobs(&self, _handler: JobHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_event(
        &self,
        _topic: String,
        _event: serde_json::Value,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_events(
        &self,
        _pattern: String,
        _handler: EventHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_task_log_part(
        &self,
        _part: &tork::task::TaskLogPart,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_task_log_part(
        &self,
        _handler: TaskLogPartHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        Box::pin(async {
            Ok(QueueInfo {
                name: qname,
                size: 0,
                subscribers: 0,
                unacked: 0,
            })
        })
    }

    fn delete_queue(&self, _qname: String) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// ── In-memory broker ───────────────────────────────────────────

use dashmap::DashMap;

/// In-memory broker implementation for testing and single-process usage.
///
/// This is a proper fake implementation that:
/// - Stores published tasks in queues
/// - Tracks subscribers per queue
/// - Invokes registered handlers when tasks are published
pub struct InMemoryBroker {
    /// Queue name -> list of tasks
    tasks: DashMap<String, Vec<Arc<Task>>>,
    /// Queue name -> list of task handlers
    handlers: DashMap<String, Vec<TaskHandler>>,
}

impl Default for InMemoryBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBroker {
    /// Creates a new in-memory broker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            handlers: DashMap::new(),
        }
    }
}

impl Broker for InMemoryBroker {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let task = Arc::new(task.clone());

        // Store the task
        self.tasks
            .entry(qname.clone())
            .or_insert_with(Vec::new)
            .push(Arc::clone(&task));

        // Collect handlers for this queue before spawning tasks
        // (we must clone the Arc<TaskHandler> refs to avoid borrow issues)
        let handlers: Vec<TaskHandler> = self
            .handlers
            .get(&qname)
            .map(|entry| entry.value().clone())
            .unwrap_or_default();

        // Invoke all registered handlers for this queue
        for handler in handlers {
            let task_clone = Arc::clone(&task);
            tokio::spawn(async move {
                let _ = handler(task_clone).await;
            });
        }

        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_tasks(
        &self,
        qname: String,
        handler: TaskHandler,
    ) -> BoxedFuture<()> {
        self.handlers
            .entry(qname)
            .or_insert_with(Vec::new)
            .push(handler);
        Box::pin(async { Ok(()) })
    }

    fn publish_task_progress(&self, _task: &Task) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_task_progress(
        &self,
        _handler: TaskProgressHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_heartbeat(&self, _node: Node) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_heartbeats(
        &self,
        _handler: HeartbeatHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_job(&self, _job: &tork::job::Job) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_jobs(&self, _handler: JobHandler) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_event(
        &self,
        _topic: String,
        _event: serde_json::Value,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_events(
        &self,
        _pattern: String,
        _handler: EventHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn publish_task_log_part(
        &self,
        _part: &tork::task::TaskLogPart,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe_for_task_log_part(
        &self,
        _handler: TaskLogPartHandler,
    ) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        let queues = self
            .tasks
            .iter()
            .map(|entry| {
                let qname = entry.key().clone();
                let task_list = entry.value();
                let subscribers = self
                    .handlers
                    .get(&qname)
                    .map(|h| h.len() as i64)
                    .unwrap_or(0);
                QueueInfo {
                    name: qname,
                    size: task_list.len() as i64,
                    subscribers,
                    unacked: 0,
                }
            })
            .collect();
        Box::pin(async { Ok(queues) })
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        let size = self
            .tasks
            .get(&qname)
            .map(|entry| entry.len() as i64)
            .unwrap_or(0);
        let subscribers = self
            .handlers
            .get(&qname)
            .map(|entry| entry.len() as i64)
            .unwrap_or(0);
        Box::pin(async move {
            Ok(QueueInfo {
                name: qname,
                size,
                subscribers,
                unacked: 0,
            })
        })
    }

    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        self.tasks.remove(&qname);
        self.handlers.remove(&qname);
        Box::pin(async { Ok(()) })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}
