//! Broker factory module
//!
//! Factory functions and configuration helpers for creating broker implementations.

use std::time::Duration;

use anyhow::{anyhow, Result};
use twerk_infrastructure::broker::{
    inmemory::InMemoryBroker, rabbitmq::RabbitMQBroker, Broker, RabbitMQOptions,
};

use super::engine_helpers::{ensure_config_loaded, env_string, env_string_default};
use twerk_common::constants::{DEFAULT_CONSUMER_TIMEOUT_MS, DEFAULT_RABBITMQ_URL, QUEUE_TYPE_CLASSIC};

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
        match s.to_ascii_lowercase().as_str() {
            "rabbitmq" => Self::RabbitMQ,
            _ => Self::InMemory,
        }
    }
}

// ── Config helpers ─────────────────────────────────────────────

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
            let consumer_timeout = env_duration_ms_default(
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
                    consumer_timeout: Some(consumer_timeout),
                },
            )
            .await
            .map_err(|e| anyhow!("unable to connect to RabbitMQ: {e}"))?;

            Ok(Box::new(broker))
        }
    }
}
