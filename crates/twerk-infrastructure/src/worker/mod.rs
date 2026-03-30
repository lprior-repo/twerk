//! Worker module for task execution.
//!
//! This module provides the worker implementation that:
//! - Subscribes to task queues via broker
//! - Executes tasks using the runtime (docker/podman/shell)
//! - Reports progress back to broker
//! - Handles heartbeats with coordinator

mod api;
mod worker_impl;

pub use api::{new_api as new_worker_api, WorkerApi};
pub use worker_impl::{Config, Limits, NewWorkerError, Worker};
