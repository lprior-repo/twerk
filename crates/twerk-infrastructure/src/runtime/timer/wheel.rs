//! TimerWheel - Main timer scheduler implementation.
//!
//! Uses tokio::time for scheduling and integrates with SignalRegistry
//! for waking waiting actors when timers fire.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, instrument, warn};

use super::entry::{TimerEntry, TimerId, TimerState, TimerVariant};
use super::persistence::TimerPersistence;
use super::registry::{SignalRegistry, TimerSignal};

/// Errors that can occur during timer wheel operations.
#[derive(Debug, thiserror::Error)]
pub enum TimerWheelError {
    #[error("timer not found: {0}")]
    TimerNotFound(String),
    #[error("timer already exists: {0}")]
    TimerAlreadyExists(String),
    #[error("timer is not pending or active: {0}")]
    InvalidTimerState(String),
    #[error("failed to persist timer: {0}")]
    PersistenceError(String),
    #[error("failed to send signal: {0}")]
    SignalError(String),
    #[error("shutdown error: {0}")]
    ShutdownError(String),
}

/// Result type for timer wheel operations.
pub type TimerWheelResult<T> = std::result::Result<T, TimerWheelError>;

/// TimerWheel manages pending timers and fires them when due.
///
/// # Architecture
///
/// - Uses tokio::time for efficient timer scheduling
/// - Persists timers to Fjall for crash recovery
/// - Sends signals via SignalRegistry when timers fire
/// - On startup, checks for and fires expired timers
pub struct TimerWheel {
    persistence: Arc<TimerPersistence>,
    registry: Arc<dyn SignalRegistry>,
    timers: Arc<RwLock<HashMap<String, TimerEntry>>>,
    shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
    fire_tx: mpsc::Sender<TimerEntry>,
    fire_rx: RwLock<Option<mpsc::Receiver<TimerEntry>>>,
}

impl TimerWheel {
    /// Creates a new TimerWheel.
    ///
    /// # Errors
    ///
    /// Returns error if persistence cannot be initialized.
    pub async fn new(
        persistence: Arc<TimerPersistence>,
        registry: Arc<dyn SignalRegistry>,
    ) -> Result<Self> {
        let (fire_tx, fire_rx) = mpsc::channel(100);
        Ok(Self {
            persistence,
            registry,
            timers: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: RwLock::new(None),
            fire_tx,
            fire_rx: RwLock::new(Some(fire_rx)),
        })
    }

    /// Starts the timer wheel background task.
    ///
    /// # Errors
    ///
    /// Returns error if startup fails.
    #[instrument(name = "timer_wheel_start", skip_all)]
    pub async fn start(&self) -> Result<()> {
        info!("Starting timer wheel");
        self.recover_timers().await?;
        self.start_fire_handler().await?;
        Ok(())
    }

    /// Recovers timers from persistence and fires any expired ones.
    async fn recover_timers(&self) -> Result<()> {
        info!("Recovering timers from persistence");
        let expired = self.persistence.get_expired_timers().await?;

        if !expired.is_empty() {
            info!(count = expired.len(), "Found expired timers to fire");
            for entry in expired {
                self.fire_timer(entry).await?;
            }
        }

        let all_timers = self.persistence.list_timers().await?;
        let mut timers = self.timers.write().await;
        for entry in all_timers {
            if entry.state == TimerState::Pending || entry.state == TimerState::Active {
                timers.insert(entry.variant.timer_id().to_string(), entry);
            }
        }
        info!(count = timers.len(), "Loaded timers from persistence");
        Ok(())
    }

    /// Starts the background fire handler task.
    async fn start_fire_handler(&self) -> Result<()> {
        let mut rx = self.fire_rx.write().await;
        let receiver = rx.take().ok_or_else(|| anyhow::anyhow!("fire receiver already taken"))?;
        drop(rx);

        let registry = self.registry.clone();
        tokio::spawn(async move {
            Self::fire_handler_loop(receiver, registry).await;
        });

        Ok(())
    }

    async fn fire_handler_loop(
        mut rx: mpsc::Receiver<TimerEntry>,
        registry: Arc<dyn SignalRegistry>,
    ) {
        while let Some(entry) = rx.recv().await {
            if let Err(e) = Self::process_fire(&registry, &entry).await {
                error!(
                    timer_id = %entry.variant.timer_id(),
                    error = %e,
                    "Failed to process timer fire"
                );
            }
        }
    }

    async fn process_fire(
        registry: &Arc<dyn SignalRegistry>,
        entry: &TimerEntry,
    ) -> TimerWheelResult<()> {
        let variant = &entry.variant;
        let timer_id = variant.timer_id().to_string();
        let job_id = variant.job_id().to_string();
        let task_id = variant.task_id().to_string();

        let signal_id = match variant {
            TimerVariant::Delay(_) => timer_id.clone(),
            TimerVariant::Scheduled(_) => timer_id.clone(),
            TimerVariant::WaitFor(w) => w.signal_id.clone(),
        };

        let is_timeout = matches!(variant, TimerVariant::WaitFor(_));

        let signal = TimerSignal::new(
            signal_id,
            timer_id,
            job_id,
            task_id,
            is_timeout,
        );

        registry
            .send_signal(signal)
            .await
            .map_err(|e| TimerWheelError::SignalError(e.to_string()))?;

        Ok(())
    }

    /// Schedules a delay timer.
    ///
    /// # Errors
    ///
    /// Returns error if timer cannot be scheduled.
    pub async fn schedule_delay(
        &self,
        duration: Duration,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> TimerWheelResult<TimerId> {
        let entry =
            TimerEntry::new_delay(duration, job_id, task_id).map_err(|e| {
                TimerWheelError::PersistenceError(e.to_string())
            })?;

        let timer_id = entry.variant.timer_id().clone();
        self.add_timer(entry).await?;
        self.schedule_timer_entry(timer_id.as_str(), duration).await?;
        Ok(timer_id)
    }

    /// Schedules a wait-for timer with timeout.
    ///
    /// # Errors
    ///
    /// Returns error if timer cannot be scheduled.
    pub async fn schedule_wait_for(
        &self,
        timeout: Duration,
        signal_id: impl Into<String>,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> TimerWheelResult<TimerId> {
        let entry =
            TimerEntry::new_wait_for(timeout, signal_id, job_id, task_id).map_err(|e| {
                TimerWheelError::PersistenceError(e.to_string())
            })?;

        let timer_id = entry.variant.timer_id().clone();
        self.add_timer(entry).await?;
        self.schedule_timer_entry(timer_id.as_str(), timeout).await?;
        Ok(timer_id)
    }

    /// Schedules a cron-based timer.
    ///
    /// # Errors
    ///
    /// Returns error if timer cannot be scheduled.
    pub async fn schedule_cron(
        &self,
        _cron_expression: impl Into<String>,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> TimerWheelResult<TimerId> {
        let entry =
            TimerEntry::new_scheduled(_cron_expression, job_id, task_id).map_err(|e| {
                TimerWheelError::PersistenceError(e.to_string())
            })?;

        let timer_id = entry.variant.timer_id().clone();
        self.add_timer(entry).await?;
        Ok(timer_id)
    }

    async fn add_timer(&self, entry: TimerEntry) -> TimerWheelResult<()> {
        let timer_id = entry.variant.timer_id().to_string();

        let mut timers = self.timers.write().await;
        if timers.contains_key(&timer_id) {
            return Err(TimerWheelError::TimerAlreadyExists(timer_id));
        }

        self.persistence.save_timer(&entry).await.map_err(|e| {
            TimerWheelError::PersistenceError(e.to_string())
        })?;

        timers.insert(timer_id, entry);
        Ok(())
    }

    async fn schedule_timer_entry(&self, timer_id: &str, duration: Duration) -> TimerWheelResult<()> {
        let timers = self.timers.read().await;
        let entry = timers
            .get(timer_id)
            .ok_or_else(|| TimerWheelError::TimerNotFound(timer_id.to_string()))?;

        let sleep = tokio::time::sleep(duration);

        let fire_tx = self.fire_tx.clone();
        let timer_id_str = timer_id.to_string();
        let timers_clone = self.timers.clone();
        let persistence = self.persistence.clone();

        let timer_id_for_delete = TimerId::from(timer_id_str.as_str());

        tokio::spawn(async move {
            sleep.await;
            if let Some(timer_entry) = timers_clone.write().await.remove(&timer_id_str) {
                if let Err(e) = fire_tx.send(timer_entry).await {
                    warn!(timer_id = %timer_id_str, error = %e, "Failed to send timer to fire handler");
                }
            }
            if let Err(e) = persistence.delete_timer(&timer_id_for_delete).await {
                warn!(timer_id = %timer_id_str, error = %e, "Failed to delete timer from persistence");
            }
        });

        Ok(())
    }

    /// Fires a timer immediately (used for recovered expired timers).
    async fn fire_timer(&self, entry: TimerEntry) -> TimerWheelResult<()> {
        let timer_id = entry.variant.timer_id().to_string();
        self.fire_tx
            .send(entry)
            .await
            .map_err(|_| TimerWheelError::ShutdownError("fire channel closed".to_string()))?;
        debug!(timer_id = %timer_id, "Fired recovered timer");
        Ok(())
    }

    /// Cancels a timer.
    ///
    /// # Errors
    ///
    /// Returns error if timer cannot be cancelled.
    pub async fn cancel(&self, timer_id: &str) -> TimerWheelResult<()> {
        let mut timers = self.timers.write().await;
        let entry = timers
            .get_mut(timer_id)
            .ok_or_else(|| TimerWheelError::TimerNotFound(timer_id.to_string()))?;

        if entry.state != TimerState::Pending && entry.state != TimerState::Active {
            return Err(TimerWheelError::InvalidTimerState(timer_id.to_string()));
        }

        entry.state = TimerState::Cancelled;

        self.persistence
            .update_timer_state(&TimerId::from(timer_id), TimerState::Cancelled)
            .await
            .map_err(|e| TimerWheelError::PersistenceError(e.to_string()))?;

        timers.remove(timer_id);

        info!(timer_id = %timer_id, "Cancelled timer");
        Ok(())
    }

    /// Shuts down the timer wheel.
    pub async fn shutdown(&self) -> TimerWheelResult<()> {
        info!("Shutting down timer wheel");

        let tx = self.shutdown_tx.write().await;
        if let Some(tx) = tx.as_ref() {
            tx.send(()).await.map_err(|_| {
                TimerWheelError::ShutdownError("shutdown channel closed".to_string())
            })?;
        }

        let mut timers = self.timers.write().await;
        timers.clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::timer::InMemorySignalRegistry;
    use super::*;

    #[tokio::test]
    async fn test_timer_wheel_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persistence = Arc::new(
            TimerPersistence::open(temp_dir.path())
                .await
                .expect("Failed to open persistence"),
        );
        let registry = Arc::new(InMemorySignalRegistry::new());
        let wheel = TimerWheel::new(persistence, registry)
            .await
            .expect("Failed to create timer wheel");
        assert!(wheel.start().await.is_ok());
    }
}