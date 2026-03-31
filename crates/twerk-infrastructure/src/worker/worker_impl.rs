//! Worker implementation module.
//!
//! Provides the core Worker struct that handles:
//! - Task execution via runtime
//! - Queue subscription via broker
//! - Heartbeat management
//!
//! This module is a thin shim that re-exports from the split implementation.

pub use super::internal::types::{Config, Limits, NewWorkerError};
pub use super::internal::worker::Worker;
