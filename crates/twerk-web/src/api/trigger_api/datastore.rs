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

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn make_trigger(id: &str, name: &str) -> Trigger {
        Trigger {
            id: TriggerId::from(id),
            name: name.to_string(),
            enabled: true,
            event: "test-event".to_string(),
            condition: None,
            action: "test-action".to_string(),
            metadata: HashMap::new(),
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn test_get_trigger_by_id_returns_trigger_when_exists() {
        let ds = InMemoryTriggerDatastore::new();
        let trigger = make_trigger("test-id", "Test Trigger");
        ds.upsert(trigger.clone());

        let result = ds.get_trigger_by_id(&trigger.id);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        assert_eq!(result.unwrap().name, "Test Trigger");
    }

    #[test]
    fn test_get_trigger_by_id_returns_not_found_for_nonexistent() {
        let ds = InMemoryTriggerDatastore::new();
        let id = TriggerId::from("nonexistent-id");

        let result = ds.get_trigger_by_id(&id);
        assert!(matches!(
            result,
            Err(TriggerUpdateError::TriggerNotFound(_))
        ));
    }

    #[test]
    fn test_upsert_creates_new_trigger() {
        let ds = InMemoryTriggerDatastore::new();
        let trigger = make_trigger("new-trigger", "New Trigger");

        ds.upsert(trigger.clone());
        let result = ds.get_trigger_by_id(&trigger.id);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "New Trigger");
    }

    #[test]
    fn test_upsert_updates_existing_trigger() {
        let ds = InMemoryTriggerDatastore::new();
        let trigger = make_trigger("existing-trigger", "Original Name");
        ds.upsert(trigger);

        let updated_trigger = make_trigger("existing-trigger", "Updated Name");
        ds.upsert(updated_trigger);

        let result = ds.get_trigger_by_id(&TriggerId::from("existing-trigger"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "Updated Name");
    }

    #[test]
    fn test_update_trigger_successfully_updates_trigger() {
        let ds = InMemoryTriggerDatastore::new();
        let trigger = make_trigger("update-test", "Before Update");
        ds.upsert(trigger);

        let updated = ds.update_trigger(
            &TriggerId::from("update-test"),
            Box::new(|current| {
                Ok(Trigger {
                    name: "After Update".to_string(),
                    ..current
                })
            }),
        );

        assert!(updated.is_ok(), "Expected Ok, got {:?}", updated);
        assert_eq!(updated.unwrap().name, "After Update");
    }

    #[test]
    fn test_update_trigger_returns_not_found_for_nonexistent() {
        let ds = InMemoryTriggerDatastore::new();
        let id = TriggerId::from("nonexistent");

        let result = ds.update_trigger(&id, Box::new(|current| Ok(current)));

        assert!(matches!(
            result,
            Err(TriggerUpdateError::TriggerNotFound(_))
        ));
    }

    #[test]
    fn test_update_trigger_fails_when_fail_next_update_is_set() {
        let ds = InMemoryTriggerDatastore::new();
        let trigger = make_trigger("fail-test", "Fail Test");
        ds.upsert(trigger);

        let _ = ds.set_fail_next_update(true);

        let result = ds.update_trigger(
            &TriggerId::from("fail-test"),
            Box::new(|current| Ok(current)),
        );

        assert!(matches!(result, Err(TriggerUpdateError::Persistence(_))));
    }

    #[test]
    fn test_set_fail_next_update_returns_previous_value() {
        let ds = InMemoryTriggerDatastore::new();

        assert_eq!(ds.set_fail_next_update(false), false);
        assert_eq!(ds.set_fail_next_update(true), false);
        assert_eq!(ds.set_fail_next_update(false), true);
    }

    #[test]
    fn test_update_trigger_closure_can_return_error() {
        let ds = InMemoryTriggerDatastore::new();
        let trigger = make_trigger("error-test", "Error Test");
        ds.upsert(trigger);

        let result = ds.update_trigger(
            &TriggerId::from("error-test"),
            Box::new(|_current| {
                Err(TriggerUpdateError::ValidationFailed(
                    "intentional error".to_string(),
                ))
            }),
        );

        assert!(matches!(
            result,
            Err(TriggerUpdateError::ValidationFailed(_))
        ));
    }

    #[test]
    fn test_multiple_triggers_stored_independently() {
        let ds = InMemoryTriggerDatastore::new();
        let trigger1 = make_trigger("id-1", "Trigger One");
        let trigger2 = make_trigger("id-2", "Trigger Two");
        ds.upsert(trigger1);
        ds.upsert(trigger2);

        let result1 = ds.get_trigger_by_id(&TriggerId::from("id-1"));
        let result2 = ds.get_trigger_by_id(&TriggerId::from("id-2"));

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert_eq!(result1.unwrap().name, "Trigger One");
        assert_eq!(result2.unwrap().name, "Trigger Two");
    }
}
