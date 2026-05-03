//! Timer entry types and TimerId.
//!
//! Defines the different timer variants and their data structures.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

/// Unique identifier for a timer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimerId(String);

impl TimerId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for TimerId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for TimerId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TimerId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl std::fmt::Display for TimerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur when creating or validating timer entries.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TimerEntryError {
    #[error("timer ID is required")]
    MissingTimerId,
    #[error("invalid duration: {0}")]
    InvalidDuration(String),
    #[error("invalid cron expression: {0}")]
    InvalidCronExpression(String),
    #[error("expiration time is required")]
    MissingExpirationTime,
    #[error("signal ID is required for wait-for timer")]
    MissingSignalId,
}

/// A delay timer that fires after a relative duration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelayTimer {
    pub timer_id: TimerId,
    pub duration: Duration,
    pub job_id: String,
    pub task_id: String,
    pub created_at: OffsetDateTime,
}

impl DelayTimer {
    /// Creates a new delay timer.
    ///
    /// # Errors
    ///
    /// Returns `TimerEntryError` if duration is zero.
    pub fn new(
        duration: Duration,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<Self, TimerEntryError> {
        if duration.is_zero() {
            return Err(TimerEntryError::InvalidDuration(
                "delay duration must be non-zero".to_owned(),
            ));
        }
        Ok(Self {
            timer_id: TimerId::new(),
            duration,
            job_id: job_id.into(),
            task_id: task_id.into(),
            created_at: OffsetDateTime::now_utc(),
        })
    }

    #[must_use]
    pub fn expiration_time(&self) -> OffsetDateTime {
        self.created_at + time::Duration::new(self.duration.as_secs() as i64, self.duration.subsec_nanos() as i32)
    }
}

/// A scheduled timer that fires at an absolute time (cron-like).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTimer {
    pub timer_id: TimerId,
    pub cron_expression: String,
    pub job_id: String,
    pub task_id: String,
    pub created_at: OffsetDateTime,
}

impl ScheduledTimer {
    /// Creates a new scheduled timer.
    ///
    /// # Errors
    ///
    /// Returns `TimerEntryError` if cron expression is empty.
    pub fn new(
        cron_expression: impl Into<String>,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<Self, TimerEntryError> {
        let cron_expression = cron_expression.into();
        if cron_expression.is_empty() {
            return Err(TimerEntryError::InvalidCronExpression(
                "cron expression cannot be empty".to_owned(),
            ));
        }
        Ok(Self {
            timer_id: TimerId::new(),
            cron_expression,
            job_id: job_id.into(),
            task_id: task_id.into(),
            created_at: OffsetDateTime::now_utc(),
        })
    }
}

/// A wait-for timer that fires after a timeout if the signal is not received.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForTimer {
    pub timer_id: TimerId,
    pub timeout: Duration,
    pub signal_id: String,
    pub job_id: String,
    pub task_id: String,
    pub created_at: OffsetDateTime,
}

impl WaitForTimer {
    /// Creates a new wait-for timer.
    ///
    /// # Errors
    ///
    /// Returns `TimerEntryError` if timeout is zero or signal_id is empty.
    pub fn new(
        timeout: Duration,
        signal_id: impl Into<String>,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<Self, TimerEntryError> {
        if timeout.is_zero() {
            return Err(TimerEntryError::InvalidDuration(
                "wait-for timeout must be non-zero".to_owned(),
            ));
        }
        let signal_id = signal_id.into();
        if signal_id.is_empty() {
            return Err(TimerEntryError::MissingSignalId);
        }
        Ok(Self {
            timer_id: TimerId::new(),
            timeout,
            signal_id,
            job_id: job_id.into(),
            task_id: task_id.into(),
            created_at: OffsetDateTime::now_utc(),
        })
    }

    #[must_use]
    pub fn expiration_time(&self) -> OffsetDateTime {
        self.created_at + time::Duration::new(self.timeout.as_secs() as i64, self.timeout.subsec_nanos() as i32)
    }
}

/// The variant type of a timer entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimerVariant {
    Delay(DelayTimer),
    Scheduled(ScheduledTimer),
    WaitFor(WaitForTimer),
}

impl TimerVariant {
    #[must_use]
    pub fn timer_id(&self) -> &TimerId {
        match self {
            Self::Delay(t) => &t.timer_id,
            Self::Scheduled(t) => &t.timer_id,
            Self::WaitFor(t) => &t.timer_id,
        }
    }

    #[must_use]
    pub fn job_id(&self) -> &str {
        match self {
            Self::Delay(t) => t.job_id.as_str(),
            Self::Scheduled(t) => t.job_id.as_str(),
            Self::WaitFor(t) => t.job_id.as_str(),
        }
    }

    #[must_use]
    pub fn task_id(&self) -> &str {
        match self {
            Self::Delay(t) => t.task_id.as_str(),
            Self::Scheduled(t) => t.task_id.as_str(),
            Self::WaitFor(t) => t.task_id.as_str(),
        }
    }

    #[must_use]
    pub fn expiration_time(&self) -> Option<OffsetDateTime> {
        match self {
            Self::Delay(t) => Some(t.expiration_time()),
            Self::Scheduled(_) => None,
            Self::WaitFor(t) => Some(t.expiration_time()),
        }
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expiration_time() {
            exp <= OffsetDateTime::now_utc()
        } else {
            false
        }
    }
}

/// A complete timer entry with metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerEntry {
    pub variant: TimerVariant,
    pub state: TimerState,
    pub last_fired_at: Option<OffsetDateTime>,
    pub fire_count: u64,
}

impl TimerEntry {
    #[must_use]
    pub fn new_delay(
        duration: Duration,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<Self, TimerEntryError> {
        Ok(Self {
            variant: TimerVariant::Delay(DelayTimer::new(duration, job_id, task_id)?),
            state: TimerState::Pending,
            last_fired_at: None,
            fire_count: 0,
        })
    }

    #[must_use]
    pub fn new_scheduled(
        cron_expression: impl Into<String>,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<Self, TimerEntryError> {
        Ok(Self {
            variant: TimerVariant::Scheduled(ScheduledTimer::new(
                cron_expression,
                job_id,
                task_id,
            )?),
            state: TimerState::Pending,
            last_fired_at: None,
            fire_count: 0,
        })
    }

    #[must_use]
    pub fn new_wait_for(
        timeout: Duration,
        signal_id: impl Into<String>,
        job_id: impl Into<String>,
        task_id: impl Into<String>,
    ) -> Result<Self, TimerEntryError> {
        Ok(Self {
            variant: TimerVariant::WaitFor(WaitForTimer::new(
                timeout,
                signal_id,
                job_id,
                task_id,
            )?),
            state: TimerState::Pending,
            last_fired_at: None,
            fire_count: 0,
        })
    }
}

/// The state of a timer entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerState {
    Pending,
    Active,
    Firing,
    Cancelled,
    Completed,
}

impl Default for TimerState {
    fn default() -> Self {
        Self::Pending
    }
}