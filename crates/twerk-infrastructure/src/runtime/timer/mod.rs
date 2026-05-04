//! Timer scheduler for workflow execution delays.
//!
//! This module provides a `TimerWheel` that tracks pending timers for:
//! - `DelayStep` (relative delay)
//! - Scheduled workflows (cron-like absolute time)
//! - Wait-for timeout (actor waiting for signal with timeout)
//!
//! On timer fire, signals are sent to `SignalRegistry` which wakes waiting actors.
//! Timers may be persisted to `PostgreSQL` for survival across restarts, or kept
//! in-memory for simpler deployments.
//!
//! # Architecture
//!
//! - `TimerWheel`: Main timer manager using `tokio::time` for scheduling
//! - `TimerEntry`: Represents a pending timer with variant-specific data
//! - `SignalRegistry`: Trait for waking waiting actors
//! - `TimerPersistence`: Trait for timer storage (in-memory or `PostgreSQL`)

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

mod entry;
mod persistence;
mod registry;
mod wheel;

pub use entry::{
    DelayTimer, ScheduledTimer, TimerEntry, TimerId, TimerState, TimerVariant, WaitForTimer,
};
pub use persistence::{
    InMemorySignalRegistry, InMemoryTimerPersistence, PostgresTimerPersistence, TimerPersistence,
};
pub use registry::SignalRegistry;
pub use wheel::TimerWheel;
