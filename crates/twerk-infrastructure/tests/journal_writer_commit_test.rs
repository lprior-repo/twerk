#![allow(clippy::unwrap_used)]

use std::io::Read as IoRead;
use futures_lite::StreamExt;
use tempfile::TempDir;
use twerk_infrastructure::journal::{
    JournalReader, JournalWriter, JournalWriterConfig, StepName, WorkflowId, JOURNAL_PARTITION,
};
use fjall::{Database, KeyspaceCreateOptions};

#[tokio::test]
async fn journal_writer_commit_flushes_entries_to_disk() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("journal");

    let config = JournalWriterConfig {
        path: journal_path.clone(),
        batch_size: 100,
        channel_capacity: 1000,
    };
    let writer = JournalWriter::new(config).await.unwrap();

    let workflow_id = WorkflowId::new("test-workflow");
    let entry_data: Vec<Vec<u8>> = (0..10)
        .map(|i| format!("entry-{}", i).into_bytes())
        .collect();

    for i in 0..10 {
        writer
            .step_completed(workflow_id.clone(), StepName::new(format!("step-{}", i)), entry_data[i].clone())
            .await
            .unwrap();
    }

    writer.commit().await.unwrap();

    drop(writer);

    let reader = JournalReader::open(&journal_path).await.unwrap();
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(entries.len(), 10, "Expected 10 entries after commit");

    for (i, entry) in entries.iter().enumerate() {
        let expected_seq = i as u64;
        assert_eq!(
            entry.seq.0, expected_seq,
            "Entry {} should have sequence number {}",
            i, expected_seq
        );
    }

    let db_path = journal_path.join("data.mdb");
    let metadata = std::fs::metadata(&db_path).unwrap();
    let file_size = metadata.len();
    assert!(
        file_size > 0,
        "Journal file size should be greater than 0, got {}",
        file_size
    );
}

#[tokio::test]
async fn journal_writer_entries_order_preserved_after_commit() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("journal");

    let config = JournalWriterConfig {
        path: journal_path.clone(),
        batch_size: 100,
        channel_capacity: 1000,
    };
    let writer = JournalWriter::new(config).await.unwrap();

    let workflow_id = WorkflowId::new("ordered-workflow");

    for i in 0..10 {
        let data = format!("ordered-entry-{}", i);
        writer
            .workflow_started(workflow_id.clone(), data.into_bytes())
            .await
            .unwrap();
    }

    writer.commit().await.unwrap();

    drop(writer);

    let reader = JournalReader::open(&journal_path).await.unwrap();
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(entries.len(), 10);

    let sequences: Vec<u64> = entries.iter().map(|e| e.seq.0).collect();
    let mut expected_sequences: Vec<u64> = (0..10).collect();
    assert_eq!(
        sequences, expected_sequences,
        "Sequence numbers should be in order 0..9"
    );
}

fn calculate_expected_journal_size(entry_count: usize, avg_entry_size: usize) -> usize {
    entry_count * (8 + avg_entry_size)
}

#[tokio::test]
async fn journal_writer_file_size_reasonable_after_commit() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("journal");

    let config = JournalWriterConfig {
        path: journal_path.clone(),
        batch_size: 100,
        channel_capacity: 1000,
    };
    let writer = JournalWriter::new(config).await.unwrap();

    let workflow_id = WorkflowId::new("size-test-workflow");
    let entry_data: Vec<Vec<u8>> = (0..10)
        .map(|i| format!("size-test-entry-data-{}", i).into_bytes())
        .collect();

    for i in 0..10 {
        writer
            .step_completed(workflow_id.clone(), StepName::new(format!("step-{}", i)), entry_data[i].clone())
            .await
            .unwrap();
    }

    writer.commit().await.unwrap();

    drop(writer);

    let reader = JournalReader::open(&journal_path).await.unwrap();
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    let total_data_size: usize = entries
        .iter()
        .map(|e| {
            let event_size = match &e.event {
                twerk_infrastructure::journal::JournalEvent::StepCompleted { output, .. } => output.len(),
                _ => 0,
            };
            8 + event_size
        })
        .sum();

    let db_path = journal_path.join("data.mdb");
    let metadata = std::fs::metadata(&db_path).unwrap();
    let file_size = metadata.len() as usize;

    assert!(
        file_size >= total_data_size,
        "File size {} should be at least the sum of entry sizes {}",
        file_size,
        total_data_size
    );
}

#[tokio::test]
async fn journal_writer_commit_survives_crash_simulation() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("journal");

    {
        let config = JournalWriterConfig {
            path: journal_path.clone(),
            batch_size: 100,
            channel_capacity: 1000,
        };
        let writer = JournalWriter::new(config).await.unwrap();

        let workflow_id = WorkflowId::new("crash-test-workflow");

        for i in 0..10 {
            let data = format!("crash-test-entry-{}", i);
            writer
                .workflow_started(workflow_id.clone(), data.into_bytes())
                .await
                .unwrap();
        }

        writer.commit().await.unwrap();

        drop(writer);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let reader = JournalReader::open(&journal_path).await.unwrap();
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(entries.len(), 10, "All 10 committed events should survive crash simulation");
}

#[tokio::test]
async fn journal_writer_without_commit_events_lost_on_crash() {
    let temp_dir = TempDir::new().unwrap();
    let journal_path = temp_dir.path().join("journal");

    {
        let config = JournalWriterConfig {
            path: journal_path.clone(),
            batch_size: 100,
            channel_capacity: 1000,
        };
        let writer = JournalWriter::new(config).await.unwrap();

        let workflow_id = WorkflowId::new("no-commit-workflow");

        for i in 0..10 {
            let data = format!("no-commit-entry-{}", i);
            writer
                .workflow_started(workflow_id.clone(), data.into_bytes())
                .await
                .unwrap();
        }

        drop(writer);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let reader = JournalReader::open(&journal_path).await.unwrap();
    let entries: Vec<_> = reader.replay().try_collect().await.unwrap();

    assert_eq!(
        entries.len(), 0,
        "Without commit(), events should be lost on crash (not durable)"
    );
}