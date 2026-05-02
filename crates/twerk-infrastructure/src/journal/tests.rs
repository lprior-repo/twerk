//! Tests for the journal writer and reader.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use futures_lite::StreamExt;
use fjall::{Database, KeyspaceCreateOptions};
use time::Duration;
use tokio::sync::{Barrier, Mutex};
use tokio::task;

use super::{JournalReader, JournalWriter, JournalWriterConfig};
use super::{JournalEntry, JournalEvent, SequenceNumber, Timestamp};
use crate::journal::{StepName, WorkflowId, JOURNAL_PARTITION};

fn temp_journal_path() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir should succeed")
}

#[tokio::test]
async fn test_concurrent_appends() {
    let temp_dir = temp_journal_path();
    let config = JournalWriterConfig {
        path: temp_dir.path().to_path_buf(),
        batch_size: 1000,
        channel_capacity: 10000,
    };

    let writer = Arc::new(Mutex::new(
        JournalWriter::new(config).await.expect("writer should create"),
    ));

    let num_tasks = 10;
    let entries_per_task = 100;
    let total_entries = num_tasks * entries_per_task;

    let barrier = Arc::new(Barrier::new(num_tasks));
    let start_signal = Arc::new(Barrier::new(num_tasks + 1));

    let handles: Vec<_> = (0..num_tasks)
        .map(|task_id| {
            let writer = Arc::clone(&writer);
            let barrier = Arc::clone(&barrier);
            let start_signal = Arc::clone(&start_signal);
            let workflow_id = WorkflowId::new(format!("workflow-{}", task_id));

            task::spawn(async move {
                barrier.wait().await;

                for entry_num in 0..entries_per_task {
                    let mut writer_guard = writer.lock().await;
                    writer_guard
                        .workflow_started(workflow_id.clone(), vec![entry_num as u8])
                        .await
                        .expect("workflow_started should succeed");
                }

                start_signal.wait().await;
            })
        })
        .collect();

    barrier.wait().await;
    start_signal.wait().await;

    for handle in handles {
        handle.await.expect("task should not panic");
    }

    drop(writer);

    let reader = JournalReader::open(temp_dir.path()).await.expect("reader should open");
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(
        entries.len(),
        total_entries,
        "should have exactly {total_entries} entries, got {}",
        entries.len()
    );

    for task_id in 0..num_tasks {
        let workflow_id = WorkflowId::new(format!("workflow-{}", task_id));
        let task_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.event.workflow_id() == &workflow_id)
            .collect();

        assert_eq!(
            task_entries.len(),
            entries_per_task,
            "task {task_id} should have {entries_per_task} entries"
        );

        let payloads: Vec<Vec<u8>> = task_entries
            .iter()
            .map(|e| {
                if let super::JournalEvent::WorkflowStarted { input, .. } = &e.event {
                    input.clone()
                } else {
                    panic!("expected WorkflowStarted event")
                }
            })
            .collect();

        for (i, payload) in payloads.iter().enumerate() {
            assert_eq!(
                payload.len(),
                1,
                "task {task_id} entry {i} should have 1 byte payload"
            );
            assert_eq!(
                payload[0], i as u8,
                "task {task_id} entry {i} should have value {i}, got {}",
                payload[0]
            );
        }
    }
}

#[tokio::test]
async fn test_concurrent_mixed_events() {
    let temp_dir = temp_journal_path();
    let config = JournalWriterConfig {
        path: temp_dir.path().to_path_buf(),
        batch_size: 50,
        channel_capacity: 5000,
    };

    let writer = Arc::new(Mutex::new(
        JournalWriter::new(config).await.expect("writer should create"),
    ));

    let num_tasks = 5;
    let events_per_task = 50;
    let total_events = num_tasks * events_per_task * 2;

    let barrier = Arc::new(Barrier::new(num_tasks));
    let start_signal = Arc::new(Barrier::new(num_tasks + 1));

    let handles: Vec<_> = (0..num_tasks)
        .map(|task_id| {
            let writer = Arc::clone(&writer);
            let barrier = Arc::clone(&barrier);
            let start_signal = Arc::clone(&start_signal);
            let workflow_id = WorkflowId::new(format!("mixed-workflow-{}", task_id));

            task::spawn(async move {
                barrier.wait().await;

                for event_num in 0..events_per_task {
                    let mut writer_guard = writer.lock().await;

                    writer_guard
                        .workflow_started(workflow_id.clone(), vec![event_num as u8])
                        .await
                        .expect("workflow_started should succeed");

                    writer_guard
                        .step_completed(
                            workflow_id.clone(),
                            StepName::new(format!("step-{}", event_num)),
                            vec![event_num as u8],
                        )
                        .await
                        .expect("step_completed should succeed");
                }

                start_signal.wait().await;
            })
        })
        .collect();

    barrier.wait().await;
    start_signal.wait().await;

    for handle in handles {
        handle.await.expect("task should not panic");
    }

    drop(writer);

    let reader = JournalReader::open(temp_dir.path()).await.expect("reader should open");
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(
        entries.len(),
        total_events,
        "should have exactly {total_events} entries, got {}",
        entries.len()
    );

    for task_id in 0..num_tasks {
        let workflow_id = WorkflowId::new(format!("mixed-workflow-{}", task_id));
        let task_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.event.workflow_id() == &workflow_id)
            .collect();

        assert_eq!(
            task_entries.len(),
            events_per_task * 2,
            "task {task_id} should have {} entries (started + step_completed per event)",
            events_per_task * 2
        );
    }
}

#[tokio::test]
async fn test_journal_writer_commit_does_not_panic() {
    let temp_dir = tempfile::tempdir().expect("tempdir should succeed");
    let config = JournalWriterConfig {
        path: temp_dir.path().to_path_buf(),
        batch_size: 100,
        channel_capacity: 1000,
    };
    let writer = JournalWriter::new(config).await.expect("writer should create");

    let workflow_id = WorkflowId::new("test-workflow");
    for i in 0..10 {
        writer
            .workflow_started(workflow_id.clone(), vec![i as u8])
            .await
            .expect("workflow_started should succeed");
    }

    writer.commit().await.expect("commit should succeed");
    drop(writer);
}

fn write_entry_direct(
    db: &Database,
    seq: SequenceNumber,
    ts: Timestamp,
    event: JournalEvent,
) {
    let keyspace = db
        .keyspace(JOURNAL_PARTITION, || KeyspaceCreateOptions::default())
        .unwrap();
    let entry = JournalEntry { seq, ts, event };
    let encoded = postcard::to_allocvec(&entry).unwrap();
    let key = seq.0.to_le_bytes().to_vec();
    keyspace.insert(key, encoded).unwrap();
}

#[tokio::test]
async fn test_journal_reader_replays_in_chronological_order() {
    let temp_dir = temp_journal_path();
    let db = Database::builder(temp_dir.path()).open().unwrap();
    let workflow_id = WorkflowId::new("chronological-test-workflow");

    let base_dt = time::OffsetDateTime::now_utc();

    for i in 0..10u64 {
        let ts_dt = base_dt + Duration::seconds(i as i64);
        let ts = Timestamp::from_offsetdatetime(ts_dt);
        let entry_num = i as u8;
        write_entry_direct(
            &db,
            SequenceNumber(i),
            ts,
            JournalEvent::WorkflowStarted {
                workflow_id: workflow_id.clone(),
                input: vec![entry_num],
            },
        );
    }

    drop(db);

    let reader = JournalReader::open(temp_dir.path()).await.expect("reader should open");
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(entries.len(), 10, "should have 10 entries");

    for i in 0..10 {
        assert_eq!(
            entries[i].seq,
            SequenceNumber(i as u64),
            "entry {} should have seq {}",
            i,
            i
        );
        let expected_ts_dt = base_dt + Duration::seconds(i as i64);
        let expected_ts = Timestamp::from_offsetdatetime(expected_ts_dt);
        assert_eq!(
            entries[i].ts, expected_ts,
            "entry {} should have ts {:?}, got {:?}",
            i,
            expected_ts,
            entries[i].ts
        );
    }

    let is_chronological = entries
        .windows(2)
        .all(|w| w[0].ts <= w[1].ts);
    assert!(
        is_chronological,
        "replay should return events in chronological (timestamp) order"
    );
}

#[tokio::test]
async fn test_journal_reader_out_of_order_timestamp_still_returns_in_seq_order() {
    let temp_dir = temp_journal_path();
    let db = Database::builder(temp_dir.path()).open().unwrap();
    let workflow_id = WorkflowId::new("ooto-test-workflow");

    let base_dt = time::OffsetDateTime::now_utc();

    write_entry_direct(
        &db,
        SequenceNumber(0),
        Timestamp::from_offsetdatetime(base_dt + Duration::seconds(5)),
        JournalEvent::WorkflowStarted {
            workflow_id: workflow_id.clone(),
            input: vec![0],
        },
    );
    write_entry_direct(
        &db,
        SequenceNumber(1),
        Timestamp::from_offsetdatetime(base_dt + Duration::seconds(1)),
        JournalEvent::WorkflowStarted {
            workflow_id: workflow_id.clone(),
            input: vec![1],
        },
    );
    write_entry_direct(
        &db,
        SequenceNumber(2),
        Timestamp::from_offsetdatetime(base_dt + Duration::seconds(3)),
        JournalEvent::WorkflowStarted {
            workflow_id: workflow_id.clone(),
            input: vec![2],
        },
    );

    drop(db);

    let reader = JournalReader::open(temp_dir.path()).await.expect("reader should open");
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(entries.len(), 3, "should have 3 entries");

    let seq_order = entries
        .windows(2)
        .all(|w| w[0].seq < w[1].seq);
    assert!(
        seq_order,
        "replay should return events in sequence number order regardless of timestamps"
    );

    assert_eq!(entries[0].seq, SequenceNumber(0));
    assert_eq!(entries[1].seq, SequenceNumber(1));
    assert_eq!(entries[2].seq, SequenceNumber(2));
}

#[tokio::test]
async fn test_journal_reader_no_native_seek_by_timestamp() {
    let temp_dir = temp_journal_path();
    let db = Database::builder(temp_dir.path()).open().unwrap();
    let workflow_id = WorkflowId::new("seek-test-workflow");

    let base_dt = time::OffsetDateTime::now_utc();

    for i in 0..5u64 {
        let ts_dt = base_dt + Duration::seconds(i as i64);
        write_entry_direct(
            &db,
            SequenceNumber(i),
            Timestamp::from_offsetdatetime(ts_dt),
            JournalEvent::WorkflowStarted {
                workflow_id: workflow_id.clone(),
                input: vec![i as u8],
            },
        );
    }

    drop(db);

    let reader = JournalReader::open(temp_dir.path()).await.expect("reader should open");

    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();
    assert_eq!(entries.len(), 5, "replay should return all 5 entries");

    let seek_target_dt = base_dt + Duration::seconds(2);
    let seek_target_ts = Timestamp::from_offsetdatetime(seek_target_dt);

    let entries_after_seek: Vec<_> = entries
        .iter()
        .filter(|e| e.ts > seek_target_ts)
        .cloned()
        .collect();

    assert_eq!(
        entries_after_seek.len(),
        2,
        "there are 2 entries with ts > seek_target_ts (seq 3, 4)"
    );

    for entry in &entries_after_seek {
        assert!(
            entry.ts > seek_target_ts,
            "entry seq {:?} has ts {:?}, should be > {:?}",
            entry.seq,
            entry.ts,
            seek_target_ts
        );
    }
}

#[tokio::test]
async fn test_journal_reader_skips_corrupt_entries() {
    let temp_dir = temp_journal_path();
    let db = Database::builder(temp_dir.path()).open().unwrap();
    let keyspace = db
        .keyspace(JOURNAL_PARTITION, || KeyspaceCreateOptions::default())
        .unwrap();

    let workflow_a = WorkflowId::new("workflow-a");
    let workflow_b = WorkflowId::new("workflow-b");
    let workflow_c = WorkflowId::new("workflow-c");
    let base_ts = time::OffsetDateTime::now_utc();
    let base_timestamp = Timestamp::from_offsetdatetime(base_ts);

    write_entry_direct(
        &db,
        SequenceNumber(0),
        base_timestamp,
        JournalEvent::WorkflowStarted {
            workflow_id: workflow_a.clone(),
            input: vec![0xA],
        },
    );
    write_entry_direct(
        &db,
        SequenceNumber(1),
        base_timestamp,
        JournalEvent::WorkflowStarted {
            workflow_id: workflow_b.clone(),
            input: vec![0xB],
        },
    );

    let corrupt_key = SequenceNumber(2).0.to_le_bytes().to_vec();
    let corrupt_value = vec![0xFF, 0xFE, 0xFD];
    keyspace.insert(corrupt_key.clone(), corrupt_value).unwrap();

    write_entry_direct(
        &db,
        SequenceNumber(3),
        base_timestamp,
        JournalEvent::WorkflowStarted {
            workflow_id: workflow_c.clone(),
            input: vec![0xC],
        },
    );

    drop(keyspace);
    drop(db);

    let reader = JournalReader::open(temp_dir.path()).await.expect("reader should open");
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(
        entries.len(),
        3,
        "should have exactly 3 valid entries (corrupt entry skipped), got {}",
        entries.len()
    );

    let entry_0 = entries
        .iter()
        .find(|e| e.seq == SequenceNumber(0))
        .expect("entry A (seq 0) should be present");
    if let JournalEvent::WorkflowStarted { workflow_id, input } = &entry_0.event {
        assert_eq!(workflow_id, &workflow_a);
        assert_eq!(input, &[0xA]);
    } else {
        panic!("expected WorkflowStarted for entry A");
    }

    let entry_1 = entries
        .iter()
        .find(|e| e.seq == SequenceNumber(1))
        .expect("entry B (seq 1) should be present");
    if let JournalEvent::WorkflowStarted { workflow_id, input } = &entry_1.event {
        assert_eq!(workflow_id, &workflow_b);
        assert_eq!(input, &[0xB]);
    } else {
        panic!("expected WorkflowStarted for entry B");
    }

    let entry_3 = entries
        .iter()
        .find(|e| e.seq == SequenceNumber(3))
        .expect("entry C (seq 3) should be present");
    if let JournalEvent::WorkflowStarted { workflow_id, input } = &entry_3.event {
        assert_eq!(workflow_id, &workflow_c);
        assert_eq!(input, &[0xC]);
    } else {
        panic!("expected WorkflowStarted for entry C");
    }

    let corrupt_present = entries.iter().any(|e| e.seq == SequenceNumber(2));
    assert!(
        !corrupt_present,
        "corrupt entry (seq 2) should NOT be present in replayed entries"
    );
}