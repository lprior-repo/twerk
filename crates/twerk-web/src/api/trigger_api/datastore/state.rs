use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use time::OffsetDateTime;

use super::super::domain::{Trigger, TriggerId, TriggerUpdateError, TriggerUpdateRequest};

pub const PERSISTENCE_MSG: &str = "internal persistence failure";

#[derive(Clone)]
pub struct InMemoryTriggerDatastore {
    data: Arc<Mutex<HashMap<String, Trigger>>>,
    fail_next_update: Arc<AtomicBool>,
}

impl InMemoryTriggerDatastore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            fail_next_update: Arc::new(AtomicBool::new(false)),
        }
    }

    fn lock_map(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, Trigger>>, TriggerUpdateError> {
        self.data
            .lock()
            .map_err(|_| TriggerUpdateError::Persistence(PERSISTENCE_MSG.to_string()))
    }

    fn parse_request_id(req: &TriggerUpdateRequest) -> Result<TriggerId, TriggerUpdateError> {
        req.id
            .as_deref()
            .map_or_else(
                || TriggerId::parse(&twerk_core::uuid::new_short_uuid()),
                TriggerId::parse,
            )
            .map_err(|err| TriggerUpdateError::InvalidIdFormat(err.to_string()))
    }

    /// Insert or replace a trigger by identifier.
    ///
    /// # Errors
    /// Returns an error when the in-memory datastore lock cannot be acquired.
    pub fn upsert(&self, trigger: Trigger) -> Result<(), TriggerUpdateError> {
        self.lock_map()?
            .insert(trigger.id.as_str().to_string(), trigger);
        Ok(())
    }

    /// Create a new trigger from an update request.
    ///
    /// # Errors
    /// Returns an error when the request ID is invalid or the datastore lock cannot be acquired.
    pub fn create_trigger(&self, req: TriggerUpdateRequest) -> Result<Trigger, TriggerUpdateError> {
        let now_utc = OffsetDateTime::now_utc();
        let trigger = Trigger {
            id: Self::parse_request_id(&req)?,
            name: req.name.trim().to_string(),
            enabled: req.enabled,
            event: req.event.trim().to_string(),
            condition: req.condition,
            action: req.action.trim().to_string(),
            metadata: req.metadata.unwrap_or_default(),
            version: 1,
            created_at: now_utc,
            updated_at: now_utc,
        };

        self.lock_map()?
            .insert(trigger.id.as_str().to_string(), trigger.clone());
        Ok(trigger)
    }

    /// Load a trigger by identifier.
    ///
    /// # Errors
    /// Returns an error when the datastore lock cannot be acquired or the trigger does not exist.
    pub fn get_trigger_by_id(&self, id: &TriggerId) -> Result<Trigger, TriggerUpdateError> {
        self.lock_map()?
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| TriggerUpdateError::TriggerNotFound(id.as_str().to_string()))
    }

    /// List every trigger stored in memory.
    ///
    /// # Errors
    /// Returns an error when the datastore lock cannot be acquired.
    pub fn list_triggers(&self) -> Result<Vec<Trigger>, TriggerUpdateError> {
        Ok(self.lock_map()?.values().cloned().collect())
    }

    /// Update an existing trigger with a caller-provided transformation.
    ///
    /// # Errors
    /// Returns an error when the datastore is configured to fail the next update, the lock cannot
    /// be acquired, the trigger does not exist, or the supplied transformation rejects the update.
    pub fn update_trigger(
        &self,
        id: &TriggerId,
        modify: Box<dyn FnOnce(Trigger) -> Result<Trigger, TriggerUpdateError> + Send>,
    ) -> Result<Trigger, TriggerUpdateError> {
        if self.fail_next_update.swap(false, Ordering::SeqCst) {
            return Err(TriggerUpdateError::Persistence(PERSISTENCE_MSG.to_string()));
        }

        let current = self
            .lock_map()?
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| TriggerUpdateError::TriggerNotFound(id.as_str().to_string()))?;
        let updated = modify(current)?;
        self.lock_map()?
            .insert(id.as_str().to_string(), updated.clone());
        Ok(updated)
    }

    #[must_use]
    pub fn set_fail_next_update(&self, should_fail: bool) -> bool {
        self.fail_next_update.swap(should_fail, Ordering::SeqCst)
    }

    /// Delete a trigger by identifier.
    ///
    /// # Errors
    /// Returns an error when the datastore lock cannot be acquired or the trigger does not exist.
    pub fn delete_trigger(&self, id: &TriggerId) -> Result<(), TriggerUpdateError> {
        self.lock_map()?
            .remove(id.as_str())
            .ok_or_else(|| TriggerUpdateError::TriggerNotFound(id.as_str().to_string()))?;
        Ok(())
    }
}

impl Default for InMemoryTriggerDatastore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct TriggerAppState {
    pub trigger_ds: Arc<InMemoryTriggerDatastore>,
}
