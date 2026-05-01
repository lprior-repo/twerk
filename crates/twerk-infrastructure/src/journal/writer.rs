//! Journal writer with batched writes via tokio::mpsc.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use fjall::{Database, Keyspace, KeyspaceCreateOptions, PersistMode};
use tokio::sync::mpsc;
use tokio::task;
use tracing::{debug, error, instrument};

use super::events::{JournalEntry, JournalEvent, SequenceNumber, Timestamp};
use super::{SlotId, StepName, WorkflowId, JOURNAL_PARTITION};

#[derive(Debug, Clone)]
pub struct JournalWriterConfig {
    pub path: PathBuf,
    pub batch_size: usize,
    pub channel_capacity: usize,
}

impl Default for JournalWriterConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("./journal"),
            batch_size: 100,
            channel_capacity: 1000,
        }
    }
}

pub struct JournalWriter {
    tx: mpsc::Sender<JournalEvent>,
    #[allow(dead_code)]
    db: Arc<Database>,
    #[allow(dead_code)]
    keyspace: Keyspace,
}

impl JournalWriter {
    #[instrument]
    pub async fn new(config: JournalWriterConfig) -> Result<Self> {
        let db = Database::builder(&config.path).open()?;

        let keyspace = db.keyspace(JOURNAL_PARTITION, KeyspaceCreateOptions::default)?;

        let (tx, rx) = mpsc::channel(config.channel_capacity);

        let writer_db: Arc<Database> = db.clone().into();
        let writer_keyspace = keyspace.clone();

        task::spawn(async move {
            if let Err(e) =
                Self::write_loop(rx, writer_db, writer_keyspace, config.batch_size).await
            {
                error!(error = %e, "journal write loop terminated");
            }
        });

        Ok(Self { tx, db: db.into(), keyspace })
    }

    async fn write_loop(
        mut rx: mpsc::Receiver<JournalEvent>,
        _db: Arc<Database>,
        keyspace: Keyspace,
        batch_size: usize,
    ) -> Result<()> {
        let mut current_seq = SequenceNumber(0);
        let mut batch_keys: Vec<Vec<u8>> = Vec::with_capacity(batch_size);
        let mut batch_values: Vec<Vec<u8>> = Vec::with_capacity(batch_size);

        loop {
            match rx.recv().await {
                Some(event) => {
                    current_seq = SequenceNumber::next(Some(current_seq));
                    let entry = JournalEntry {
                        seq: current_seq,
                        ts: Timestamp::now(),
                        event,
                    };
                    let encoded = postcard::to_allocvec(&entry)?;
                    let key = current_seq.0.to_le_bytes().to_vec();
                    batch_keys.push(key);
                    batch_values.push(encoded);

                    if batch_keys.len() >= batch_size {
                        Self::flush_batch(&keyspace, &mut batch_keys, &mut batch_values)?;
                    }
                }
                None => {
                    if !batch_keys.is_empty() {
                        Self::flush_batch(&keyspace, &mut batch_keys, &mut batch_values)?;
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    fn flush_batch(
        keyspace: &Keyspace,
        batch_keys: &mut Vec<Vec<u8>>,
        batch_values: &mut Vec<Vec<u8>>,
    ) -> Result<()> {
        if batch_keys.is_empty() {
            return Ok(());
        }

        let count = batch_keys.len();
        for (key, value) in batch_keys.drain(..).zip(batch_values.drain(..)) {
            keyspace.insert(key, value)?;
        }

        debug!(count, "flushed journal batch");
        Ok(())
    }

    pub async fn workflow_started(&self, workflow_id: WorkflowId, input: Vec<u8>) -> Result<()> {
        self.tx
            .send(JournalEvent::WorkflowStarted { workflow_id, input })
            .await?;
        Ok(())
    }

    pub async fn step_completed(
        &self,
        workflow_id: WorkflowId,
        step: StepName,
        output: Vec<u8>,
    ) -> Result<()> {
        self.tx
            .send(JournalEvent::StepCompleted {
                workflow_id,
                step,
                output,
            })
            .await?;
        Ok(())
    }

    pub async fn step_failed(
        &self,
        workflow_id: WorkflowId,
        step: StepName,
        error: String,
    ) -> Result<()> {
        self.tx
            .send(JournalEvent::StepFailed {
                workflow_id,
                step,
                error,
            })
            .await?;
        Ok(())
    }

    pub async fn slot_updated(
        &self,
        workflow_id: WorkflowId,
        slot_id: SlotId,
        data: Vec<u8>,
    ) -> Result<()> {
        self.tx
            .send(JournalEvent::SlotUpdated {
                workflow_id,
                slot_id,
                data,
            })
            .await?;
        Ok(())
    }

    pub async fn workflow_completed(&self, workflow_id: WorkflowId, output: Vec<u8>) -> Result<()> {
        self.tx
            .send(JournalEvent::WorkflowCompleted { workflow_id, output })
            .await?;
        Ok(())
    }

    #[must_use]
    pub fn current_seq(&self) -> SequenceNumber {
        SequenceNumber(0)
    }

    pub async fn commit(&self) -> Result<()> {
        self.db.persist(PersistMode::SyncAll)?;
        Ok(())
    }
}