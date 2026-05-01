//! Twerk Engine - Main orchestration engine for task execution
//!
//! This crate provides the core Engine struct that coordinates
//! between broker, datastore, locker, worker, and coordinator components.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]
#![allow(clippy::pedantic)]

pub mod broker;
pub mod coordinator;
pub mod datastore;
pub mod default;
pub mod endpoints;
#[allow(clippy::module_inception)]
pub mod engine;
pub mod engine_helpers;
pub mod engine_lifecycle;
pub mod engine_registration;
pub mod locker;
pub mod middleware;
pub mod mounts;
pub mod providers;
pub mod signals;
pub mod state;
pub mod types;
pub mod worker;

// Re-export commonly used types
pub use self::engine::Engine;
pub use broker::BrokerProxy;
pub use datastore::DatastoreProxy;
pub use engine_helpers::{resolve_locker_type, MockRuntime};
pub use types::{
    Config, EndpointHandler, JobEventType, JobHandlerError, JobHandlerFunc, JobListener,
    JobMiddlewareFunc, LogHandlerError, LogMiddlewareFunc, Middleware, Mode,
    NodeHandlerError, NodeHandlerFunc, NodeMiddlewareFunc, State, SubmitTaskError, TaskEventType,
    TaskHandlerError, TaskHandlerFunc, TaskHandle, TaskMiddlewareFunc, WebMiddlewareFunc,
};

/// Topic constant for job events
pub const TOPIC_JOB: &str = "job.*";
/// Topic for completed job events
pub const TOPIC_JOB_COMPLETED: &str = "job.completed";
/// Topic for failed job events
pub const TOPIC_JOB_FAILED: &str = "job.failed";
/// Topic for job progress events
pub const TOPIC_JOB_PROGRESS: &str = "job.progress";
