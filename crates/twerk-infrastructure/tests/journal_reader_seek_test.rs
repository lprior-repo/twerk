//! Tests for JournalReader seek_to functionality.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use tempfile::TempDir;
use time::OffsetDateTime;
use twerk_infrastructure::journal::events::{JournalEntry, JournalEvent, SequenceNumber};
use twerk_infrastructure::journal::{JournalReader, JOURNAL_PARTITION};
use fjall::{Database, KeyspaceCreateOptions};

fn insert_entry(
    db: &Database,
    seq: SequenceNumber,
    timestamp_ms: i64,
    event: JournalEvent,
) {
    let keyspace = db.keyspace(JOURNAL_PARTITION, KeyspaceCreateOptions::default).unwrap();
    let ts = OffsetDateTime::from_unix_timestamp_nanos(timestamp_ms * 1_000_000).unwrap();
    let entry = JournalEntry { seq, ts, event };
    let encoded = postcard::to_allocvec(&entry).unwrap();
    let key = seq.0.to_le_bytes().to_vec();
    keyspace.insert(key, encoded).unwrap();
}

#[tokio::test]
async fn journal_reader_seek_to_timestamp() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("journal");
    let db = Database::builder(&path).open().unwrap();

    let wid = twerk_infrastructure::journal::WorkflowId::new("test-workflow");
    let slot_id = twerk_infrastructure::journal::SlotId::new(0);

    insert_entry(&db, SequenceNumber(0), 0, JournalEvent::WorkflowStarted {
        workflow_id: wid.clone(),
        input: vec![],
    });
    insert_entry(&db, SequenceNumber(1), 100, JournalEvent::SlotUpdated {
        workflow_id: wid.clone(),
        slot_id,
        data: vec![],
    });
    insert_entry(&db, SequenceNumber(2), 200, JournalEvent::StepCompleted {
        workflow_id: wid.clone(),
        step: twerk_infrastructure::journal::StepName::new("step1"),
        output: vec![],
    });
    insert_entry(&db, SequenceNumber(3), 300, JournalEvent::WorkflowCompleted {
        workflow_id: wid,
        output: vec![],
    });

    drop(db);

    let reader = JournalReader::open(&path).await.unwrap();
    let mut reader = reader;

    assert!(reader.seek_to(150), "seek_to(150) should find entry at t=200");
    let entry = reader.next();
    assert!(entry.is_some(), "next() after seek_to(150) should return Some");
    assert_eq!(entry.unwrap().ts.unix_timestamp_nanos() / 1_000_000, 200);

    assert!(reader.seek_to(0), "seek_to(0) should find entry at t=0");
    let entry = reader.next();
    assert!(entry.is_some(), "next() after seek_to(0) should return Some");
    assert_eq!(entry.unwrap().ts.unix_timestamp_nanos() / 1_000_000, 0);

    assert!(!reader.seek_to(999), "seek_to(999) should return false (past end)");
}