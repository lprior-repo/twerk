//! Tests for the journal writer.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use tokio::sync::{Barrier, Mutex};
use tokio::task;

use super::{JournalReader, JournalWriter, JournalWriterConfig};
use crate::journal::{StepName, WorkflowId};

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
        handle.expect("task should not panic");
    }

    drop(writer);

    let reader = JournalReader::open(temp_dir.path()).expect("reader should open");
    let entries: Vec<_> = reader.replay().collect::<Result<Vec<_>, _>>().await.unwrap();

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

        let payloads: Vec<u8> = task_entries
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
        handle.expect("task should not panic");
    }

    drop(writer);

    let reader = JournalReader::open(temp_dir.path()).expect("reader should open");
    let entries: Vec<_> = reader.replay().collect::<Result<Vec<_>, _>>().await.unwrap();

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