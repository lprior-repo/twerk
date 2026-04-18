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
use utoipa::ToSchema;

// Re-export validated identifiers from id module to maintain API surface
pub use crate::id::{IdError as TriggerIdError, JobId, TriggerId};

// =============================================================================
// TriggerState - Runtime state machine
// =============================================================================

/// The runtime state of a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, ToSchema)]
pub enum TriggerState {
    #[default]
    Active, // Can fire, retains resources
    Paused,   // Cannot fire, retains resources
    Disabled, // Cannot fire, releases all resources
    Error,    // Terminal state for polling failures, requires manual resume
}

/// Case-insensitive string matching for FromStr (accepts all case variants).
fn match_state_variant_for_fromstr(s: &str) -> Option<TriggerState> {
    match s.to_uppercase().as_str() {
        "ACTIVE" => Some(TriggerState::Active),
        "PAUSED" => Some(TriggerState::Paused),
        "DISABLED" => Some(TriggerState::Disabled),
        "ERROR" => Some(TriggerState::Error),
        _ => None,
    }
}

/// Serde-friendly matching: accepts SCREAMING_SNAKE_CASE only (serde convention).
/// Serialization outputs "Active" (PascalCase) but deserialization accepts only "ACTIVE".
fn match_state_variant_for_serde(s: &str) -> Option<TriggerState> {
    match s {
        "ACTIVE" => Some(TriggerState::Active),
        "PAUSED" => Some(TriggerState::Paused),
        "DISABLED" => Some(TriggerState::Disabled),
        "ERROR" => Some(TriggerState::Error),
        _ => None,
    }
}

impl std::fmt::Display for TriggerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerState::Active => write!(f, "ACTIVE"),
            TriggerState::Paused => write!(f, "PAUSED"),
            TriggerState::Disabled => write!(f, "DISABLED"),
            TriggerState::Error => write!(f, "ERROR"),
        }
    }
}

impl std::str::FromStr for TriggerState {
    type Err = ParseTriggerStateError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match_state_variant_for_fromstr(s).ok_or_else(|| ParseTriggerStateError(s.to_string()))
    }
}

/// Custom serializer for TriggerState - outputs SCREAMING_SNAKE_CASE.
impl Serialize for TriggerState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            TriggerState::Active => "ACTIVE",
            TriggerState::Paused => "PAUSED",
            TriggerState::Disabled => "DISABLED",
            TriggerState::Error => "ERROR",
        };
        serializer.serialize_str(s)
    }
}

/// Custom deserializer for TriggerState - accepts uppercase variants only.
impl<'de> Deserialize<'de> for TriggerState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TriggerStateVisitor;

        impl<'de> serde::de::Visitor<'de> for TriggerStateVisitor {
            type Value = TriggerState;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a TriggerState string (ACTIVE, PAUSED, DISABLED, ERROR)")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match_state_variant_for_serde(value).ok_or_else(|| {
                    serde::de::Error::custom(format!("unknown TriggerState: {value}"))
                })
            }
        }

        deserializer.deserialize_str(TriggerStateVisitor)
    }
}

/// Error type for TriggerState parsing failures.
#[derive(Debug, Clone, PartialEq, Eq, utoipa::ToSchema)]
pub struct ParseTriggerStateError(pub String);

impl std::fmt::Display for ParseTriggerStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown TriggerState: {}", self.0)
    }
}

impl std::error::Error for ParseTriggerStateError {}

// =============================================================================
// TriggerVariant - Type/kind of trigger
// =============================================================================

/// The type/kind of a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    pub id: TriggerId,
    pub state: TriggerState,
    pub variant: TriggerVariant,
}

/// Execution context passed to TriggerRegistry::fire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
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

    // --- State Machine (5) ---
    #[error("invalid state transition: {0} -> {1}")]
    InvalidStateTransition(TriggerState, TriggerState),
    #[error("trigger in error state: {0}")]
    TriggerInErrorState(TriggerId),
    #[error("trigger disabled: {0}")]
    TriggerDisabled(TriggerId),
    #[error("trigger not active: {0}")]
    TriggerNotActive(TriggerState),
    #[error("invalid trigger configuration: {0}")]
    InvalidConfiguration(String),

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
