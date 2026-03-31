//! Worker types module.
//!
//! Core data types for worker configuration and state.

use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;
use tokio::sync::broadcast;

use crate::broker::Broker;
use crate::runtime::Runtime as RuntimeTrait;

/// Worker configuration
#[derive(Clone)]
pub struct Config {
    /// Worker name
    pub name: String,
    /// API address (empty for dynamic)
    pub address: String,
    /// Broker for task queue
    pub broker: Arc<dyn Broker>,
    /// Runtime for task execution
    pub runtime: Arc<dyn RuntimeTrait>,
    /// Queue subscriptions (queue name -> concurrency)
    pub queues: HashMap<String, i32>,
    /// Default resource limits
    pub limits: Limits,
}

/// Default resource limits for tasks
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// Default CPU limit (e.g., "1", "2")
    pub default_cpus_limit: String,
    /// Default memory limit (e.g., "512m", "1g")
    pub default_memory_limit: String,
    /// Default timeout duration (e.g., "5m", "1h")
    pub default_timeout: String,
}

/// Errors that can occur during worker operations
#[derive(Debug, Error)]
pub enum NewWorkerError {
    #[error("no queues configured")]
    NoQueuesConfigured,

    #[error("broker is required")]
    BrokerRequired,

    #[error("runtime is required")]
    RuntimeRequired,
}

/// Running task tracking
#[derive(Debug, Clone)]
pub struct RunningTask {
    /// Cancellation sender for this task
    pub cancel_tx: broadcast::Sender<()>,
}
