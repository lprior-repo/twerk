//! Error types for the cache module.

use thiserror::Error;

/// Errors that can occur during cache operations.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub(crate) enum CacheError {
    /// The requested key was not found in the cache.
    #[error("key not found: {0}")]
    KeyNotFound(String),

    /// Modification of the cached value failed.
    #[error("modification failed: {0}")]
    ModificationFailed(String),

    /// The cache has been closed and can no longer accept operations.
    #[error("cache is closed")]
    CacheClosed,
}
