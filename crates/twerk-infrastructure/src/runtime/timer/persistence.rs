//! Timer persistence layer.
//!
//! Provides pluggable storage for timer entries:
//! - [`InMemoryTimerPersistence`]: volatile HashMap-backed storage for tests and standalone mode
//! - [`PostgresTimerPersistence`]: durable PostgreSQL-backed storage for production
//!
//! Both implementations implement the [`TimerPersistence`] trait.

use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashMap;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use super::entry::{TimerEntry, TimerId, TimerState};

/// Storage backend for timer entries.
///
/// Implement this trait to swap between in-memory, `PostgreSQL`, or other backends.
/// All methods are async and thread-safe.
#[async_trait]
pub trait TimerPersistence: Send + Sync {
    /// Persist a timer entry. Overwrites any existing entry with the same ID.
    async fn save_timer(&self, entry: &TimerEntry) -> Result<()>;

    /// Retrieve a timer entry by its ID.
    async fn get_timer(&self, timer_id: &TimerId) -> Result<Option<TimerEntry>>;

    /// Remove a timer entry by its ID.
    async fn delete_timer(&self, timer_id: &TimerId) -> Result<()>;

    /// List all persisted timer entries.
    async fn list_timers(&self) -> Result<Vec<TimerEntry>>;

    /// Update the state of an existing timer entry.
    async fn update_timer_state(&self, timer_id: &TimerId, new_state: TimerState) -> Result<()>;

    /// Return all timers that are expired and in a pending/active/firing state.
    async fn get_expired_timers(&self) -> Result<Vec<TimerEntry>>;
}

// ---------------------------------------------------------------------------
// InMemoryTimerPersistence
// ---------------------------------------------------------------------------

/// Volatile in-memory timer storage backed by a [`HashMap`].
///
/// Timers are lost on process restart. Suitable for tests and standalone mode
/// where durability is not required.
#[derive(Default)]
pub struct InMemoryTimerPersistence {
    timers: RwLock<HashMap<String, TimerEntry>>,
}

impl InMemoryTimerPersistence {
    #[must_use]
    pub fn new() -> Self {
        Self {
            timers: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl TimerPersistence for InMemoryTimerPersistence {
    async fn save_timer(&self, entry: &TimerEntry) -> Result<()> {
        let key = entry.variant.timer_id().to_string();
        let mut timers = self.timers.write().await;
        timers.insert(key.clone(), entry.clone());
        debug!(timer_id = %key, "Persisted timer (in-memory)");
        Ok(())
    }

    async fn get_timer(&self, timer_id: &TimerId) -> Result<Option<TimerEntry>> {
        let timers = self.timers.read().await;
        Ok(timers.get(timer_id.as_str()).cloned())
    }

    async fn delete_timer(&self, timer_id: &TimerId) -> Result<()> {
        let mut timers = self.timers.write().await;
        timers.remove(timer_id.as_str());
        debug!(timer_id = %timer_id, "Deleted timer (in-memory)");
        Ok(())
    }

    async fn list_timers(&self) -> Result<Vec<TimerEntry>> {
        let timers = self.timers.read().await;
        Ok(timers.values().cloned().collect())
    }

    async fn update_timer_state(&self, timer_id: &TimerId, new_state: TimerState) -> Result<()> {
        let mut timers = self.timers.write().await;
        if let Some(entry) = timers.get_mut(timer_id.as_str()) {
            entry.state = new_state;
        }
        Ok(())
    }

    async fn get_expired_timers(&self) -> Result<Vec<TimerEntry>> {
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

// ---------------------------------------------------------------------------
// PostgresTimerPersistence
// ---------------------------------------------------------------------------

/// Durable PostgreSQL-backed timer storage.
///
/// Timers survive process restarts. Requires the `timers` table to exist:
///
/// ```sql
/// CREATE TABLE timers (
///     timer_id TEXT PRIMARY KEY,
///     payload  JSONB NOT NULL,
///     state    TEXT NOT NULL DEFAULT 'pending',
///     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
/// );
/// ```
#[derive(Clone)]
pub struct PostgresTimerPersistence {
    pool: PgPool,
}

impl PostgresTimerPersistence {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialise the timers table if it does not already exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the DDL statement fails.
    pub async fn ensure_schema(&self) -> Result<()> {
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS timers (
                timer_id   TEXT PRIMARY KEY,
                payload    JSONB NOT NULL,
                state      TEXT NOT NULL DEFAULT 'pending',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            ",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create timers table")?;
        info!("Timer persistence schema ensured (PostgreSQL)");
        Ok(())
    }
}

#[async_trait]
impl TimerPersistence for PostgresTimerPersistence {
    async fn save_timer(&self, entry: &TimerEntry) -> Result<()> {
        let key = entry.variant.timer_id().to_string();
        let payload = serde_json::to_value(entry).context("Failed to serialize timer")?;
        let state = format!("{:?}", entry.state).to_lowercase();

        sqlx::query(
            r"
            INSERT INTO timers (timer_id, payload, state, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (timer_id) DO UPDATE
            SET payload = EXCLUDED.payload, state = EXCLUDED.state
            ",
        )
        .bind(&key)
        .bind(payload)
        .bind(&state)
        .execute(&self.pool)
        .await
        .context("Failed to upsert timer")?;

        debug!(timer_id = %key, "Persisted timer (PostgreSQL)");
        Ok(())
    }

    async fn get_timer(&self, timer_id: &TimerId) -> Result<Option<TimerEntry>> {
        let key = timer_id.as_str();
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT payload FROM timers WHERE timer_id = $1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .context("Failed to get timer")?;

        match row {
            Some((payload,)) => {
                let entry: TimerEntry =
                    serde_json::from_value(payload).context("Failed to deserialize timer")?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    async fn delete_timer(&self, timer_id: &TimerId) -> Result<()> {
        sqlx::query("DELETE FROM timers WHERE timer_id = $1")
            .bind(timer_id.as_str())
            .execute(&self.pool)
            .await
            .context("Failed to delete timer")?;
        debug!(timer_id = %timer_id, "Deleted timer (PostgreSQL)");
        Ok(())
    }

    async fn list_timers(&self) -> Result<Vec<TimerEntry>> {
        let rows: Vec<(serde_json::Value,)> = sqlx::query_as("SELECT payload FROM timers")
            .fetch_all(&self.pool)
            .await
            .context("Failed to list timers")?;

        let mut entries = Vec::with_capacity(rows.len());
        for (payload,) in rows {
            match serde_json::from_value::<TimerEntry>(payload) {
                Ok(entry) => entries.push(entry),
                Err(e) => error!(error = %e, "Failed to deserialize timer row"),
            }
        }
        Ok(entries)
    }

    async fn update_timer_state(&self, timer_id: &TimerId, new_state: TimerState) -> Result<()> {
        let state = format!("{new_state:?}").to_lowercase();
        sqlx::query("UPDATE timers SET state = $1 WHERE timer_id = $2")
            .bind(&state)
            .bind(timer_id.as_str())
            .execute(&self.pool)
            .await
            .context("Failed to update timer state")?;
        Ok(())
    }

    async fn get_expired_timers(&self) -> Result<Vec<TimerEntry>> {
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

// ---------------------------------------------------------------------------
// SignalRegistry helpers (unchanged, kept here for re-export convenience)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InMemorySignalRegistry {
    waiters: RwLock<std::collections::HashMap<String, SignalData>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SignalData {
    job_id: String,
    task_id: String,
    registered_at: OffsetDateTime,
}

impl InMemorySignalRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            waiters: RwLock::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait]
impl super::registry::SignalRegistry for InMemorySignalRegistry {
    async fn register_waiter(
        &self,
        signal_id: &str,
        job_id: &str,
        task_id: &str,
    ) -> std::result::Result<(), super::registry::SignalRegistryError> {
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
    ) -> std::result::Result<(), super::registry::SignalRegistryError> {
        let mut waiters = self.waiters.write().await;
        waiters.remove(signal_id);
        debug!(signal_id, "Unregistered signal waiter");
        Ok(())
    }

    async fn send_signal(
        &self,
        signal: super::registry::TimerSignal,
    ) -> std::result::Result<(), super::registry::SignalRegistryError> {
        let waiters = self.waiters.read().await;
        if waiters.get(&signal.signal_id).is_some() {
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
            Err(super::registry::SignalRegistryError::SignalNotFound(
                signal.signal_id,
            ))
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
