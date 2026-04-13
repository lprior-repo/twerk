//! Triggers API module.
//!
//! Split into logical submodules:
//! - domain.rs: `TriggerId`, `Trigger`, `TriggerView`, `TriggerUpdateRequest` types and validation
//! - datastore.rs: `InMemoryTriggerDatastore` for testing
//! - handlers.rs: HTTP request handlers

pub mod datastore;
pub mod domain;
pub mod handlers;

// Re-exports for convenience
pub use datastore::{InMemoryTriggerDatastore, TriggerAppState};
pub use domain::{
    apply_trigger_update, validate_trigger_update, Trigger, TriggerId, TriggerUpdateError,
    TriggerUpdateRequest, TriggerView,
};
pub use domain::{
    ACTION_REQUIRED_MSG, EVENT_REQUIRED_MSG, MALFORMED_JSON_MSG, METADATA_KEY_MSG,
    NAME_REQUIRED_MSG, SERIALIZATION_MSG, TRIGGER_FIELD_MAX_LEN, TRIGGER_ID_MAX_LEN,
    UPDATED_AT_BACKWARDS_MSG,
};
pub use handlers::update_trigger_handler;
