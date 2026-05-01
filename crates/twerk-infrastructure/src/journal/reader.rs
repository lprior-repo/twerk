//! Journal reader for replaying workflow events.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use fjall::{Database, KeyspaceCreateOptions};
use futures_lite::Stream;
use tracing::{debug, warn};

use super::events::{JournalEntry, SequenceNumber};
use super::{WorkflowId, JOURNAL_PARTITION};

pub struct JournalReader {
    db: Arc<Database>,
    entries: RefCell<Vec<JournalEntry>>,
    position: RefCell<usize>,
}

impl JournalReader {
    pub async fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let db = Database::builder(&path).open()?;
        Ok(Self {
            db: db.into(),
            entries: RefCell::new(Vec::new()),
            position: RefCell::new(0),
        })
    }

    fn load_entries(&self) -> Result<()> {
        let mut entries = self.entries.borrow_mut();
        if !entries.is_empty() {
            return Ok(());
        }

        let keyspace = match self.db.keyspace(JOURNAL_PARTITION, KeyspaceCreateOptions::default) {
            Ok(ks) => ks,
            Err(e) => return Err(anyhow::anyhow!("failed to open keyspace: {}", e)),
        };

        for guard in keyspace.range::<Vec<u8>, _>(..) {
            match guard.into_inner() {
                Ok((key, value)) => match postcard::from_bytes::<JournalEntry>(&value) {
                    Ok(entry) => entries.push(entry),
                    Err(e) => {
                        warn!(key = ?key, error = %e, "skipping corrupt journal entry during load");
                    }
                },
                Err(e) => {
                    warn!(error = %e, "guard error during journal load");
                }
            }
        }

        entries.sort_by_key(|e| e.ts);
        *self.position.borrow_mut() = 0;
        Ok(())
    }

    pub fn seek_to(&self, timestamp_ms: i64) -> bool {
        if let Err(e) = self.load_entries() {
            tracing::debug!("seek_to failed to load entries: {}", e);
            return false;
        }

        let entries = self.entries.borrow().clone();
        let mut position = self.position.borrow_mut();

        if entries.is_empty() {
            return false;
        }

        for (i, entry) in entries.iter().enumerate() {
            if entry.ts.to_offsetdatetime().unix_timestamp_nanos() / 1_000_000 >= timestamp_ms as i128 {
                *position = i;
                return true;
            }
        }
        false
    }

    pub fn next(&self) -> Option<JournalEntry> {
        let mut position = self.position.borrow_mut();
        let entries = self.entries.borrow();

        if *position >= entries.len() {
            return None;
        }

        let entry = entries[*position].clone();
        *position += 1;
        Some(entry)
    }

    pub fn replay(&self) -> impl Stream<Item = Result<JournalEntry>> + '_ {
        debug!("starting journal replay");

        let needs_load = self.entries.borrow().is_empty();
        if needs_load {
            if let Err(e) = self.load_entries() {
                return futures_lite::stream::iter(vec![Err(e)]);
            }
        }

        *self.position.borrow_mut() = 0;
        let entries = self.entries.borrow().clone();
        let results: Vec<Result<JournalEntry>> = entries.into_iter().map(Ok).collect();
        futures_lite::stream::iter(results)
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
                    Ok((key, value)) => match postcard::from_bytes::<JournalEntry>(&value) {
                        Ok(entry) => {
                            if entry.event.workflow_id() == &wid {
                                entries.push(Ok(entry));
                            }
                        }
                        Err(e) => {
                            warn!(key = ?key, error = %e, "skipping corrupt journal entry during workflow replay");
                        }
                    },
                    Err(e) => {
                        warn!(error = %e, "guard error during workflow journal replay");
                    }
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