//! Journal reader for replaying workflow events.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use fjall::{Database, KeyspaceCreateOptions};
use futures_lite::Stream;
use tracing::debug;

use super::events::{JournalEntry, SequenceNumber};
use super::{WorkflowId, JOURNAL_PARTITION};

pub struct JournalReader {
    db: Arc<Database>,
}

impl JournalReader {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let db = Database::builder(&path).open()?;
        Ok(Self { db: db.into() })
    }

    pub fn replay(&self) -> impl Stream<Item = Result<JournalEntry>> + '_ {
        debug!("starting journal replay");

        let db = self.db.clone();
        let entries: Vec<Result<JournalEntry>> = (move || {
            let keyspace = match db.keyspace(JOURNAL_PARTITION, KeyspaceCreateOptions::default) {
                Ok(ks) => ks,
                Err(e) => {
                    return vec![Err(anyhow::anyhow!("failed to open keyspace: {}", e))];
                }
            };

            let mut entries = Vec::new();
            for guard in keyspace.range::<Vec<u8>, _>(..) {
                match guard.into_inner() {
                    Ok((_key, value)) => match postcard::from_bytes::<JournalEntry>(&value) {
                        Ok(entry) => entries.push(Ok(entry)),
                        Err(e) => {
                            entries.push(Err(anyhow::anyhow!("failed to deserialize: {}", e)))
                        }
                    },
                    Err(e) => entries.push(Err(anyhow::anyhow!("guard error: {}", e))),
                }
            }
            entries
        })();

        futures_lite::stream::iter(entries)
    }

    pub fn replay_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> impl Stream<Item = Result<JournalEntry>> + '_ {
        debug!("starting workflow replay");

        let db = self.db.clone();
        let wid = workflow_id.clone();
        let entries: Vec<Result<JournalEntry>> = (move || {
            let keyspace = match db.keyspace(JOURNAL_PARTITION, KeyspaceCreateOptions::default) {
                Ok(ks) => ks,
                Err(e) => {
                    return vec![Err(anyhow::anyhow!("failed to open keyspace: {}", e))];
                }
            };

            let mut entries = Vec::new();
            for guard in keyspace.range::<Vec<u8>, _>(..) {
                match guard.into_inner() {
                    Ok((_key, value)) => match postcard::from_bytes::<JournalEntry>(&value) {
                        Ok(entry) => {
                            if entry.event.workflow_id() == &wid {
                                entries.push(Ok(entry));
                            }
                        }
                        Err(e) => {
                            entries.push(Err(anyhow::anyhow!("failed to deserialize: {}", e)))
                        }
                    },
                    Err(e) => entries.push(Err(anyhow::anyhow!("guard error: {}", e))),
                }
            }
            entries
        })();

        futures_lite::stream::iter(entries)
    }

    #[must_use]
    pub fn latest_seq(&self) -> Option<SequenceNumber> {
        let keyspace = match self.db.keyspace(JOURNAL_PARTITION, KeyspaceCreateOptions::default) {
            Ok(ks) => ks,
            Err(_) => return None,
        };

        let mut max_seq = None;
        for guard in keyspace.range::<Vec<u8>, _>(..) {
            if let Ok((key, _)) = guard.into_inner() {
                if key.len() >= 8 {
                    let bytes: [u8; 8] = key.as_ref()[..8].try_into().unwrap_or([0u8; 8]);
                    let seq = SequenceNumber(u64::from_le_bytes(bytes));
                    max_seq = Some(
                        max_seq
                            .map(|m| if seq > m { seq } else { m })
                            .unwrap_or(seq),
                    );
                }
            }
        }
        max_seq
    }
}