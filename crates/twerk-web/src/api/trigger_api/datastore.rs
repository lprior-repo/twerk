#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::domain::{Trigger, TriggerId, TriggerUpdateError};

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

    pub fn upsert(&self, trigger: Trigger) {
        if let Ok(mut map) = self.data.lock() {
            map.insert(trigger.id.as_str().to_string(), trigger);
        }
    }

    /// Fetch trigger by id.
    ///
    /// # Errors
    /// Returns not-found or persistence errors.
    pub fn get_trigger_by_id(&self, id: &TriggerId) -> Result<Trigger, TriggerUpdateError> {
        self.data
            .lock()
            .map_err(|_| TriggerUpdateError::Persistence(PERSISTENCE_MSG.to_string()))?
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| TriggerUpdateError::TriggerNotFound(id.as_str().to_string()))
    }

    /// Atomically update trigger via closure, returning the updated trigger.
    ///
    /// # Errors
    /// Returns persistence, not-found, or closure domain errors.
    pub fn update_trigger(
        &self,
        id: &TriggerId,
        modify: Box<dyn FnOnce(Trigger) -> Result<Trigger, TriggerUpdateError> + Send>,
    ) -> Result<Trigger, TriggerUpdateError> {
        if self.fail_next_update.swap(false, Ordering::SeqCst) {
            return Err(TriggerUpdateError::Persistence(PERSISTENCE_MSG.to_string()));
        }

        let mut map = self
            .data
            .lock()
            .map_err(|_| TriggerUpdateError::Persistence(PERSISTENCE_MSG.to_string()))?;
        let current = map
            .get(id.as_str())
            .cloned()
            .ok_or_else(|| TriggerUpdateError::TriggerNotFound(id.as_str().to_string()))?;
        let updated = modify(current)?;
        map.insert(id.as_str().to_string(), updated.clone());
        Ok(updated)
    }

    #[must_use]
    pub fn set_fail_next_update(&self, should_fail: bool) -> bool {
        self.fail_next_update.swap(should_fail, Ordering::SeqCst)
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
