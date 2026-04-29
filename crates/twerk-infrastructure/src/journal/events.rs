//! Journal event types for workflow lifecycle.
//!
//! All events are serialized with postcard for compact binary encoding.

use serde::{Deserialize, Serialize};

use super::{SlotId, StepName, WorkflowId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

impl SequenceNumber {
    #[must_use]
    pub fn next(current: Option<SequenceNumber>) -> Self {
        match current {
            None => Self(0),
            Some(SequenceNumber(n)) => Self(n.saturating_add(1)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub seq: SequenceNumber,
    pub ts: time::OffsetDateTime,
    pub event: JournalEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum JournalEvent {
    WorkflowStarted {
        workflow_id: WorkflowId,
        input: Vec<u8>,
    },
    StepCompleted {
        workflow_id: WorkflowId,
        step: StepName,
        output: Vec<u8>,
    },
    StepFailed {
        workflow_id: WorkflowId,
        step: StepName,
        error: String,
    },
    SlotUpdated {
        workflow_id: WorkflowId,
        slot_id: SlotId,
        data: Vec<u8>,
    },
    WorkflowCompleted {
        workflow_id: WorkflowId,
        output: Vec<u8>,
    },
}

impl JournalEvent {
    #[must_use]
    pub fn workflow_id(&self) -> &WorkflowId {
        match self {
            JournalEvent::WorkflowStarted { workflow_id, .. } => workflow_id,
            JournalEvent::StepCompleted { workflow_id, .. } => workflow_id,
            JournalEvent::StepFailed { workflow_id, .. } => workflow_id,
            JournalEvent::SlotUpdated { workflow_id, .. } => workflow_id,
            JournalEvent::WorkflowCompleted { workflow_id, .. } => workflow_id,
        }
    }
}