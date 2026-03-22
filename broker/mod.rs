//! Broker module for message queue and pub/sub functionality.
//!
//! This module provides broker implementations for delivering tasks
//! and coordinating between workers and the coordinator.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub mod inmemory;
pub mod log;
pub mod rabbitmq;

// Re-export broker types from tork crate
pub use tork::broker::{
    Broker, BoxedFuture, BoxedHandlerFuture, EventHandler, HeartbeatHandler, JobHandler,
    QueueInfo, TaskHandler, TaskLogPartHandler, TaskProgressHandler,
};

// Re-export queue constants
pub use tork::broker::queue;
pub use tork::broker::{is_coordinator_queue, is_task_queue, is_worker_queue};
