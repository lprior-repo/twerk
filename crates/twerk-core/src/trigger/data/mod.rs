//! Trigger DATA types for defining triggers.
//!
//! These types are distinct from the runtime `Trigger`/`TriggerState` types in `types.rs`.
//! This module contains DATA types for constructing and serializing trigger configurations.

pub mod cron_trigger;
pub mod error;
pub mod http_method;
pub mod polling_trigger;
pub mod trigger_enum;
pub mod validation;
pub mod webhook_auth;
pub mod webhook_trigger;

// Re-export domain types that were previously re-exported from this module.
pub use crate::domain::{CronExpression, CronExpressionError, GoDuration, GoDurationError};
pub use crate::id::{IdError, TriggerId};

// Re-export all public items at module root for backward compatibility.
pub use error::TriggerDataError;
pub use http_method::HttpMethod;
pub use webhook_auth::WebhookAuth;
pub use cron_trigger::CronTrigger;
pub use webhook_trigger::WebhookTrigger;
pub use polling_trigger::PollingTrigger;
pub use trigger_enum::Trigger;
