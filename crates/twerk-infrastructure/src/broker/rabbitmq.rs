//! `RabbitMQ` broker implementation using `lapin`.

use anyhow::{anyhow, Result};
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, BasicQosOptions,
        QueueDeclareOptions, QueueDeleteOptions,
    },
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use serde_json::Value;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

use super::{
    prefixed_queue, queue, BoxedFuture, BoxedHandlerFuture, Broker, EventHandler, HeartbeatHandler,
    JobHandler, QueueInfo, RabbitMQOptions, TaskHandler, TaskLogPartHandler, TaskProgressHandler,
};
use twerk_core::job::Job;
use twerk_core::node::Node;
use twerk_core::task::{Task, TaskLogPart};

/// AMQP message type constants
const MSG_TYPE_TASK: &str = "*twerk.Task";
const MSG_TYPE_JOB: &str = "*twerk.Job";
const MSG_TYPE_NODE: &str = "*twerk.Node";
const MSG_TYPE_TASK_LOG_PART: &str = "*twerk.TaskLogPart";
const MSG_TYPE_EVENT: &str = "*twerk.Event";

// ── Shared subscription helpers ─────────────────────────────────────────────────

/// Creates a formatted `RabbitMQ` connection error message.
#[inline]
fn rabbitmq_conn_err(conn_idx: usize, e: &impl std::fmt::Display) -> anyhow::Error {
    anyhow!("RabbitMQ connection {conn_idx} failed: {e}")
}

/// Type alias for the JSON message handler used in subscriptions.
type JsonHandler = Arc<dyn Fn(Value) -> BoxedHandlerFuture + Send + Sync>;

/// Creates a typed JSON subscription handler that deserializes JSON and invokes the handler.
///
/// This eliminates the repeated `Arc::new(move |val| { ... Box::pin(async move {...}) })`
/// pattern across all `subscribe_for_*` methods (except `subscribe_for_events` which has no deserialization).
fn make_json_handler<T>(handler: Arc<dyn Fn(T) -> BoxedHandlerFuture + Send + Sync>) -> JsonHandler
where
    T: serde::de::DeserializeOwned + Send + 'static,
{
    Arc::new(move |val: Value| {
        let handler = handler.clone();
        Box::pin(async move {
            if let Ok(msg) = serde_json::from_value::<T>(val) {
                handler(msg).await?;
            }
            Ok(())
        })
    })
}

/// Creates a typed JSON subscription handler for types wrapped in Arc.
///
/// Use this for handlers like `TaskHandler` that expect `Arc<T>` instead of T directly.
fn make_json_handler_arc<T>(
    handler: Arc<dyn Fn(Arc<T>) -> BoxedHandlerFuture + Send + Sync>,
) -> JsonHandler
where
    T: serde::de::DeserializeOwned + Send + 'static,
{
    Arc::new(move |val: Value| {
        let handler = handler.clone();
        Box::pin(async move {
            if let Ok(msg) = serde_json::from_value::<T>(val) {
                handler(Arc::new(msg)).await?;
            }
            Ok(())
        })
    })
}

// ── Functional helpers for JSON extraction and type conversion ─────────────────

/// Extracts an i64 from JSON, returning 0 for null/missing values.
/// Idiomatic alternative to `unwrap_or`.
#[inline]
fn extract_i64(val: &Value) -> i64 {
    val.as_i64().map_or(0, |v| v)
}

/// Safely converts i64 to i32, clamping to `i32::MAX`/`i32::MIN` on overflow.
/// For monitoring/metrics where we never want to fail on large counts.
#[inline]
fn clamp_i32(val: i64) -> i32 {
    i32::try_from(val).unwrap_or_else(|_| {
        if val > 0 {
            debug!(
                value = val,
                "i64 overflow on i32 conversion, clamping to MAX"
            );
            i32::MAX
        } else {
            debug!(
                value = val,
                "i64 underflow on i32 conversion, clamping to MIN"
            );
            i32::MIN
        }
    })
}

/// Extracts i32 from JSON, returning 0 for null/missing values.
#[inline]
fn extract_i32(val: &Value) -> i32 {
    clamp_i32(extract_i64(val))
}

/// RabbitMQ-backed broker implementation.
pub struct RabbitMQBroker {
    url: String,
    conn_pool: Arc<Vec<Arc<Connection>>>,
    last_conn_idx: Arc<AtomicUsize>,
    management_url: Option<String>,
    durable_queues: bool,
    queue_type: String,
    consumer_timeout: Option<Duration>,
    shutting_down: Arc<RwLock<bool>>,
    declared_queues: Arc<RwLock<std::collections::HashMap<String, String>>>,
    engine_id: String,
}

impl RabbitMQBroker {
    /// Create a new `RabbitMQ` broker with the given URL and options.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection to `RabbitMQ` fails.
    pub async fn new(url: &str, opts: RabbitMQOptions, engine_id: Option<&str>) -> Result<Self> {
        let engine_id = engine_id.unwrap_or("");
        let conn1 = Connection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| rabbitmq_conn_err(1, &e))?;

        let conn2 = Connection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| rabbitmq_conn_err(2, &e))?;

        let conn3 = Connection::connect(url, ConnectionProperties::default())
            .await
            .map_err(|e| rabbitmq_conn_err(3, &e))?;

        let ch = conn1.create_channel().await?;
        ch.exchange_declare(
            "amq.topic".into(),
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions {
                durable: true,
                ..lapin::options::ExchangeDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await?;

        let durable = opts.durable_queues || opts.queue_type == "quorum";
        let redelivery_queue = prefixed_queue(queue::QUEUE_REDELIVERIES, engine_id);
        ch.queue_declare(
            redelivery_queue.as_str().into(),
            lapin::options::QueueDeclareOptions {
                durable,
                ..lapin::options::QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await?;

        Ok(Self {
            url: url.to_string(),
            conn_pool: Arc::new(vec![Arc::new(conn1), Arc::new(conn2), Arc::new(conn3)]),
            last_conn_idx: Arc::new(AtomicUsize::new(0)),
            management_url: opts.management_url,
            durable_queues: opts.durable_queues,
            queue_type: opts.queue_type,
            consumer_timeout: opts.consumer_timeout,
            shutting_down: Arc::new(RwLock::new(false)),
            declared_queues: Arc::new(RwLock::new(std::collections::HashMap::new())),
            engine_id: engine_id.to_string(),
        })
    }

    #[allow(clippy::unused_async)]
    async fn get_connection(&self) -> Result<Arc<Connection>> {
        let idx = self.last_conn_idx.fetch_add(1, Ordering::SeqCst) % self.conn_pool.len();
        let conn = self
            .conn_pool
            .get(idx)
            .ok_or_else(|| anyhow!("Connection pool index out of bounds"))?;

        if conn.status().connected() {
            return Ok(Arc::clone(conn));
        }

        // If connection is dead, we could try to reconnect, but for high-throughput
        // with 3 connections, we just fail and let the next one handle it,
        // or we could try the other 2 in the pool.
        self.conn_pool
            .iter()
            .find(|c| c.status().connected())
            .map(Arc::clone)
            .ok_or_else(|| anyhow!("All RabbitMQ connections are down"))
    }

    fn queue_args(&self, qname: &str) -> FieldTable {
        let mut args = FieldTable::default();
        if self.queue_type == "quorum" {
            args.insert(
                "x-queue-type".into(),
                lapin::types::AMQPValue::LongString("quorum".into()),
            );
        }
        if super::is_worker_queue(qname) {
            args.insert(
                "x-max-priority".into(),
                lapin::types::AMQPValue::LongLongInt(10),
            );
        }
        if let Some(timeout) = self.consumer_timeout {
            let timeout_ms = i64::try_from(timeout.as_millis()).map_or(30 * 60 * 1000, |v| v);
            args.insert(
                "x-consumer-timeout".into(),
                lapin::types::AMQPValue::LongLongInt(timeout_ms),
            );
        }
        args
    }

    async fn declare_queue(&self, ch: &lapin::Channel, qname: &str) -> Result<()> {
        let mut declared = self.declared_queues.write().await;
        if declared.contains_key(qname) {
            return Ok(());
        }
        let durable = self.durable_queues || self.queue_type == "quorum";
        ch.queue_declare(
            qname.into(),
            QueueDeclareOptions {
                durable,
                ..QueueDeclareOptions::default()
            },
            self.queue_args(qname),
        )
        .await?;
        declared.insert(qname.to_string(), qname.to_string());
        Ok(())
    }

    async fn is_shutting_down(&self) -> bool {
        *self.shutting_down.read().await
    }

    async fn publish_raw(
        &self,
        exchange: &str,
        routing_key: &str,
        data: Vec<u8>,
        msg_type: &str,
        priority: u8,
    ) -> Result<()> {
        let conn = self.get_connection().await?;
        let ch = conn.create_channel().await?;
        let props = BasicProperties::default()
            .with_type(msg_type.into())
            .with_priority(priority);
        ch.basic_publish(
            exchange.into(),
            routing_key.into(),
            BasicPublishOptions::default(),
            &data,
            props,
        )
        .await?
        .await?;
        Ok(())
    }

    async fn subscribe_raw(
        &self,
        qname: &str,
        handler: Arc<dyn Fn(Value) -> BoxedHandlerFuture + Send + Sync>,
    ) -> Result<()> {
        self.subscribe_with_binding("", "", qname, handler).await
    }

    async fn subscribe_with_binding(
        &self,
        exchange: &str,
        routing_key: &str,
        qname: &str,
        handler: Arc<dyn Fn(Value) -> BoxedHandlerFuture + Send + Sync>,
    ) -> Result<()> {
        let conn = self.get_connection().await?;
        let ch = conn.create_channel().await?;
        self.declare_queue(&ch, qname).await?;
        if !exchange.is_empty() {
            ch.queue_bind(
                qname.into(),
                exchange.into(),
                routing_key.into(),
                lapin::options::QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;
        }
        ch.basic_qos(1, BasicQosOptions::default()).await?;
        let mut consumer = ch
            .basic_consume(
                qname.into(),
                "".into(),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        tokio::spawn(async move {
            while let Some(delivery) = futures_util::StreamExt::next(&mut consumer).await {
                if b.is_shutting_down().await {
                    break;
                }
                if let Ok(delivery) = delivery {
                    if delivery.redelivered {
                        let msg_type = delivery
                            .properties
                            .kind()
                            .as_ref()
                            .map_or("", |s| s.as_str());
                        let redelivery_queue =
                            prefixed_queue(queue::QUEUE_REDELIVERIES, &engine_id);
                        let _ = b
                            .publish_raw("", &redelivery_queue, delivery.data.clone(), msg_type, 0)
                            .await;
                        let _ = delivery.ack(BasicAckOptions::default()).await;
                        continue;
                    }
                    if let Ok(val) = serde_json::from_slice(&delivery.data) {
                        let _ = handler(val).await;
                    }
                    let _ = delivery.ack(BasicAckOptions::default()).await;
                }
            }
        });
        Ok(())
    }
}

impl Broker for RabbitMQBroker {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let task = task.clone();
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let data = serde_json::to_vec(&task)?;
            let priority = u8::try_from(task.priority).map_or(0, |v| v);
            let queue = prefixed_queue(&qname, &engine_id);
            b.publish_raw("", &queue, data, MSG_TYPE_TASK, priority)
                .await
        })
    }

    fn publish_tasks(&self, qname: String, tasks: &[Task]) -> BoxedFuture<()> {
        let tasks = tasks.to_vec();
        let b = self.clone();
        let engine_id = self.engine_id.clone();

        Box::pin(async move {
            let queue = prefixed_queue(&qname, &engine_id);
            // Serialize all tasks first (fail fast on serialization errors)
            let serialized: Vec<(Vec<u8>, u8)> = tasks
                .iter()
                .map(|task| {
                    let data = serde_json::to_vec(task)?;
                    let priority = u8::try_from(task.priority).map_or(0, |v| v);
                    Ok((data, priority))
                })
                .collect::<Result<Vec<_>, serde_json::Error>>()
                .map_err(|e| anyhow::anyhow!("serialization failed: {e}"))?;

            // Publish all concurrently via try_join_all for batch-like throughput
            let futures: Vec<_> = serialized
                .into_iter()
                .map(|(data, priority)| b.publish_raw("", &queue, data, MSG_TYPE_TASK, priority))
                .collect();

            // NOTE: Batch publish is non-atomic. RabbitMQ has no transactional batch publish
            // in the AMQP model. If some messages fail to publish, already-published messages
            // remain in the queue. The coordinator's compensating rollback pattern handles this
            // by marking orphaned tasks as FAILED when the publish call returns an error.
            futures_util::future::try_join_all(futures).await?;
            Ok(())
        })
    }

    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let queue = prefixed_queue(&qname, &engine_id);
            b.subscribe_raw(&queue, make_json_handler_arc(handler))
                .await
        })
    }

    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()> {
        let task = task.clone();
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let data = serde_json::to_vec(&task)?;
            let queue = prefixed_queue(queue::QUEUE_PROGRESS, &engine_id);
            b.publish_raw("", &queue, data, MSG_TYPE_TASK, 0).await
        })
    }

    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()> {
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let queue = prefixed_queue(queue::QUEUE_PROGRESS, &engine_id);
            b.subscribe_raw(&queue, make_json_handler(handler)).await
        })
    }

    fn publish_heartbeat(&self, node: Node) -> BoxedFuture<()> {
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let data = serde_json::to_vec(&node)?;
            let queue = prefixed_queue(queue::QUEUE_HEARTBEAT, &engine_id);
            b.publish_raw("", &queue, data, MSG_TYPE_NODE, 0).await
        })
    }

    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()> {
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let queue = prefixed_queue(queue::QUEUE_HEARTBEAT, &engine_id);
            b.subscribe_raw(&queue, make_json_handler(handler)).await
        })
    }

    fn publish_job(&self, job: &Job) -> BoxedFuture<()> {
        let job = job.clone();
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let data = serde_json::to_vec(&job)?;
            let queue = prefixed_queue(queue::QUEUE_JOBS, &engine_id);
            b.publish_raw("", &queue, data, MSG_TYPE_JOB, 0).await
        })
    }

    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()> {
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let queue = prefixed_queue(queue::QUEUE_JOBS, &engine_id);
            b.subscribe_raw(&queue, make_json_handler(handler)).await
        })
    }

    fn publish_event(&self, topic: String, event: Value) -> BoxedFuture<()> {
        let b = self.clone();
        Box::pin(async move {
            let data = serde_json::to_vec(&event)?;
            b.publish_raw("amq.topic", &topic, data, MSG_TYPE_EVENT, 0)
                .await
        })
    }

    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()> {
        let b = self.clone();
        Box::pin(async move {
            let qname = format!(
                "{}.{}",
                queue::QUEUE_EXCLUSIVE_PREFIX,
                twerk_core::uuid::new_short_uuid()
            );
            b.subscribe_with_binding(
                "amq.topic",
                &pattern,
                &qname,
                Arc::new(move |val| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        handler(val).await?;
                        Ok(())
                    })
                }),
            )
            .await
        })
    }

    fn publish_task_log_part(&self, part: &TaskLogPart) -> BoxedFuture<()> {
        let part = part.clone();
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let data = serde_json::to_vec(&part)?;
            let queue = prefixed_queue(queue::QUEUE_TASK_LOG_PART, &engine_id);
            b.publish_raw("", &queue, data, MSG_TYPE_TASK_LOG_PART, 0)
                .await
        })
    }

    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()> {
        let b = self.clone();
        let engine_id = self.engine_id.clone();
        Box::pin(async move {
            let queue = prefixed_queue(queue::QUEUE_TASK_LOG_PART, &engine_id);
            b.subscribe_raw(&queue, make_json_handler(handler)).await
        })
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        let b = self.clone();
        Box::pin(async move {
            if let Some(mgmt_url) = &b.management_url {
                let client = reqwest::Client::new();
                let url = format!("{mgmt_url}/api/queues");
                let res = client
                    .get(&url)
                    .send()
                    .await?
                    .json::<Vec<serde_json::Value>>()
                    .await?;
                let queues = res
                    .into_iter()
                    .map(|q| {
                        let name = q["name"]
                            .as_str()
                            .map_or(String::new(), ToString::to_string);
                        // Use helper functions for idiomatic JSON extraction and type conversion
                        let size = extract_i32(&q["messages"]);
                        let subscribers = extract_i32(&q["consumers"]);
                        let unacked = extract_i32(&q["messages_unacknowledged"]);
                        QueueInfo {
                            name,
                            size,
                            subscribers,
                            unacked,
                        }
                    })
                    .collect();
                Ok(queues)
            } else {
                Ok(Vec::new())
            }
        })
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        let b = self.clone();
        Box::pin(async move {
            if let Some(mgmt_url) = &b.management_url {
                let client = reqwest::Client::new();
                let url = format!("{mgmt_url}/api/queues/%2f/{qname}");
                let q = client
                    .get(&url)
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await?;
                let name = q["name"]
                    .as_str()
                    .map_or(String::new(), ToString::to_string);
                // Use helper functions for idiomatic JSON extraction and type conversion
                let size = extract_i32(&q["messages"]);
                let subscribers = extract_i32(&q["consumers"]);
                let unacked = extract_i32(&q["messages_unacknowledged"]);
                Ok(QueueInfo {
                    name,
                    size,
                    subscribers,
                    unacked,
                })
            } else {
                Ok(QueueInfo {
                    name: qname,
                    size: 0,
                    subscribers: 0,
                    unacked: 0,
                })
            }
        })
    }

    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        let b = self.clone();
        Box::pin(async move {
            let conn = b.get_connection().await?;
            let ch = conn.create_channel().await?;
            ch.queue_delete(qname.as_str().into(), QueueDeleteOptions::default())
                .await?;
            Ok(())
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        let b = self.clone();
        Box::pin(async move {
            let _conn = b.get_connection().await?;
            Ok(())
        })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        let b = self.clone();
        Box::pin(async move {
            let mut sd = b.shutting_down.write().await;
            *sd = true;
            Ok(())
        })
    }
}

impl Clone for RabbitMQBroker {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            conn_pool: Arc::clone(&self.conn_pool),
            last_conn_idx: Arc::clone(&self.last_conn_idx),
            management_url: self.management_url.clone(),
            durable_queues: self.durable_queues,
            queue_type: self.queue_type.clone(),
            consumer_timeout: self.consumer_timeout,
            shutting_down: Arc::clone(&self.shutting_down),
            declared_queues: Arc::clone(&self.declared_queues),
            engine_id: self.engine_id.clone(),
        }
    }
}
