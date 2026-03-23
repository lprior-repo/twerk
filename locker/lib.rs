//! Locker crate — provides distributed locking abstractions.
//!
//! This crate offers two locker implementations:
//! - [`InMemoryLocker`]: A simple in-memory locker for single-process usage.
//! - [`PostgresLocker`]: A PostgreSQL-backed distributed locker using advisory locks.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

pub mod error;
pub mod inmemory;
pub mod postgres;

use std::pin::Pin;

pub use error::LockError;
pub use inmemory::InMemoryLocker;
pub use postgres::{hash_key, PostgresLocker};

/// Locker type constant — in-memory (single process only).
pub const LOCKER_INMEMORY: &str = "inmemory";

/// Locker type constant — `PostgreSQL` (distributed).
pub const LOCKER_POSTGRES: &str = "postgres";

/// Represents an acquired lock that can be released.
///
/// The lock is consumed when released, preventing double-release.
pub trait Lock: Send + Sync {
    /// Release the lock, consuming it.
    fn release_lock(self: Pin<Box<Self>>) -> Pin<Box<dyn std::future::Future<Output = Result<(), LockError>> + Send>>;
}

/// Result type for [`Locker::acquire_lock`] operations.
pub type AcquireLockFuture = Pin<Box<dyn std::future::Future<Output = Result<Pin<Box<dyn Lock>>, LockError>> + Send>>;

/// Trait for lockers that can acquire and release locks.
pub trait Locker: Send + Sync {
    /// Attempt to acquire a lock for the given key.
    ///
    /// Returns `Ok(lock)` if successful, or `Err(LockError)` if the lock
    /// could not be acquired (e.g., already held).
    fn acquire_lock(&self, key: &str) -> AcquireLockFuture;
}
