//! Worker module for task execution.
//!
//! This module provides the core worker functionality for executing tasks
//! from a broker queue.
//!
//! # Architecture
//!
//! - **Data**: `Worker`, `Config`, `Limits` - pure data structs
//! - **Calc`: Pure functions for task handling and middleware application
//! - **Actions**: I/O operations at the boundary (broker, runtime, HTTP API)
//!
//! # Example
//!
//! ```ignore
//! use tork::worker::{Worker, Config, Limits};
//! use tork::broker::InMemoryBroker;
//! use tork::runtime::ShellRuntime;
//!
//! let broker = InMemoryBroker::new();
//! let runtime = ShellRuntime::new();
//! let config = Config {
//!     name: "worker-1".to_string(),
//!     broker: Arc::new(broker),
//!     runtime: Arc::new(runtime),
//!     queues: Default::default(),
//!     limits: Limits::default(),
//!     middleware: vec![],
//! };
//! let worker = Worker::new(config)?;
//! ```

pub mod api;
pub mod worker;

pub use worker::{Config, Limits, RunningTask, Worker, WorkerError};
