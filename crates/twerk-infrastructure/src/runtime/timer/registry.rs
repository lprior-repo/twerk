//! Signal registry for waking waiting actors.
//!
//! When a timer fires, the `SignalRegistry` is used to wake the actor
//! that is waiting on that signal.

use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur when interacting with the signal registry.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SignalRegistryError {
    #[error("signal not found: {0}")]
    SignalNotFound(String),
    #[error("failed to send signal: {0}")]
    SendFailed(String),
    #[error("registry error: {0}")]
    RegistryError(String),
}

/// Result type for `SignalRegistry` operations.
pub type SignalRegistryResult<T> = std::result::Result<T, SignalRegistryError>;

/// Signal payload sent when a timer fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerSignal {
    pub signal_id: String,
    pub timer_id: String,
    pub job_id: String,
    pub task_id: String,
    pub fired_at: time::OffsetDateTime,
    pub is_timeout: bool,
}

impl TimerSignal {
    #[must_use]
    pub fn new(
        signal_id: String,
        timer_id: String,
        job_id: String,
        task_id: String,
        is_timeout: bool,
    ) -> Self {
        Self {
            signal_id,
            timer_id,
            job_id,
            task_id,
            fired_at: time::OffsetDateTime::now_utc(),
            is_timeout,
        }
    }
}

/// Trait for signal registry operations.
///
/// Implement this trait to integrate with different actor systems
/// that need to be woken up when timers fire.
#[async_trait]
pub trait SignalRegistry: Send + Sync {
    /// Register a signal that an actor is waiting on.
    ///
    /// # Errors
    ///
    /// Returns error if registration fails.
    async fn register_waiter(
        &self,
        signal_id: &str,
        job_id: &str,
        task_id: &str,
    ) -> SignalRegistryResult<()>;

    /// Unregister a signal waiter (e.g., when the wait is cancelled).
    ///
    /// # Errors
    ///
    /// Returns error if unregistration fails.
    async fn unregister_waiter(&self, signal_id: &str) -> SignalRegistryResult<()>;

    /// Send a signal to wake a waiting actor.
    ///
    /// # Errors
    ///
    /// Returns error if the signal cannot be sent.
    async fn send_signal(&self, signal: TimerSignal) -> SignalRegistryResult<()>;

    /// Check if a signal is registered.
    #[must_use]
    async fn is_registered(&self, signal_id: &str) -> bool;

    /// Get all signal IDs registered for a job.
    #[must_use]
    async fn get_job_signals(&self, job_id: &str) -> Vec<String>;
}
