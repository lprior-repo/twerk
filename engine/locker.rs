//! Locker initialization module
//!
//! This module handles locker creation based on configuration.

use std::pin::Pin;

// Re-export Locker trait and InMemoryLocker from the external locker crate
pub use locker::InMemoryLocker;
pub use locker::Locker;

// Re-export the Lock trait
pub use locker::Lock;

// Re-export locker errors
pub use locker::LockError;

/// Boxed future for locker operations
pub type BoxedFuture<T> =
    Pin<Box<dyn std::future::Future<Output = Result<T, locker::LockError>> + Send>>;

/// Creates a locker based on configuration
pub fn create_locker(_locker_type: &str) -> Result<Box<dyn Locker + Send + Sync>, anyhow::Error> {
    // For now, just return an in-memory locker
    // Full postgres support requires async initialization which needs more setup
    Ok(Box::new(InMemoryLocker::new()) as Box<dyn Locker + Send + Sync>)
}
