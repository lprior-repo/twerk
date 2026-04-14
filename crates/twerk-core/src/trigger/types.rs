//! Trigger system domain types.
//!
//! This module defines the core types for the trigger system:
//! - `TriggerState`: runtime state machine
//! - `TriggerVariant`: type of trigger (Cron, Webhook, Polling)
//! - `Trigger`: a trigger entity
//! - `TriggerContext`: execution context for `fire()`
//! - `TriggerError`: error types
//!
//! Note: `TriggerId` and `JobId` are re-exported from `crate::id` to ensure
//! single source of truth for validated identifier types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Re-export validated identifiers from id module to maintain API surface
pub use crate::id::{IdError as TriggerIdError, JobId, TriggerId};

// =============================================================================
// TriggerState - Runtime state machine
// =============================================================================

/// The runtime state of a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum TriggerState {
    #[default]
    Active, // Can fire, retains resources
    Paused,   // Cannot fire, retains resources
    Disabled, // Cannot fire, releases all resources
    Error,    // Terminal state for polling failures, requires manual resume
}

impl std::fmt::Display for TriggerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerState::Active => write!(f, "Active"),
            TriggerState::Paused => write!(f, "Paused"),
            TriggerState::Disabled => write!(f, "Disabled"),
            TriggerState::Error => write!(f, "Error"),
        }
    }
}

impl std::str::FromStr for TriggerState {
    type Err = ParseTriggerStateError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Active" => Ok(TriggerState::Active),
            "Paused" => Ok(TriggerState::Paused),
            "Disabled" => Ok(TriggerState::Disabled),
            "Error" => Ok(TriggerState::Error),
            _ => Err(ParseTriggerStateError(s.to_string())),
        }
    }
}

/// Error type for TriggerState parsing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTriggerStateError(pub String);

impl std::fmt::Display for ParseTriggerStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid TriggerState: {}", self.0)
    }
}

impl std::error::Error for ParseTriggerStateError {}

// =============================================================================
// TriggerVariant - Type/kind of trigger
// =============================================================================

/// The type/kind of a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TriggerVariant {
    Cron,    // Cron expression-based scheduling
    Webhook, // HTTP endpoint trigger
    Polling, // Periodic HTTP polling with failure tracking
}

// =============================================================================
// Trigger and TriggerContext - Entity and execution context
// =============================================================================

/// A trigger entity with state and variant information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    pub id: TriggerId,
    pub state: TriggerState,
    pub variant: TriggerVariant,
}

/// Execution context passed to TriggerRegistry::fire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerContext {
    pub trigger_id: TriggerId,
    pub timestamp: time::OffsetDateTime,
    pub event_data: Option<serde_json::Value>,
    pub trigger_type: TriggerVariant,
}

// =============================================================================
// TriggerError - Domain errors
// =============================================================================

/// Errors that can occur during trigger operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
pub enum TriggerError {
    // --- Not Found (1) ---
    #[error("trigger not found: {0}")]
    NotFound(TriggerId),

    // --- Registration (4) ---
    #[error("trigger already registered: {0}")]
    AlreadyExists(TriggerId),
    #[error("invalid cron expression: {0}")]
    InvalidCronExpression(String),
    #[error("invalid interval: {0}")]
    InvalidInterval(String),
    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),

    // --- State Machine (3) ---
    #[error("invalid state transition: {0}")]
    InvalidStateTransition(String),
    #[error("trigger in error state: {0}")]
    TriggerInErrorState(TriggerId),
    #[error("trigger disabled: {0}")]
    TriggerDisabled(TriggerId),

    // --- Webhook (3) ---
    #[error("payload too large: {0} bytes")]
    PayloadTooLarge(usize),
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    // --- Polling (2) ---
    #[error("polling HTTP error: {0}")]
    PollingHttpError(String),
    #[error("polling expression error: {0}")]
    PollingExpressionError(String),

    // --- Runtime (3) ---
    #[error("max consecutive failures: {0}")]
    MaxConsecutiveFailures(usize),
    #[error("job creation failed: {0}")]
    JobCreationFailed(String),
    #[error("job publish failed: {0}")]
    JobPublishFailed(String),

    // --- Infrastructure (3) ---
    #[error("datastore unavailable: {0}")]
    DatastoreUnavailable(String),
    #[error("broker unavailable: {0}")]
    BrokerUnavailable(String),
    #[error("concurrency limit reached")]
    ConcurrencyLimitReached,

    // --- Job ID Generation (1) ---
    #[error("failed to generate job ID: {0}")]
    JobIdGenerationFailed(String),
}

// =============================================================================
// From trait implementations for TriggerError
// =============================================================================

/// Converts `std::io::Error` to `TriggerError::DatastoreUnavailable`.
impl From<std::io::Error> for TriggerError {
    fn from(err: std::io::Error) -> Self {
        TriggerError::DatastoreUnavailable(err.to_string())
    }
}

/// Converts `serde_json::Error` to `TriggerError::PollingExpressionError`.
impl From<serde_json::Error> for TriggerError {
    fn from(err: serde_json::Error) -> Self {
        TriggerError::PollingExpressionError(err.to_string())
    }
}

// NOTE: reqwest::Error conversion is NOT implemented here because reqwest is not
// a dependency of twerk-core (it uses ureq instead). The conversion should be
// implemented in crates that use reqwest (e.g., twerk-app).
//
// NOTE: cron::Error conversion is NOT implemented because the cron crate uses
// a custom error mechanism wrapped by domain_types::CronError. Cron expression
// errors are handled via CronError::InvalidExpression in domain_types.rs.
