//! Broker configuration types.

use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct RabbitMQOptions {
    pub management_url: Option<String>,
    pub durable_queues: bool,
    pub queue_type: String,
    pub consumer_timeout: Option<Duration>,
}
