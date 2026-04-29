//! Durable journal for workflow lifecycle events.
//!
//! This module provides a journal that records workflow events to a Fjall
//! LSM-tree storage for durability and replay capability.

mod events;
mod reader;
mod writer;

pub use events::{JournalEntry, JournalEvent, SequenceNumber};
pub use reader::JournalReader;
pub use writer::{JournalWriter, JournalWriterConfig};

use serde::{Deserialize, Serialize};

pub const JOURNAL_PARTITION: &str = "workflow-journal";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlotId(pub u64);

impl SlotId {
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub String);

impl WorkflowId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepName(pub String);

impl StepName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}