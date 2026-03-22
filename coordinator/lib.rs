//! Coordinator module for Tork task queue system
//!
//! This module is responsible for:
//! - Accepting tasks from clients via HTTP API
//! - Scheduling tasks for workers to execute
//! - Exposing cluster state to the outside world
//!
//! # Architecture
//!
//! - **Data**: Domain types - pure data structs (Job, Task, Node, etc.)
//! - **Calc**: Pure calculation functions - state transitions, scheduling logic
//! - **Actions**: I/O operations at the boundary (HTTP handlers, broker operations)
//!
//! # Modules
//!
//! - [`coordinator`]: The coordinator "brain" — wires all handlers with broker subscriptions
//! - [`api`]: HTTP API server and context
//! - [`handlers`]: Event handlers for task and job state changes
//! - [`scheduler`]: Task scheduling logic

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

pub mod api;
pub mod coordinator;
pub mod handlers;
pub mod scheduler;