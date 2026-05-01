//! Journal event types for workflow lifecycle.
//!
//! All events are serialized with postcard for compact binary encoding.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{SlotId, StepName, WorkflowId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Timestamp(i64);

impl Timestamp {
    #[must_use]
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc().unix_timestamp_nanos() as i64)
    }

    #[must_use]
    pub fn from_offsetdatetime(dt: OffsetDateTime) -> Self {
        Self(dt.unix_timestamp_nanos() as i64)
    }

    #[must_use]
    pub fn to_offsetdatetime(self) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp_nanos(self.0 as i128)
            .expect("valid unix timestamp")
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(i64::deserialize(deserializer)?))
    }
}

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
    pub ts: Timestamp,
    pub event: JournalEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_serde() {
        let ts = Timestamp::now();
        let encoded = postcard::to_allocvec(&ts).unwrap();
        let decoded: Timestamp = postcard::from_bytes(&encoded).unwrap();
        assert_eq!(ts, decoded);
    }

    #[test]
    fn test_sequence_number_serde() {
        let seq = SequenceNumber(42);
        let encoded = postcard::to_allocvec(&seq).unwrap();
        let decoded: SequenceNumber = postcard::from_bytes(&encoded).unwrap();
        assert_eq!(seq, decoded);
    }

    #[test]
    fn test_journal_event_serde() {
        let event = JournalEvent::WorkflowStarted {
            workflow_id: WorkflowId::new("test-workflow"),
            input: vec![1, 2, 3],
        };
        let encoded = postcard::to_allocvec(&event).unwrap();
        let decoded: JournalEvent = postcard::from_bytes(&encoded).unwrap();
        assert!(matches!(decoded, JournalEvent::WorkflowStarted { .. }));
    }

    #[test]
    fn test_journal_entry_serde() {
        let entry = JournalEntry {
            seq: SequenceNumber(42),
            ts: Timestamp::now(),
            event: JournalEvent::WorkflowStarted {
                workflow_id: WorkflowId::new("test-workflow"),
                input: vec![1, 2, 3],
            },
        };
        let encoded = postcard::to_allocvec(&entry).unwrap();
        let decoded: JournalEntry = postcard::from_bytes(&encoded).unwrap();
        assert_eq!(entry.seq, decoded.seq);
        assert_eq!(entry.ts, decoded.ts);
    }
}