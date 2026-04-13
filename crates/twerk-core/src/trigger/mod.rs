#![allow(unexpected_cfgs)]

//! Trigger system types and the `TriggerRegistry` trait.
//!
//! This module defines the core types for the trigger system:
//! - `TriggerId`: validated trigger identifiers
//! - `TriggerState`: runtime state machine
//! - `TriggerVariant`: type of trigger (Cron, Webhook, Polling)
//! - `Trigger`: a trigger entity
//! - `TriggerContext`: execution context for `fire()`
//! - `TriggerError`: error types
//! - `TriggerIdError`: `TriggerId` validation errors
//! - `TriggerRegistry`: trait for trigger lifecycle management

pub mod in_memory;
pub mod tests;
pub mod r#trait;
pub mod types;

pub use in_memory::is_valid_transition;
pub use in_memory::InMemoryTriggerRegistry;
pub use r#trait::{BoxedTriggerFuture, TriggerRegistry, TriggerRegistryResult};
pub use types::{
    JobId, ParseTriggerStateError, Trigger, TriggerContext, TriggerError, TriggerId,
    TriggerIdError, TriggerState, TriggerVariant,
};
