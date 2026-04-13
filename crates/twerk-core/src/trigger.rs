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

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;

/// A validated trigger identifier.
///
/// # Validation Rules
/// - Length: 3-64 characters
/// - Allowed characters: alphanumeric, hyphen (`-`), underscore (`_`)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerId(pub String);

impl TriggerId {
    /// Creates a new TriggerId after validating the input string.
    ///
    /// # Preconditions
    /// - Length MUST be between 3 and 64 characters (inclusive)
    /// - All characters MUST be alphanumeric, hyphen, or underscore
    ///
    /// # Postconditions
    /// - Returns `Ok(TriggerId)` if preconditions satisfied
    /// - Returns `Err(TriggerIdError::LengthOutOfRange(n))` if length violation
    /// - Returns `Err(TriggerIdError::InvalidCharacter(c))` if character violation
    pub fn new(s: &str) -> Result<Self, TriggerIdError> {
        // Check length first
        let len = s.len();
        if len < 3 || len > 64 {
            return Err(TriggerIdError::LengthOutOfRange(len));
        }

        // Check characters - find first invalid character
        // Only ASCII alphanumeric, hyphen, and underscore are allowed
        s.chars()
            .find(|&c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
            .map_or(Ok(Self(s.to_string())), |c| {
                Err(TriggerIdError::InvalidCharacter(c))
            })
    }

    /// Returns the underlying string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur during TriggerId validation.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TriggerIdError {
    #[error("trigger ID length out of range: {0} (must be 3-64 characters)")]
    LengthOutOfRange(usize),

    #[error("trigger ID contains invalid character: {0}")]
    InvalidCharacter(char),
}

/// The runtime state of a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TriggerState {
    Active,   // Can fire, retains resources
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

impl Default for TriggerState {
    fn default() -> Self {
        TriggerState::Active
    }
}

/// The type/kind of a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TriggerVariant {
    Cron,    // Cron expression-based scheduling
    Webhook, // HTTP endpoint trigger
    Polling, // Periodic HTTP polling with failure tracking
}

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

/// Errors that can occur during trigger operations.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TriggerError {
    // --- Not Found Errors ---
    #[error("trigger not found: {0}")]
    NotFound(TriggerId),

    // --- Already Exists Errors ---
    #[error("trigger already registered: {0}")]
    AlreadyExists(TriggerId),

    // --- State Transition Errors ---
    #[error("invalid state transition: cannot transition from {0} to {1}")]
    InvalidStateTransition(TriggerState, TriggerState),

    // --- Resource Errors ---
    #[error("datastore unavailable: {0}")]
    DatastoreUnavailable(String),

    #[error("broker unavailable: {0}")]
    BrokerUnavailable(String),

    #[error("concurrency limit reached")]
    ConcurrencyLimitReached,

    // --- Trigger-Specific Errors ---
    #[error("trigger is not active (current state: {0})")]
    TriggerNotActive(TriggerState),

    #[error("trigger is in error state, manual resume required: {0}")]
    TriggerInErrorState(TriggerId),

    #[error("trigger is disabled: {0}")]
    TriggerDisabled(TriggerId),

    // --- Validation Errors ---
    #[error("invalid trigger configuration: {0}")]
    InvalidConfiguration(String),
}

// -----------------------------------------------------------------------------
// Into<TriggerError> Implementations
// -----------------------------------------------------------------------------

/// Converts `std::io::Error` to `TriggerError::DatastoreUnavailable`.
impl From<std::io::Error> for TriggerError {
    fn from(err: std::io::Error) -> Self {
        TriggerError::DatastoreUnavailable(err.to_string())
    }
}

/// Result type for TriggerRegistry operations
pub type TriggerRegistryResult<T> = std::result::Result<T, TriggerError>;

/// Boxed future type for TriggerRegistry operations
pub type BoxedTriggerFuture<T> =
    Pin<Box<dyn std::future::Future<Output = TriggerRegistryResult<T>> + Send>>;

/// A unique identifier for a created job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobId(pub String);

impl JobId {
    /// Creates a new JobId from a UUID string.
    pub fn new(uuid: &str) -> Self {
        JobId(uuid.to_string())
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trait for trigger registry operations.
///
/// All methods are async and require `Send + Sync` bounds for thread safety.
///
/// # Invariant
/// Implementations MUST be thread-safe (implement `Send + Sync`).
#[async_trait]
pub trait TriggerRegistry: Send + Sync {
    /// Register a new trigger.
    ///
    /// # Preconditions
    /// - `trigger.id` MUST be a valid `TriggerId`
    /// - `trigger.state` MUST be `Active` or `Paused`
    /// - No trigger with the same `trigger.id` may already exist
    ///
    /// # Postconditions
    /// - Returns `Ok(())` if registration succeeds
    /// - Returns `Err(TriggerError::AlreadyExists)` if ID already registered
    /// - Returns `Err(TriggerError::InvalidConfiguration)` if trigger data is invalid
    async fn register(&self, trigger: Trigger) -> TriggerRegistryResult<()>;

    /// Unregister (delete) a trigger by ID.
    ///
    /// # Preconditions
    /// - A trigger with the given `id` MUST exist in the registry
    ///
    /// # Postconditions
    /// - Returns `Ok(())` if unregistration succeeds
    /// - Returns `Err(TriggerError::NotFound)` if ID does not exist
    async fn unregister(&self, id: &TriggerId) -> TriggerRegistryResult<()>;

    /// Update a trigger's state.
    ///
    /// # Preconditions
    /// - A trigger with the given `id` MUST exist
    /// - The state transition MUST be valid per the state transition table
    ///
    /// # Postconditions
    /// - Returns `Ok(())` if state update succeeds
    /// - Returns `Err(TriggerError::NotFound)` if ID does not exist
    /// - Returns `Err(TriggerError::InvalidStateTransition)` if transition not allowed
    async fn set_state(&self, id: &TriggerId, new_state: TriggerState)
        -> TriggerRegistryResult<()>;

    /// Get a trigger by its ID.
    ///
    /// # Preconditions
    /// - None (idempotent read operation)
    ///
    /// # Postconditions
    /// - Returns `Ok(Some(trigger))` if trigger exists
    /// - Returns `Ok(None)` if no trigger with given ID exists
    async fn get(&self, id: &TriggerId) -> TriggerRegistryResult<Option<Trigger>>;

    /// List all registered triggers.
    ///
    /// # Preconditions
    /// - None (idempotent read operation)
    ///
    /// # Postconditions
    /// - Returns `Ok(Vec<Trigger>)` containing all registered triggers
    async fn list(&self) -> TriggerRegistryResult<Vec<Trigger>>;

    /// List all triggers in a specific state.
    ///
    /// # Preconditions
    /// - None (idempotent read operation)
    ///
    /// # Postconditions
    /// - Returns `Ok(Vec<Trigger>)` containing only triggers with `state == target_state`
    async fn list_by_state(
        &self,
        target_state: TriggerState,
    ) -> TriggerRegistryResult<Vec<Trigger>>;

    /// Fire a trigger, creating a job.
    ///
    /// # Preconditions
    /// - A trigger with `ctx.trigger_id` MUST exist
    /// - The trigger MUST be in `Active` state
    /// - Broker (job queue) must be available
    ///
    /// # Postconditions
    /// - Returns `Ok(JobId)` if fire succeeds and job was enqueued
    /// - Returns `Err(TriggerError::NotFound)` if trigger does not exist
    /// - Returns `Err(TriggerError::TriggerNotActive)` if trigger is not Active
    /// - Returns `Err(TriggerError::TriggerDisabled)` if trigger is Disabled
    /// - Returns `Err(TriggerError::TriggerInErrorState)` if Polling trigger is in Error
    /// - Returns `Err(TriggerError::BrokerUnavailable)` if job broker unavailable
    /// - Returns `Err(TriggerError::ConcurrencyLimitReached)` if concurrency limit hit
    async fn fire(&self, ctx: TriggerContext) -> TriggerRegistryResult<JobId>;
}

// =============================================================================
// In-memory fake implementation for testing
// =============================================================================

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// An in-memory fake implementation of TriggerRegistry for testing.
pub struct InMemoryTriggerRegistry {
    triggers: Arc<RwLock<HashMap<TriggerId, Trigger>>>,
    fire_count: AtomicUsize,
    broker_available: AtomicBool,
    datastore_available: AtomicBool,
    concurrency_limiter: Arc<Semaphore>,
}

impl InMemoryTriggerRegistry {
    /// Creates a new InMemoryTriggerRegistry with all systems available.
    pub fn new() -> Self {
        Self {
            triggers: Arc::new(RwLock::new(HashMap::new())),
            fire_count: AtomicUsize::new(0),
            broker_available: AtomicBool::new(true),
            datastore_available: AtomicBool::new(true),
            // Use a large but finite limit (within MAX_PERMITS of 2^61-1)
            concurrency_limiter: Arc::new(Semaphore::new(1024)),
        }
    }

    /// Creates a new InMemoryTriggerRegistry with specified concurrency limit.
    pub fn with_concurrency_limit(limit: usize) -> Self {
        Self {
            triggers: Arc::new(RwLock::new(HashMap::new())),
            fire_count: AtomicUsize::new(0),
            broker_available: AtomicBool::new(true),
            datastore_available: AtomicBool::new(true),
            concurrency_limiter: Arc::new(Semaphore::new(limit)),
        }
    }

    /// Simulates broker unavailability.
    pub fn set_broker_available(&self, available: bool) {
        self.broker_available.store(available, Ordering::SeqCst);
    }

    /// Simulates datastore unavailability.
    pub fn set_datastore_available(&self, available: bool) {
        self.datastore_available.store(available, Ordering::SeqCst);
    }

    /// Returns the number of times fire() was called.
    pub fn fire_count(&self) -> usize {
        self.fire_count.load(Ordering::SeqCst)
    }
}

impl Default for InMemoryTriggerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TriggerRegistry for InMemoryTriggerRegistry {
    async fn register(&self, trigger: Trigger) -> TriggerRegistryResult<()> {
        // Check datastore availability
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }

        let mut triggers = self.triggers.write();

        // Check if already exists
        if triggers.contains_key(&trigger.id) {
            return Err(TriggerError::AlreadyExists(trigger.id.clone()));
        }

        // Check precondition: state must be Active or Paused
        match trigger.state {
            TriggerState::Active | TriggerState::Paused => {}
            TriggerState::Disabled => {
                return Err(TriggerError::InvalidConfiguration(
                    "new triggers cannot start in Disabled state".into(),
                ));
            }
            TriggerState::Error => {
                return Err(TriggerError::InvalidConfiguration(
                    "new triggers cannot start in Error state".into(),
                ));
            }
        }

        triggers.insert(trigger.id.clone(), trigger);
        Ok(())
    }

    async fn unregister(&self, id: &TriggerId) -> TriggerRegistryResult<()> {
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }

        let mut triggers = self.triggers.write();
        if triggers.remove(id).is_none() {
            return Err(TriggerError::NotFound(id.clone()));
        }
        Ok(())
    }

    async fn set_state(
        &self,
        id: &TriggerId,
        new_state: TriggerState,
    ) -> TriggerRegistryResult<()> {
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }

        let mut triggers = self.triggers.write();
        let trigger = triggers
            .get_mut(id)
            .ok_or_else(|| TriggerError::NotFound(id.clone()))?;

        let old_state = trigger.state;

        // Validate state transition
        let variant = trigger.variant;
        if !is_valid_transition(old_state, new_state, variant) {
            return Err(TriggerError::InvalidStateTransition(
                old_state,
                new_state,
            ));
        }

        trigger.state = new_state;
        Ok(())
    }

    async fn get(&self, id: &TriggerId) -> TriggerRegistryResult<Option<Trigger>> {
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }

        let triggers = self.triggers.read();
        Ok(triggers.get(id).cloned())
    }

    async fn list(&self) -> TriggerRegistryResult<Vec<Trigger>> {
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }

        let triggers = self.triggers.read();
        Ok(triggers.values().cloned().collect())
    }

    async fn list_by_state(
        &self,
        target_state: TriggerState,
    ) -> TriggerRegistryResult<Vec<Trigger>> {
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }

        let triggers = self.triggers.read();
        Ok(triggers
            .values()
            .filter(|t| t.state == target_state)
            .cloned()
            .collect())
    }

    async fn fire(&self, ctx: TriggerContext) -> TriggerRegistryResult<JobId> {
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }

        // Check broker availability
        if !self.broker_available.load(Ordering::SeqCst) {
            return Err(TriggerError::BrokerUnavailable("connection refused".into()));
        }

        // Try to acquire semaphore permit (concurrency limit)
        let permit = self.concurrency_limiter.acquire().await;
        if permit.is_err() {
            // Semaphore was closed, treat as limit reached
            return Err(TriggerError::ConcurrencyLimitReached);
        }
        let _permit = permit.unwrap();

        // Increment fire count
        self.fire_count.fetch_add(1, Ordering::SeqCst);

        // Get the trigger
        let trigger = {
            let triggers = self.triggers.read();
            triggers.get(&ctx.trigger_id).cloned()
        };

        let trigger = trigger.ok_or_else(|| TriggerError::NotFound(ctx.trigger_id.clone()))?;

        match trigger.state {
            TriggerState::Active => {}
            TriggerState::Paused => {
                return Err(TriggerError::TriggerNotActive(trigger.state));
            }
            TriggerState::Disabled => {
                return Err(TriggerError::TriggerDisabled(trigger.id));
            }
            TriggerState::Error => {
                if trigger.variant == TriggerVariant::Polling {
                    return Err(TriggerError::TriggerInErrorState(trigger.id));
                }
                // Error state for non-Polling triggers should not happen but handle it
                return Err(TriggerError::TriggerInErrorState(trigger.id));
            }
        }

        // Create a job ID
        Ok(JobId::new(&uuid::Uuid::new_v4().to_string()))
    }
}

/// Checks if a state transition is valid for the given variant.
fn is_valid_transition(from: TriggerState, to: TriggerState, variant: TriggerVariant) -> bool {
    match (from, to) {
        // Self-transitions are always valid
        (TriggerState::Active, TriggerState::Active) => true,
        (TriggerState::Paused, TriggerState::Paused) => true,
        (TriggerState::Disabled, TriggerState::Disabled) => true,
        (TriggerState::Error, TriggerState::Error) => true,

        // Active can go to Paused, Disabled, Error (Polling only)
        (TriggerState::Active, TriggerState::Paused) => true,
        (TriggerState::Active, TriggerState::Disabled) => true,
        (TriggerState::Active, TriggerState::Error) => variant == TriggerVariant::Polling,

        // Paused can go to Active, Disabled
        (TriggerState::Paused, TriggerState::Active) => true,
        (TriggerState::Paused, TriggerState::Disabled) => true,
        (TriggerState::Paused, TriggerState::Error) => false,

        // Disabled can go to Active, Paused
        (TriggerState::Disabled, TriggerState::Active) => true,
        (TriggerState::Disabled, TriggerState::Paused) => true,
        (TriggerState::Disabled, TriggerState::Error) => false,

        // Error can only go to Active for Polling triggers
        (TriggerState::Error, TriggerState::Active) => variant == TriggerVariant::Polling,
        (TriggerState::Error, TriggerState::Paused) => false,
        (TriggerState::Error, TriggerState::Disabled) => false,
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TriggerId Validation Tests
    // =========================================================================

    #[test]
    fn trigger_id_returns_ok_when_input_is_3_alphanumeric_chars() {
        let result = TriggerId::new("abc");
        assert_eq!(result, Ok(TriggerId("abc".into())));
    }

    #[test]
    fn trigger_id_returns_ok_when_input_contains_hyphens() {
        let result = TriggerId::new("my-trigger-001");
        assert_eq!(result, Ok(TriggerId("my-trigger-001".into())));
    }

    #[test]
    fn trigger_id_returns_ok_when_input_contains_underscores() {
        let result = TriggerId::new("my_trigger_001");
        assert_eq!(result, Ok(TriggerId("my_trigger_001".into())));
    }

    #[test]
    fn trigger_id_returns_ok_when_input_is_64_alphanumeric_chars() {
        let input = "a".repeat(64);
        let result = TriggerId::new(&input);
        assert_eq!(result, Ok(TriggerId(input)));
    }

    #[test]
    fn trigger_id_returns_length_error_when_input_too_short() {
        let result = TriggerId::new("ab");
        assert_eq!(result, Err(TriggerIdError::LengthOutOfRange(2)));
    }

    #[test]
    fn trigger_id_returns_length_error_when_input_too_long() {
        let input = "a".repeat(65);
        let result = TriggerId::new(&input);
        assert_eq!(result, Err(TriggerIdError::LengthOutOfRange(65)));
    }

    #[test]
    fn trigger_id_returns_length_error_when_input_is_empty() {
        let result = TriggerId::new("");
        assert_eq!(result, Err(TriggerIdError::LengthOutOfRange(0)));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_spaces() {
        let result = TriggerId::new("my trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacter(' ')));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_special_chars() {
        let result = TriggerId::new("my@trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacter('@')));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_unicode() {
        let result = TriggerId::new("触发器");
        assert!(matches!(result, Err(TriggerIdError::InvalidCharacter(_))));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_control_char() {
        let result = TriggerId::new("my\ntrigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacter('\n')));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_dot() {
        let result = TriggerId::new("my.trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacter('.')));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_hash() {
        let result = TriggerId::new("my#trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacter('#')));
    }

    // =========================================================================
    // TriggerState Serialization Tests
    // =========================================================================

    #[test]
    fn trigger_state_serializes_to_pascal_case() {
        assert_eq!(
            serde_json::to_string(&TriggerState::Active).unwrap(),
            "\"Active\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerState::Paused).unwrap(),
            "\"Paused\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerState::Disabled).unwrap(),
            "\"Disabled\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerState::Error).unwrap(),
            "\"Error\""
        );
    }

    #[test]
    fn trigger_state_deserializes_from_pascal_case() {
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"Active\"").unwrap(),
            TriggerState::Active
        );
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"Paused\"").unwrap(),
            TriggerState::Paused
        );
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"Disabled\"").unwrap(),
            TriggerState::Disabled
        );
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"Error\"").unwrap(),
            TriggerState::Error
        );
    }

    // =========================================================================
    // TriggerVariant Serialization Tests
    // =========================================================================

    #[test]
    fn trigger_variant_serializes_to_pascal_case() {
        assert_eq!(
            serde_json::to_string(&TriggerVariant::Cron).unwrap(),
            "\"Cron\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerVariant::Webhook).unwrap(),
            "\"Webhook\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerVariant::Polling).unwrap(),
            "\"Polling\""
        );
    }

    #[test]
    fn trigger_variant_deserializes_from_pascal_case() {
        assert_eq!(
            serde_json::from_str::<TriggerVariant>("\"Cron\"").unwrap(),
            TriggerVariant::Cron
        );
        assert_eq!(
            serde_json::from_str::<TriggerVariant>("\"Webhook\"").unwrap(),
            TriggerVariant::Webhook
        );
        assert_eq!(
            serde_json::from_str::<TriggerVariant>("\"Polling\"").unwrap(),
            TriggerVariant::Polling
        );
    }

    // =========================================================================
    // TriggerError Display Tests
    // =========================================================================

    #[test]
    fn trigger_error_not_found_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::NotFound(id.clone());
        assert!(err.to_string().contains("trigger not found"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_already_exists_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::AlreadyExists(id.clone());
        assert!(err.to_string().contains("trigger already registered"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_invalid_state_transition_displays_correctly() {
        let err = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error);
        assert!(err.to_string().contains("invalid state transition"));
        assert!(err.to_string().contains("Active"));
        assert!(err.to_string().contains("Error"));
    }

    #[test]
    fn trigger_error_datastore_unavailable_displays_correctly() {
        let err = TriggerError::DatastoreUnavailable("connection refused".into());
        assert!(err.to_string().contains("datastore unavailable"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn trigger_error_broker_unavailable_displays_correctly() {
        let err = TriggerError::BrokerUnavailable("connection refused".into());
        assert!(err.to_string().contains("broker unavailable"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn trigger_error_concurrency_limit_reached_displays_correctly() {
        let err = TriggerError::ConcurrencyLimitReached;
        assert!(err.to_string().contains("concurrency limit reached"));
    }

    #[test]
    fn trigger_error_trigger_not_active_displays_correctly() {
        let err = TriggerError::TriggerNotActive(TriggerState::Paused);
        assert!(err.to_string().contains("trigger is not active"));
        assert!(err.to_string().contains("Paused"));
    }

    #[test]
    fn trigger_error_trigger_in_error_state_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::TriggerInErrorState(id.clone());
        assert!(err.to_string().contains("trigger is in error state"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_trigger_disabled_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::TriggerDisabled(id.clone());
        assert!(err.to_string().contains("trigger is disabled"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_invalid_configuration_displays_correctly() {
        let err = TriggerError::InvalidConfiguration("test error".into());
        assert!(err.to_string().contains("invalid trigger configuration"));
        assert!(err.to_string().contains("test error"));
    }

    // =========================================================================
    // TriggerIdError Display Tests
    // =========================================================================

    #[test]
    fn trigger_id_error_length_out_of_range_displays_correctly() {
        let err = TriggerIdError::LengthOutOfRange(5);
        assert!(err.to_string().contains("length out of range"));
        assert!(err.to_string().contains("5"));
    }

    #[test]
    fn trigger_id_error_invalid_character_displays_correctly() {
        let err = TriggerIdError::InvalidCharacter('@');
        assert!(err.to_string().contains("invalid character"));
        assert!(err.to_string().contains("@"));
    }

    // =========================================================================
    // State Transition Matrix Tests
    // =========================================================================

    #[test]
    fn is_valid_transition_active_to_paused_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_active_to_disabled_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_active_to_error_for_polling_only() {
        assert!(!is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_paused_to_active_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_paused_to_disabled_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_paused_to_error_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_disabled_to_active_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_disabled_to_paused_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_disabled_to_error_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_error_to_active_for_polling_only() {
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_error_to_paused_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_error_to_disabled_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_active_state() {
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_paused_state() {
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_disabled_state() {
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_error_state() {
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    // =========================================================================
    // InMemoryTriggerRegistry Tests
    // =========================================================================

    #[tokio::test]
    async fn register_succeeds_when_trigger_is_valid_and_id_is_unique() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger-001".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        let result = registry.register(trigger).await;
        assert_eq!(result, Ok(()));

        // Verify it's accessible via get
        let retrieved = registry.get(&TriggerId("test-trigger-001".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().id.as_str(), "test-trigger-001");
    }

    #[tokio::test]
    async fn register_returns_already_exists_when_id_is_duplicate() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger.clone()).await.unwrap();
        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::AlreadyExists(TriggerId(
                "test-trigger".into()
            )))
        );
    }

    #[tokio::test]
    async fn register_returns_invalid_configuration_when_state_is_disabled() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Disabled,
            variant: TriggerVariant::Cron,
        };

        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Disabled state".into()
            ))
        );
    }

    #[tokio::test]
    async fn register_returns_invalid_configuration_when_state_is_error() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Error,
            variant: TriggerVariant::Polling,
        };

        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Error state".into()
            ))
        );
    }

    #[tokio::test]
    async fn unregister_succeeds_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("to-delete".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry.unregister(&TriggerId("to-delete".into())).await;
        assert_eq!(result, Ok(()));

        // Verify it's gone
        let retrieved = registry.get(&TriggerId("to-delete".into())).await;
        assert_eq!(retrieved.unwrap(), None);
    }

    #[tokio::test]
    async fn unregister_returns_not_found_when_trigger_does_not_exist() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry.unregister(&TriggerId("nonexistent".into())).await;
        assert_eq!(
            result,
            Err(TriggerError::NotFound(TriggerId("nonexistent".into())))
        );
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_paused_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Paused)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Paused);
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_disabled_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Disabled);
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_error_for_polling_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Error);
    }

    #[tokio::test]
    async fn set_state_transitions_paused_to_active_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Active);
    }

    #[tokio::test]
    async fn set_state_transitions_paused_to_disabled_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Disabled);
    }

    #[tokio::test]
    async fn set_state_transitions_disabled_to_active_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Disabled,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Active);
    }

    #[tokio::test]
    async fn set_state_transitions_disabled_to_paused_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Disabled,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Paused)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Paused);
    }

    #[tokio::test]
    async fn set_state_transitions_error_to_active_for_polling_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Error,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Active);
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_active_self_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn set_state_rejects_active_to_error_for_cron_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Active,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_active_to_error_for_webhook_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Webhook,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Active,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_paused_to_error_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Paused,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_disabled_to_error_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Disabled,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Disabled,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_error_to_paused_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Error,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Paused)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Error,
                TriggerState::Paused
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_error_to_disabled_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Error,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Error,
                TriggerState::Disabled
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_error_to_active_for_cron_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Error,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Error,
                TriggerState::Active
            ))
        );
    }

    #[tokio::test]
    async fn set_state_returns_not_found_when_trigger_does_not_exist() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry
            .set_state(&TriggerId("nonexistent".into()), TriggerState::Active)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::NotFound(TriggerId("nonexistent".into())))
        );
    }

    #[tokio::test]
    async fn get_returns_some_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(result.unwrap().unwrap().id.as_str(), "test-trigger");
    }

    #[tokio::test]
    async fn get_returns_none_when_trigger_not_found() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry.get(&TriggerId("nonexistent".into())).await;
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn list_returns_all_triggers_when_triggers_exist() {
        let registry = InMemoryTriggerRegistry::new();

        registry
            .register(Trigger {
                id: TriggerId("trigger-1".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Cron,
            })
            .await
            .unwrap();

        registry
            .register(Trigger {
                id: TriggerId("trigger-2".into()),
                state: TriggerState::Paused,
                variant: TriggerVariant::Webhook,
            })
            .await
            .unwrap();

        let result = registry.list().await;
        let triggers = result.unwrap();
        assert_eq!(triggers.len(), 2);
    }

    #[tokio::test]
    async fn list_returns_empty_vec_when_no_triggers_exist() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry.list().await;
        assert_eq!(result.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn list_by_state_returns_matching_triggers_when_matches_exist() {
        let registry = InMemoryTriggerRegistry::new();

        registry
            .register(Trigger {
                id: TriggerId("active-1".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Cron,
            })
            .await
            .unwrap();

        registry
            .register(Trigger {
                id: TriggerId("paused-1".into()),
                state: TriggerState::Paused,
                variant: TriggerVariant::Webhook,
            })
            .await
            .unwrap();

        registry
            .register(Trigger {
                id: TriggerId("active-2".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Polling,
            })
            .await
            .unwrap();

        let result = registry.list_by_state(TriggerState::Active).await;
        let triggers = result.unwrap();
        assert_eq!(triggers.len(), 2);
        assert!(triggers.iter().all(|t| t.state == TriggerState::Active));
    }

    #[tokio::test]
    async fn list_by_state_returns_empty_when_no_matches() {
        let registry = InMemoryTriggerRegistry::new();

        registry
            .register(Trigger {
                id: TriggerId("active-1".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Cron,
            })
            .await
            .unwrap();

        let result = registry.list_by_state(TriggerState::Paused).await;
        assert_eq!(result.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn fire_returns_job_id_when_trigger_is_active_and_broker_available() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert!(result.is_ok(), "fire should succeed for active trigger with broker available");
        let job_id = result.unwrap();
        assert_eq!(job_id.0.len(), 36); // UUID v4 length
    }

    #[tokio::test]
    async fn fire_returns_not_found_when_trigger_does_not_exist() {
        let registry = InMemoryTriggerRegistry::new();

        let ctx = TriggerContext {
            trigger_id: TriggerId("nonexistent".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::NotFound(TriggerId("nonexistent".into())))
        );
    }

    #[tokio::test]
    async fn fire_returns_trigger_not_active_when_trigger_is_paused() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::TriggerNotActive(TriggerState::Paused))
        );
    }

    #[tokio::test]
    async fn fire_returns_trigger_disabled_when_trigger_is_disabled() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Disabled,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::TriggerDisabled(TriggerId(
                "test-trigger".into()
            )))
        );
    }

    #[tokio::test]
    async fn fire_returns_trigger_in_error_state_when_polling_trigger_is_in_error() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Error,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Polling,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::TriggerInErrorState(TriggerId(
                "test-trigger".into()
            )))
        );
    }

    #[tokio::test]
    async fn fire_returns_broker_unavailable_when_broker_is_down() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        registry.set_broker_available(false);

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::BrokerUnavailable("connection refused".into()))
        );
    }

    #[tokio::test]
    async fn fire_returns_concurrency_limit_when_limit_reached() {
        let registry = InMemoryTriggerRegistry::with_concurrency_limit(0);
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(result, Err(TriggerError::ConcurrencyLimitReached));
    }

    // =========================================================================
    // Datastore Unavailability Tests
    // =========================================================================

    #[tokio::test]
    async fn register_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn unregister_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.unregister(&TriggerId("test-trigger".into())).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn set_state_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Paused)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn get_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn list_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.list().await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn list_by_state_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.list_by_state(TriggerState::Active).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }
}

// =============================================================================
// Proptest Property-Based Tests
// =============================================================================

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    // Implement Arbitrary for TriggerState manually
    impl Arbitrary for TriggerState {
        type Strategy = BoxedStrategy<TriggerState>;
        type Parameters = ();

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            prop_oneof![
                Just(TriggerState::Active),
                Just(TriggerState::Paused),
                Just(TriggerState::Disabled),
                Just(TriggerState::Error),
            ]
            .boxed()
        }
    }

    // Implement Arbitrary for TriggerVariant manually
    impl Arbitrary for TriggerVariant {
        type Strategy = BoxedStrategy<TriggerVariant>;
        type Parameters = ();

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            prop_oneof![
                Just(TriggerVariant::Cron),
                Just(TriggerVariant::Webhook),
                Just(TriggerVariant::Polling),
            ]
            .boxed()
        }
    }

    proptest! {
        #[test]
        fn trigger_id_new_accepts_any_valid_3_to_64_char_alphanumeric_input(s in "[a-zA-Z0-9]{3,64}") {
            let result = TriggerId::new(&s);
            prop_assert_eq!(result, Ok(TriggerId(s)));
        }

        #[test]
        fn trigger_id_new_accepts_hyphens_and_underscores(
            prefix in "[a-zA-Z0-9]{1,20}",
            suffix in "[a-zA-Z0-9]{1,20}"
        ) {
            let with_hyphen = format!("{}-{}", prefix, suffix);
            let with_underscore = format!("{}_{}", prefix, suffix);

            prop_assert_eq!(TriggerId::new(&with_hyphen), Ok(TriggerId(with_hyphen.clone())));
            prop_assert_eq!(TriggerId::new(&with_underscore), Ok(TriggerId(with_underscore.clone())));
        }

        #[test]
        fn trigger_id_new_rejects_strings_shorter_than_3_chars(s in "[a-zA-Z0-9]{0,2}") {
            let result = TriggerId::new(&s);
            prop_assert_eq!(result, Err(TriggerIdError::LengthOutOfRange(s.len())));
        }

        #[test]
        fn trigger_id_new_rejects_strings_longer_than_64_chars(s in "[a-zA-Z0-9]{65,128}") {
            let result = TriggerId::new(&s);
            prop_assert_eq!(result, Err(TriggerIdError::LengthOutOfRange(s.len())));
        }

        #[test]
        fn trigger_id_roundtrip_through_string_preserves_value(s in "[a-zA-Z0-9\\-_]{3,64}") {
            let id = TriggerId::new(&s).unwrap();
            prop_assert_eq!(id.as_str(), s);
        }
    }

    proptest! {
        #[test]
        fn state_transition_matrix_exhaustive_validation(
            from_state: TriggerState,
            to_state: TriggerState,
            variant: TriggerVariant
        ) {
            let result = is_valid_transition(from_state, to_state, variant);

            // Self-transitions are always valid
            if from_state == to_state {
                prop_assert!(result);
            } else if from_state == TriggerState::Error && to_state == TriggerState::Active {
                // Error -> Active only valid for Polling
                prop_assert_eq!(result, variant == TriggerVariant::Polling);
            } else if from_state == TriggerState::Active && to_state == TriggerState::Error {
                // Active -> Error only valid for Polling
                prop_assert_eq!(result, variant == TriggerVariant::Polling);
            } else if from_state == TriggerState::Error {
                // All other transitions from Error are invalid
                prop_assert!(!result);
            } else if to_state == TriggerState::Error {
                // Transitions to Error are only valid from Active for Polling
                prop_assert!(!result);
            }
        }
    }
}

// =============================================================================
// Kani Formal Verification Harnesses
// =============================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[cfg(kani)]
    #[kani::proof]
    fn verify_trigger_id_length_bounds() {
        let input: String = kani::any();
        let len = input.len();

        // If TriggerId::new returns Ok, the length must be in valid range
        if TriggerId::new(&input).is_ok() {
            kani::assert(
                len >= 3 && len <= 64,
                "Valid TriggerId must have length 3-64",
            );
        }
    }

    #[cfg(kani)]
    #[kani::proof]
    fn verify_state_transition_matrix_completeness() {
        // Exhaustively check all combinations
        let states = [
            TriggerState::Active,
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerState::Error,
        ];
        let variants = [
            TriggerVariant::Cron,
            TriggerVariant::Webhook,
            TriggerVariant::Polling,
        ];

        for from in &states {
            for to in &states {
                for variant in &variants {
                    let _ = is_valid_transition(*from, *to, *variant);
                }
            }
        }
    }

    #[cfg(kani)]
    #[kani::proof]
    async fn verify_unique_id_constraint_after_register() {
        // This would need a more complex harness to verify truly
        // Placeholder for the invariant that register never creates duplicates
        let registry = InMemoryTriggerRegistry::new();
        let id = TriggerId("unique-trigger".into());
        let trigger = Trigger {
            id: id.clone(),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        // First register should succeed
        let result = registry.register(trigger.clone()).await;
        kani::assert(result.is_ok(), "First register of unique ID should succeed");
    }
}
