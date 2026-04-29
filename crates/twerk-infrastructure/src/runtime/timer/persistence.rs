//! Fjall-based persistence for timers.
//!
//! Persists timer entries to Fjall LSM-tree storage so they survive restarts.
//! On startup, timers are loaded and expired timers are fired.

use anyhow::{Context, Result};
use fjall::{Config, Keyspace, PartitionHandle};
use std::path::Path;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use super::entry::{TimerEntry, TimerId, TimerState, TimerVariant};
use super::registry::{SignalRegistry, SignalRegistryError, TimerSignal};

const TIMER_PARTITION: &str = "timers";
const SIGNAL_PARTITION: &str = "signals";

pub struct TimerPersistence {
    timers: PartitionHandle,
    signals: PartitionHandle,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SignalData {
    job_id: String,
    task_id: String,
    registered_at: OffsetDateTime,
}

impl TimerPersistence {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        info!(path = %path.display(), "Opening timer persistence");

        let keyspace: Keyspace = Config::new(path).open().context("Failed to open Fjall keyspace")?;

        let timers = keyspace
            .open_partition(TIMER_PARTITION, fjall::PartitionCreateOptions::default())
            .context("Failed to create timers partition")?;

        let signals = keyspace
            .open_partition(SIGNAL_PARTITION, fjall::PartitionCreateOptions::default())
            .context("Failed to create signals partition")?;

        Ok(Self {
            timers,
            signals,
        })
    }

    pub async fn save_timer(&self, entry: &TimerEntry) -> Result<()> {
        let key = entry.variant.timer_id().to_string();
        let value = serde_json::to_string(entry).context("Failed to serialize timer")?;
        self.timers
            .insert(&key, &value)
            .context("Failed to insert timer")?;
        debug!(timer_id = %key, "Persisted timer");
        Ok(())
    }

    pub async fn get_timer(&self, timer_id: &TimerId) -> Result<Option<TimerEntry>> {
        let key = timer_id.to_string();
        match self.timers.get(&key) {
            Ok(Some(value)) => {
                let entry: TimerEntry =
                    serde_json::from_str(std::str::from_utf8(&*value)?)
                        .context("Failed to deserialize timer")?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e).context("Failed to get timer"),
        }
    }

    pub async fn delete_timer(&self, timer_id: &TimerId) -> Result<()> {
        let key = timer_id.to_string();
        self.timers
            .remove(&key)
            .context("Failed to delete timer")?;
        debug!(timer_id = %timer_id, "Deleted timer");
        Ok(())
    }

    pub async fn list_timers(&self) -> Result<Vec<TimerEntry>> {
        let mut entries = Vec::new();
        for item in self.timers.iter() {
            if let Ok((_key, value)) = item {
                if let Ok(entry) = serde_json::from_str::<TimerEntry>(std::str::from_utf8(&*value)?) {
                    entries.push(entry);
                } else {
                    error!(value = %String::from_utf8_lossy(&*value), "Failed to deserialize timer entry");
                }
            }
        }
        Ok(entries)
    }

    pub async fn update_timer_state(
        &self,
        timer_id: &TimerId,
        new_state: TimerState,
    ) -> Result<()> {
        if let Some(mut entry) = self.get_timer(timer_id).await? {
            entry.state = new_state;
            self.save_timer(&entry).await?;
        }
        Ok(())
    }

    pub async fn get_expired_timers(&self) -> Result<Vec<TimerEntry>> {
        let now = OffsetDateTime::now_utc();
        let all_timers = self.list_timers().await?;
        let expired: Vec<TimerEntry> = all_timers
            .into_iter()
            .filter(|e| {
                e.state == TimerState::Pending
                    || e.state == TimerState::Active
                    || e.state == TimerState::Firing
            })
            .filter(|e| {
                if let Some(exp_time) = e.variant.expiration_time() {
                    exp_time <= now
                } else {
                    false
                }
            })
            .collect();
        Ok(expired)
    }
}

#[derive(Default)]
pub struct InMemorySignalRegistry {
    waiters: RwLock<std::collections::HashMap<String, SignalData>>,
}

impl InMemorySignalRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            waiters: RwLock::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl SignalRegistry for InMemorySignalRegistry {
    async fn register_waiter(
        &self,
        signal_id: &str,
        job_id: &str,
        task_id: &str,
    ) -> std::result::Result<(), SignalRegistryError> {
        let data = SignalData {
            job_id: job_id.to_owned(),
            task_id: task_id.to_owned(),
            registered_at: OffsetDateTime::now_utc(),
        };
        let mut waiters = self.waiters.write().await;
        waiters.insert(signal_id.to_owned(), data);
        debug!(signal_id, job_id, task_id, "Registered signal waiter");
        Ok(())
    }

    async fn unregister_waiter(
        &self,
        signal_id: &str,
    ) -> std::result::Result<(), SignalRegistryError> {
        let mut waiters = self.waiters.write().await;
        waiters.remove(signal_id);
        debug!(signal_id, "Unregistered signal waiter");
        Ok(())
    }

    async fn send_signal(
        &self,
        signal: TimerSignal,
    ) -> std::result::Result<(), SignalRegistryError> {
        let waiters = self.waiters.read().await;
        if let Some(data) = waiters.get(&signal.signal_id) {
            info!(
                signal_id = %signal.signal_id,
                timer_id = %signal.timer_id,
                job_id = %signal.job_id,
                task_id = %signal.task_id,
                is_timeout = signal.is_timeout,
                "Timer signal fired"
            );
            Ok(())
        } else {
            Err(SignalRegistryError::SignalNotFound(signal.signal_id))
        }
    }

    async fn is_registered(&self, signal_id: &str) -> bool {
        let waiters = self.waiters.read().await;
        waiters.contains_key(signal_id)
    }

    async fn get_job_signals(&self, job_id: &str) -> Vec<String> {
        let waiters = self.waiters.read().await;
        waiters
            .iter()
            .filter(|(_, data)| data.job_id == job_id)
            .map(|(id, _)| id.clone())
            .collect()
    }
}