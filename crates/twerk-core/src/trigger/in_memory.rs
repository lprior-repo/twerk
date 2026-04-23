//! In-memory implementation of TriggerRegistry.
//!
//! This module provides a thread-safe, in-memory implementation for testing.

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

use super::r#trait::{TriggerRegistry, TriggerRegistryResult};
use super::types::{
    JobId, Trigger, TriggerContext, TriggerError, TriggerId, TriggerState, TriggerVariant,
};

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

// -----------------------------------------------------------------------------
// Helper methods for fire() decomposition (pub(crate) for testing)
// -----------------------------------------------------------------------------

impl InMemoryTriggerRegistry {
    pub(crate) fn check_datastore_available(&self) -> TriggerRegistryResult<()> {
        if !self.datastore_available.load(Ordering::SeqCst) {
            return Err(TriggerError::DatastoreUnavailable(
                "connection refused".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn check_broker_available(&self) -> TriggerRegistryResult<()> {
        if !self.broker_available.load(Ordering::SeqCst) {
            return Err(TriggerError::BrokerUnavailable("connection refused".into()));
        }
        Ok(())
    }

    pub(crate) fn acquire_concurrency_permit(&self) -> TriggerRegistryResult<SemaphorePermit<'_>> {
        self.concurrency_limiter
            .try_acquire()
            .map_err(|_| TriggerError::ConcurrencyLimitReached)
    }

    pub(crate) fn get_trigger(&self, trigger_id: &TriggerId) -> TriggerRegistryResult<Trigger> {
        let triggers = self.triggers.read();
        triggers
            .get(trigger_id)
            .cloned()
            .ok_or_else(|| TriggerError::NotFound(trigger_id.clone()))
    }

    pub(crate) fn validate_trigger_for_fire(&self, trigger: &Trigger) -> TriggerRegistryResult<()> {
        match trigger.state {
            TriggerState::Active => Ok(()),
            TriggerState::Paused => Err(TriggerError::TriggerNotActive(trigger.state)),
            TriggerState::Disabled => Err(TriggerError::TriggerDisabled(trigger.id.clone())),
            TriggerState::Error => Err(TriggerError::TriggerInErrorState(trigger.id.clone())),
        }
    }

    pub(crate) fn validate_trigger_for_registration(
        &self,
        trigger: &Trigger,
    ) -> TriggerRegistryResult<()> {
        match trigger.state {
            TriggerState::Active | TriggerState::Paused => Ok(()),
            TriggerState::Disabled => Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Disabled state".into(),
            )),
            TriggerState::Error => Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Error state".into(),
            )),
        }
    }

    pub(crate) fn apply_state_transition(
        trigger: &mut Trigger,
        new_state: TriggerState,
    ) -> TriggerRegistryResult<()> {
        let old_state = trigger.state;
        if !is_valid_transition(old_state, new_state, trigger.variant) {
            Err(TriggerError::InvalidStateTransition(old_state, new_state))
        } else {
            trigger.state = new_state;
            Ok(())
        }
    }
}

#[async_trait]
impl TriggerRegistry for InMemoryTriggerRegistry {
    async fn register(&self, trigger: Trigger) -> TriggerRegistryResult<()> {
        self.check_datastore_available()?;

        let mut triggers = self.triggers.write();
        if triggers.contains_key(&trigger.id) {
            return Err(TriggerError::AlreadyExists(trigger.id));
        }

        self.validate_trigger_for_registration(&trigger)?;
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
        self.check_datastore_available()?;
        let mut triggers = self.triggers.write();
        let trigger = triggers
            .get_mut(id)
            .ok_or_else(|| TriggerError::NotFound(id.clone()))?;
        Self::apply_state_transition(trigger, new_state)
    }

    async fn get(&self, id: &TriggerId) -> TriggerRegistryResult<Option<Trigger>> {
        self.check_datastore_available()?;

        let triggers = self.triggers.read();
        Ok(triggers.get(id).cloned())
    }

    async fn list(&self) -> TriggerRegistryResult<Vec<Trigger>> {
        self.check_datastore_available()?;

        let triggers = self.triggers.read();
        Ok(triggers.values().cloned().collect())
    }

    async fn list_by_state(
        &self,
        target_state: TriggerState,
    ) -> TriggerRegistryResult<Vec<Trigger>> {
        self.check_datastore_available()?;

        let triggers = self.triggers.read();
        Ok(triggers
            .values()
            .filter(|t| t.state == target_state)
            .cloned()
            .collect())
    }

    async fn fire(&self, ctx: TriggerContext) -> TriggerRegistryResult<JobId> {
        self.check_datastore_available()?;
        self.check_broker_available()?;
        let _permit = self.acquire_concurrency_permit()?;
        self.fire_count.fetch_add(1, Ordering::SeqCst);
        let trigger = self.get_trigger(&ctx.trigger_id)?;
        self.validate_trigger_for_fire(&trigger)?;
        let uuid_str = uuid::Uuid::new_v4().to_string();
        JobId::new(uuid_str).map_err(|e| TriggerError::JobIdGenerationFailed(e.to_string()))
    }
}

/// Checks if a state transition is valid for the given variant.
pub fn is_valid_transition(from: TriggerState, to: TriggerState, variant: TriggerVariant) -> bool {
    // Self-transitions are always valid
    if from == to {
        return true;
    }

    // Paused/Disabled cannot transition to Error
    if to == TriggerState::Error {
        return matches!(
            (from, variant),
            (
                TriggerState::Active | TriggerState::Error,
                TriggerVariant::Polling
            )
        );
    }

    // Error can only go to Active (Polling only)
    if from == TriggerState::Error {
        return to == TriggerState::Active && variant == TriggerVariant::Polling;
    }

    // All other cross-state transitions are valid
    true
}
