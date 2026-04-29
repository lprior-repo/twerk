//! Broker configuration types.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RabbitMQOptions {
    pub management_url: Option<String>,
    pub durable_queues: bool,
    pub queue_type: String,
    pub consumer_timeout: Option<Duration>,
    pub prefetch_count: u16,
}

impl Default for RabbitMQOptions {
    fn default() -> Self {
        Self {
            management_url: None,
            durable_queues: false,
            queue_type: String::new(),
            consumer_timeout: None,
            prefetch_count: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RabbitMQOptions;

    #[test]
    fn default_prefetch_preserves_single_message_in_flight_semantics() {
        assert_eq!(RabbitMQOptions::default().prefetch_count, 1);
    }
}
