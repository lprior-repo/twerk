//! TriggerRegistry trait definition.
//!
//! Defines the interface for trigger lifecycle management.

use async_trait::async_trait;
use std::pin::Pin;

use super::types::{JobId, Trigger, TriggerContext, TriggerError, TriggerId, TriggerState};

/// Result type for TriggerRegistry operations
pub type TriggerRegistryResult<T> = std::result::Result<T, TriggerError>;

/// Boxed future type for TriggerRegistry operations
pub type BoxedTriggerFuture<T> =
    Pin<Box<dyn std::future::Future<Output = TriggerRegistryResult<T>> + Send>>;

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
