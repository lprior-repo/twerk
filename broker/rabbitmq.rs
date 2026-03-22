//! RabbitMQ broker implementation.
//!
//! Provides a RabbitMQ-based broker implementation using the lapin AMQP client.

use crate::broker::{
    is_coordinator_queue, queue, BoxedFuture, BoxedHandlerFuture, Broker, EventHandler,
    HeartbeatHandler, JobHandler, QueueInfo, TaskHandler, TaskLogPartHandler,
    TaskProgressHandler,
};
use tork::task::Task;
use crate::uuid::new_short_uuid;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

/// Default consumer timeout (30 minutes)
const DEFAULT_CONSUMER_TIMEOUT_MS: i64 = 30 * 60 * 1000;
/// Default heartbeat TTL (60 seconds)
const DEFAULT_HEARTBEAT_TTL_MS: i32 = 60000;
/// Default queue type
const DEFAULT_QUEUE_TYPE: &str = "classic";
/// Maximum priority
const MAX_PRIORITY: u8 = 9;
/// Connection pool size
const POOL_SIZE: usize = 3;
/// Maximum reconnection attempts
const MAX_RECONNECT_ATTEMPTS: usize = 20;

/// Subscription tracking with cancel and done channels
struct Subscription {
    /// Channel to cancel the subscription
    cancel: mpsc::Sender<()>,
    /// Channel that's closed when subscription is done
    done: mpsc::Receiver<()>,
    /// Queue name for this subscription
    qname: String,
    /// Channel for this subscription (to delete exclusive queues)
    channel: lapin::Channel,
    /// Consumer tag for this subscription
    consumer_tag: String,
}

/// RabbitMQ broker implementation.
pub struct RabbitMQBroker {
    url: String,
    heartbeat_ttl: i32,
    consumer_timeout: i32,
    queue_type: String,
    management_url: Option<String>,
    durable: bool,
    /// Connection pool for persistent connections
    conn_pool: Arc<RwLock<Vec<Arc<lapin::Connection>>>>,
    /// Next connection index for round-robin
    next_conn: Arc<AtomicUsize>,
    /// Track if shutting down
    shutting_down: Arc<RwLock<bool>>,
    /// Track declared queues (to re-declare after reconnect)
    declared_queues: Arc<RwLock<HashMap<String, String>>>,
    /// Internal state for tracking subscriptions
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
}

impl Clone for RabbitMQBroker {
    fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            heartbeat_ttl: self.heartbeat_ttl,
            consumer_timeout: self.consumer_timeout,
            queue_type: self.queue_type.clone(),
            management_url: self.management_url.clone(),
            durable: self.durable,
            conn_pool: self.conn_pool.clone(),
            next_conn: self.next_conn.clone(),
            shutting_down: self.shutting_down.clone(),
            declared_queues: self.declared_queues.clone(),
            subscriptions: self.subscriptions.clone(),
        }
    }
}

impl RabbitMQBroker {
    /// Creates a new RabbitMQ broker configuration.
    pub async fn new(url: &str) -> Result<Self, anyhow::Error> {
        let broker = Self {
            url: url.to_string(),
            heartbeat_ttl: DEFAULT_HEARTBEAT_TTL_MS,
            consumer_timeout: DEFAULT_CONSUMER_TIMEOUT_MS as i32,
            queue_type: DEFAULT_QUEUE_TYPE.to_string(),
            management_url: None,
            durable: false,
            conn_pool: Arc::new(RwLock::new(Vec::with_capacity(POOL_SIZE))),
            next_conn: Arc::new(AtomicUsize::new(0)),
            shutting_down: Arc::new(RwLock::new(false)),
            declared_queues: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        };

        // Initialize connection pool
        broker.init_pool().await?;
        Ok(broker)
    }

    /// Initialize the connection pool.
    async fn init_pool(&self) -> Result<(), anyhow::Error> {
        let mut pool = self.conn_pool.write().await;
        for _ in 0..POOL_SIZE {
            let conn = self.connect().await?;
            pool.push(Arc::new(conn));
        }
        Ok(())
    }

    /// Create a new connection to RabbitMQ.
    async fn connect(&self) -> Result<lapin::Connection, anyhow::Error> {
        lapin::Connection::connect(&self.url, lapin::ConnectionProperties::default())
            .await
            .map_err(|e| anyhow::anyhow!("error dialing to RabbitMQ: {}", e))
    }

    /// Get a connection from the pool using round-robin.
    async fn get_connection(&self) -> Result<Arc<lapin::Connection>, anyhow::Error> {
        let pool = self.conn_pool.read().await;
        if pool.is_empty() {
            return Err(anyhow::anyhow!("connection pool is empty"));
        }

        let idx = self.next_conn.fetch_add(1, Ordering::SeqCst) % pool.len();
        let conn = pool
            .get(idx)
            .ok_or_else(|| anyhow::anyhow!("connection pool index out of bounds"))?;

        // Check if connection is closed
        if conn.status().connected() {
            Ok(conn.clone())
        } else {
            drop(pool);
            // Connection is closed, need to reconnect
            self.reconnect(idx).await
        }
    }

    /// Reconnect a specific connection in the pool.
    async fn reconnect(&self, idx: usize) -> Result<Arc<lapin::Connection>, anyhow::Error> {
        tracing::warn!("connection is closed. reconnecting to RabbitMQ");

        // Clear declared queues since broker might have crashed
        {
            let mut queues = self.declared_queues.write().await;
            queues.clear();
        }

        let new_conn = self.connect().await?;
        let new_conn_arc = Arc::new(new_conn);

        // Update pool with new connection
        let mut pool = self.conn_pool.write().await;
        if idx < pool.len() {
            pool[idx] = new_conn_arc.clone();
        }

        Ok(new_conn_arc)
    }

    /// Sets the heartbeat TTL in milliseconds.
    pub fn with_heartbeat_ttl(mut self, ttl: i32) -> Self {
        Self {
            heartbeat_ttl: ttl,
            ..self
        }
    }

    /// Sets the consumer timeout in milliseconds.
    pub fn with_consumer_timeout_ms(mut self, timeout: i64) -> Self {
        self.consumer_timeout = timeout as i32;
        self
    }

    /// Sets the queue type (classic or quorum).
    pub fn with_queue_type(mut self, qtype: &str) -> Self {
        self.queue_type = qtype.to_string();
        self
    }

    /// Sets the management URL for the RabbitMQ management API.
    pub fn with_management_url(mut self, url: &str) -> Self {
        self.management_url = Some(url.to_string());
        self
    }

    /// Sets whether queues should be durable.
    pub fn with_durable_queues(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Check if broker is shutting down.
    async fn is_shutting_down(&self) -> bool {
        *self.shutting_down.read().await
    }

    /// Declare a queue on the given channel.
    async fn declare_queue(
        &self,
        channel: &lapin::Channel,
        qname: &str,
    ) -> Result<(), anyhow::Error> {
        // Check if already declared
        {
            let queues = self.declared_queues.read().await;
            if queues.contains_key(qname) {
                return Ok(());
            }
        }

        let mut args = lapin::types::FieldTable::default();
        if qname == queue::QUEUE_HEARTBEAT {
            args.insert("x-message-ttl".into(), self.heartbeat_ttl.into());
        }
        if crate::broker::is_task_queue(qname) && self.queue_type == DEFAULT_QUEUE_TYPE {
            args.insert("x-max-priority".into(), MAX_PRIORITY.into());
        }
        if !qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX) {
            // Note: x-queue-type insertion skipped - lapin API changed in 2.x
            // The queue_type is set via QueueDeclareOptions if needed
        }
        args.insert("x-consumer-timeout".into(), self.consumer_timeout.into());

        let queue_durable = self.durable || self.queue_type == "quorum";

        channel
            .queue_declare(
                qname,
                lapin::options::QueueDeclareOptions {
                    durable: queue_durable,
                    passive: false,
                    exclusive: qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX),
                    auto_delete: false,
                    nowait: false,
                },
                args,
            )
            .await
            .map_err(|e| anyhow::anyhow!("error declaring queue {}: {}", qname, e))?;

        // Mark as declared
        {
            let mut queues = self.declared_queues.write().await;
            queues.insert(qname.to_string(), qname.to_string());
        }

        Ok(())
    }

    /// Gets the RabbitMQ queue info via management API.
    async fn get_queues(&self) -> Result<Vec<RabbitQueueInfo>, anyhow::Error> {
        #[derive(serde::Deserialize)]
        struct ApiResponse {
            name: String,
            messages: i64,
            consumers: i64,
            messages_unacknowledged: i64,
        }

        let url = if let Some(ref mgmt_url) = self.management_url {
            format!("{}/api/queues/", mgmt_url)
        } else {
            // Parse host from AMQP URL
            let parsed = url::Url::parse(&self.url)?;
            format!(
                "http://{}:15672/api/queues/",
                parsed.host_str().unwrap_or("localhost")
            )
        };

        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await?
            .json::<Vec<ApiResponse>>()
            .await?;

        Ok(resp
            .into_iter()
            .map(|r| RabbitQueueInfo {
                name: r.name,
                messages: r.messages,
                consumers: r.consumers,
                messages_unacknowledged: r.messages_unacknowledged,
            })
            .collect())
    }

    /// Publish a message to a queue.
    async fn publish(
        &self,
        exchange: &str,
        routing_key: &str,
        msg: &(impl serde::Serialize + Send + 'static),
    ) -> Result<(), anyhow::Error> {
        let conn = self.get_connection().await?;
        let channel = conn
            .create_channel()
            .await
            .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;

        // Serialize message
        let body = serde_json::to_vec(msg)?;

        // Determine priority for tasks
        let priority = if let Some(task) = msg.downcast_ref::<Task>() {
            task.priority.min(MAX_PRIORITY as i64) as u8
        } else {
            0
        };

        let props = lapin::BasicProperties::default()
            .with_content_type("application/json".into())
            .with_priority(priority);

        channel
            .basic_publish(
                exchange,
                routing_key,
                lapin::options::BasicPublishOptions::default(),
                &body,
                props,
            )
            .await?
            .await
            .map_err(|e| anyhow::anyhow!("error publishing message: {}", e))?;

        Ok(())
    }

    /// Subscribe to a queue with exponential backoff retry.
    async fn subscribe(
        &self,
        exchange: &str,
        routing_key: &str,
        qname: String,
        handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync>,
    ) -> Result<(), anyhow::Error> {
        let mut attempt = 0usize;

        loop {
            // Check if shutting down
            if self.is_shutting_down().await {
                return Ok(());
            }

            match self.try_subscribe(&qname, handler.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    if attempt > MAX_RECONNECT_ATTEMPTS {
                        tracing::error!(
                            "failed to subscribe to {} after {} attempts: {}",
                            qname,
                            MAX_RECONNECT_ATTEMPTS,
                            e
                        );
                        return Err(e);
                    }

                    // Exponential backoff: 1s, 2s, 4s, 8s, ...
                    let delay = Duration::from_secs(2u64.pow(attempt as u32 - 1).min(64));
                    tracing::info!(
                        "error subscribing to {} (attempt {}/{}): {}. retrying in {:?}",
                        qname,
                        attempt,
                        MAX_RECONNECT_ATTEMPTS,
                        e,
                        delay
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Try to subscribe to a queue once.
    async fn try_subscribe(
        &self,
        qname: &str,
        handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync>,
    ) -> Result<(), anyhow::Error> {
        let conn = self.get_connection().await?;
        let channel = conn
            .create_channel()
            .await
            .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;

        // Declare queue
        self.declare_queue(&channel, qname).await?;

        channel
            .basic_qos(1, lapin::options::BasicQosOptions::default())
            .await
            .map_err(|e| anyhow::anyhow!("error setting qos: {}", e))?;

        let consumer_tag = new_short_uuid();
        let mut consumer = channel
            .basic_consume(
                qname,
                &consumer_tag,
                lapin::options::BasicConsumeOptions {
                    no_ack: false,
                    exclusive: false,
                    nowait: false,
                    ..Default::default()
                },
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("error starting consumer: {}", e))?;

        tracing::debug!("created consumer {} for queue: {}", consumer_tag, qname);

        let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);
        let (done_tx, done_rx) = mpsc::channel::<()>(1);
        let sub_id = new_short_uuid();
        let subscription = Subscription {
            cancel: cancel_tx,
            done: done_rx,
            qname: qname.to_string(),
            channel: channel.clone(),
            consumer_tag: consumer_tag.clone(),
        };
        self.subscriptions.write().await.insert(sub_id, subscription);

        let qname_for_handler = qname.to_string();
        let broker_for_handler = self.clone();
        let channel_for_cleanup = channel.clone();

        tokio::spawn(async move {
            let mut consumer = consumer;
            loop {
                tokio::select! {
                    delivery = consumer.next() => {
                        match delivery {
                            Some(Ok(delivery)) => {
                                Self::handle_delivery(
                                    &broker_for_handler,
                                    &delivery,
                                    &handler,
                                    &qname_for_handler,
                                ).await;
                            }
                            Some(Err(e)) => {
                                tracing::error!("consumer error on queue {}: {}", qname_for_handler, e);
                            }
                            None => {
                                tracing::debug!("consumer channel closed for queue: {}", qname_for_handler);
                                // Signal done
                                let _ = done_tx.send(()).await;
                                return;
                            }
                        }
                    }
                    _ = cancel_rx.recv() => {
                        tracing::debug!("subscription for queue {} canceled", qname_for_handler);
                        // Cancel the consumer
                        if let Err(e) = channel_for_cleanup.basic_cancel(&consumer_tag, lapin::options::BasicCancelOptions { nowait: false }).await {
                            tracing::error!("error canceling consumer for queue {}: {}", qname_for_handler, e);
                        }
                        // Close the channel
                        if let Err(e) = channel_for_cleanup.close(0, "subscription cancelled").await {
                            tracing::error!("error closing channel for queue {}: {}", qname_for_handler, e);
                        }
                        // Signal done
                        let _ = done_tx.send(()).await;
                        return;
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle an incoming delivery.
    async fn handle_delivery(
        broker: &RabbitMQBroker,
        delivery: &lapin::message::Delivery,
        handler: &Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync>,
        qname: &str,
    ) {
        let data = &delivery.data;

        // Check if message is redelivered - send to redeliveries queue
        if delivery.redelivered {
            tracing::debug!(
                "message redelivered on queue {}, sending to redeliveries",
                qname
            );
            // Re-queue to redeliveries queue for later processing
            if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(data) {
                if let Err(e) = broker.publish_redelivery(&msg).await {
                    tracing::error!("failed to publish redelivery: {}", e);
                }
            }
            // Ack the original message
            if let Err(e) = delivery
                .ack(lapin::options::BasicAckOptions::default())
                .await
            {
                tracing::error!("failed to ack redelivered message: {}", e);
            }
            return;
        }

        // Normal message handling
        match serde_json::from_slice::<serde_json::Value>(data) {
            Ok(msg) => {
                handler(msg).await;
                if let Err(e) = delivery
                    .ack(lapin::options::BasicAckOptions::default())
                    .await
                {
                    tracing::error!("failed to ack message: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("failed to deserialize message on queue {}: {}", qname, e);
                // Reject the message without requeue
                if let Err(e) = delivery
                    .reject(lapin::options::BasicRejectOptions { requeue: false })
                    .await
                {
                    tracing::error!("failed to reject message: {}", e);
                }
            }
        }
    }

    /// Publish a redelivery message to the redeliveries queue.
    async fn publish_redelivery(&self, msg: &serde_json::Value) -> Result<(), anyhow::Error> {
        self.publish("", queue::QUEUE_REDELIVERIES, msg).await
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RabbitQueueInfo {
    name: String,
    messages: i64,
    consumers: i64,
    messages_unacknowledged: i64,
}

impl Broker for RabbitMQBroker {
    fn publish_task(&self, qname: String, task: &Task) -> BoxedFuture<()> {
        let task = task.deep_clone();
        Box::pin(async move {
            // Ensure queue exists
            let conn = self.get_connection().await?;
            let channel = conn
                .create_channel()
                .await
                .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;
            self.declare_queue(&channel, &qname).await?;

            // Publish task
            let body = serde_json::to_vec(&task)?;
            let priority = task.priority.min(MAX_PRIORITY as i64) as u8;

            let props = lapin::BasicProperties::default()
                .with_content_type("application/json".into())
                .with_priority(priority);

            channel
                .basic_publish(
                    "",
                    &qname,
                    lapin::options::BasicPublishOptions::default(),
                    &body,
                    props,
                )
                .await?
                .await
                .map_err(|e| anyhow::anyhow!("error publishing task: {}", e))?;

            Ok(())
        })
    }

    fn subscribe_for_tasks(&self, qname: String, handler: TaskHandler) -> BoxedFuture<()> {
        let subscriptions = self.subscriptions.clone();
        Box::pin(async move {
            let handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync> =
                Arc::new(move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Ok(task) = serde_json::from_value::<Arc<Task>>(msg) {
                            handler(task).await;
                        }
                    })
                });

            self.subscribe("", "", qname, handler).await
        })
    }

    fn publish_task_progress(&self, task: &Task) -> BoxedFuture<()> {
        let task = task.deep_clone();
        Box::pin(async move {
            let conn = self.get_connection().await?;
            let channel = conn
                .create_channel()
                .await
                .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;
            self.declare_queue(&channel, queue::QUEUE_PROGRESS).await?;

            let body = serde_json::to_vec(&task)?;
            let props =
                lapin::BasicProperties::default().with_content_type("application/json".into());

            channel
                .basic_publish(
                    "",
                    queue::QUEUE_PROGRESS,
                    lapin::options::BasicPublishOptions::default(),
                    &body,
                    props,
                )
                .await?
                .await
                .map_err(|e| anyhow::anyhow!("error publishing progress: {}", e))?;

            Ok(())
        })
    }

    fn subscribe_for_task_progress(&self, handler: TaskProgressHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            let handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync> =
                Arc::new(move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Ok(task) = serde_json::from_value::<Task>(msg) {
                            handler(task).await;
                        }
                    })
                });

            self.subscribe("", "", queue::QUEUE_PROGRESS.to_string(), handler)
                .await
        })
    }

    fn publish_heartbeat(&self, node: tork::node::Node) -> BoxedFuture<()> {
        let node = node.deep_clone();
        Box::pin(async move {
            let conn = self.get_connection().await?;
            let channel = conn
                .create_channel()
                .await
                .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;
            self.declare_queue(&channel, queue::QUEUE_HEARTBEAT).await?;

            let body = serde_json::to_vec(&node)?;
            let props =
                lapin::BasicProperties::default().with_content_type("application/json".into());

            channel
                .basic_publish(
                    "",
                    queue::QUEUE_HEARTBEAT,
                    lapin::options::BasicPublishOptions::default(),
                    &body,
                    props,
                )
                .await?
                .await
                .map_err(|e| anyhow::anyhow!("error publishing heartbeat: {}", e))?;

            Ok(())
        })
    }

    fn subscribe_for_heartbeats(&self, handler: HeartbeatHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            let handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync> =
                Arc::new(move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Ok(node) = serde_json::from_value::<tork::node::Node>(msg) {
                            handler(node).await;
                        }
                    })
                });

            self.subscribe("", "", queue::QUEUE_HEARTBEAT.to_string(), handler)
                .await
        })
    }

    fn publish_job(&self, job: &tork::job::Job) -> BoxedFuture<()> {
        let job = job.deep_clone();
        Box::pin(async move {
            let conn = self.get_connection().await?;
            let channel = conn
                .create_channel()
                .await
                .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;
            self.declare_queue(&channel, queue::QUEUE_JOBS).await?;

            let body = serde_json::to_vec(&job)?;
            let props =
                lapin::BasicProperties::default().with_content_type("application/json".into());

            channel
                .basic_publish(
                    "",
                    queue::QUEUE_JOBS,
                    lapin::options::BasicPublishOptions::default(),
                    &body,
                    props,
                )
                .await?
                .await
                .map_err(|e| anyhow::anyhow!("error publishing job: {}", e))?;

            Ok(())
        })
    }

    fn subscribe_for_jobs(&self, handler: JobHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            let handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync> =
                Arc::new(move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Ok(job) = serde_json::from_value::<tork::job::Job>(msg) {
                            handler(job).await;
                        }
                    })
                });

            self.subscribe("", "", queue::QUEUE_JOBS.to_string(), handler)
                .await
        })
    }

    fn publish_event(&self, topic: String, event: serde_json::Value) -> BoxedFuture<()> {
        let event = event.clone();
        Box::pin(async move {
            let conn = self.get_connection().await?;
            let channel = conn
                .create_channel()
                .await
                .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;

            let body = serde_json::to_vec(&event)?;
            let props =
                lapin::BasicProperties::default().with_content_type("application/json".into());

            channel
                .basic_publish(
                    "amq.topic",
                    &topic,
                    lapin::options::BasicPublishOptions::default(),
                    &body,
                    props,
                )
                .await?
                .await
                .map_err(|e| anyhow::anyhow!("error publishing event: {}", e))?;

            Ok(())
        })
    }

    fn subscribe_for_events(&self, pattern: String, handler: EventHandler) -> BoxedFuture<()> {
        let key = pattern.replace('*', "#");
        let qname = format!("{}{}", queue::QUEUE_EXCLUSIVE_PREFIX, new_short_uuid());
        Box::pin(async move {
            let handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync> =
                Arc::new(move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        handler(msg).await;
                    })
                });

            // Subscribe with topic exchange
            self.subscribe("amq.topic", &key, qname, handler).await
        })
    }

    fn publish_task_log_part(&self, part: &tork::task::TaskLogPart) -> BoxedFuture<()> {
        let part = part.clone();
        Box::pin(async move {
            let conn = self.get_connection().await?;
            let channel = conn
                .create_channel()
                .await
                .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;
            self.declare_queue(&channel, queue::QUEUE_LOGS).await?;

            let body = serde_json::to_vec(&part)?;
            let props =
                lapin::BasicProperties::default().with_content_type("application/json".into());

            channel
                .basic_publish(
                    "",
                    queue::QUEUE_LOGS,
                    lapin::options::BasicPublishOptions::default(),
                    &body,
                    props,
                )
                .await?
                .await
                .map_err(|e| anyhow::anyhow!("error publishing log part: {}", e))?;

            Ok(())
        })
    }

    fn subscribe_for_task_log_part(&self, handler: TaskLogPartHandler) -> BoxedFuture<()> {
        Box::pin(async move {
            let handler: Arc<dyn Fn(serde_json::Value) -> BoxedHandlerFuture + Send + Sync> =
                Arc::new(move |msg| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        if let Ok(part) = serde_json::from_value::<tork::task::TaskLogPart>(msg) {
                            handler(part).await;
                        }
                    })
                });

            self.subscribe("", "", queue::QUEUE_LOGS.to_string(), handler)
                .await
        })
    }

    fn queues(&self) -> BoxedFuture<Vec<QueueInfo>> {
        let broker = self.clone();
        Box::pin(async move {
            let rqs = broker.get_queues().await?;
            Ok(rqs
                .into_iter()
                .map(|rq| QueueInfo {
                    name: rq.name,
                    size: rq.messages,
                    subscribers: rq.consumers,
                    unacked: rq.messages_unacknowledged,
                })
                .collect())
        })
    }

    fn queue_info(&self, qname: String) -> BoxedFuture<QueueInfo> {
        let broker = self.clone();
        Box::pin(async move {
            let rqs = broker.get_queues().await?;
            rqs.into_iter()
                .find(|rq| rq.name == qname)
                .map(|rq| QueueInfo {
                    name: rq.name,
                    size: rq.messages,
                    subscribers: rq.consumers,
                    unacked: rq.messages_unacknowledged,
                })
                .ok_or_else(|| anyhow::anyhow!("queue {} not found", qname))
        })
    }

    fn delete_queue(&self, qname: String) -> BoxedFuture<()> {
        Box::pin(async move {
            let conn = self.get_connection().await?;
            let channel = conn
                .create_channel()
                .await
                .map_err(|e| anyhow::anyhow!("error creating channel: {}", e))?;

            channel
                .queue_delete(
                    &qname,
                    lapin::options::QueueDeleteOptions {
                        if_empty: false,
                        if_unused: false,
                        nowait: false,
                    },
                )
                .await
                .map_err(|e| anyhow::anyhow!("error deleting queue: {}", e))?;

            // Remove from declared queues
            {
                let mut queues = self.declared_queues.write().await;
                queues.remove(&qname);
            }

            Ok(())
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async move {
            let _conn = self.get_connection().await?;
            Ok(())
        })
    }

    fn shutdown(&self) -> BoxedFuture<()> {
        let subscriptions = self.subscriptions.clone();
        let conn_pool = self.conn_pool.clone();
        Box::pin(async move {
            // Check if already shutting down
            {
                let mut sd = self.shutting_down.write().await;
                if *sd {
                    return Ok(());
                }
                *sd = true;
            }

            // Collect coordinator subscriptions and send cancel
            let subs_to_wait: Vec<_> = {
                let subs = subscriptions.read().await;
                let mut to_wait = Vec::new();
                for (id, sub) in subs.iter() {
                    // Only cancel coordinator queues (not exclusive worker queues)
                    if !is_coordinator_queue(&sub.qname) {
                        continue;
                    }
                    let _ = sub.cancel.send(()).await;
                    to_wait.push((id.clone(), sub.done));
                }
                to_wait
            };

            // Wait for subscriptions to terminate (grace period)
            for (id, mut done) in subs_to_wait {
                tracing::debug!("waiting for subscription {} to terminate", id);
                tokio::select! {
                    _ = done.recv() => {
                        tracing::debug!("subscription {} terminated", id);
                    }
                }
            }

            // Delete exclusive queues
            {
                let subs = subscriptions.read().await;
                for sub in subs.values() {
                    if sub.qname.starts_with(queue::QUEUE_EXCLUSIVE_PREFIX) {
                        tracing::debug!("deleting exclusive queue: {}", sub.qname);
                        if let Err(e) = sub.channel.queue_delete(
                            &sub.qname,
                            lapin::options::QueueDeleteOptions {
                                if_empty: false,
                                if_unused: false,
                                nowait: false,
                            },
                        ).await {
                            tracing::error!("error deleting queue {}: {}", sub.qname, e);
                        }
                    }
                }
            }

            // Clear subscriptions
            {
                let mut subs = subscriptions.write().await;
                subs.clear();
            }

            // Close all connections
            let pool = conn_pool.read().await;
            for conn in pool.iter() {
                tracing::debug!("shutting down connection");
                let conn = conn.clone();
                tokio::spawn(async move {
                    if let Err(e) = conn.close(0, "shutdown").await {
                        tracing::error!("error closing connection: {}", e);
                    }
                });
            }

            // Wait a bit for connections to close
            tokio::time::sleep(Duration::from_millis(100)).await;

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: RabbitMQ tests require a running RabbitMQ instance
    // These are integration tests that would typically be skipped in CI

    #[tokio::test]
    #[ignore]
    async fn test_rabbitmq_connect() {
        let broker = RabbitMQBroker::new("amqp://guest:guest@localhost:5672/")
            .await
            .expect("failed to create broker");
        broker.health_check().await.expect("health check failed");
    }
}
