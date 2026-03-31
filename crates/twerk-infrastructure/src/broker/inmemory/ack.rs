//! Acknowledgement and lifecycle handling for the in-memory broker.

use super::super::BoxedFuture;

/// Perform a health check.
pub(crate) fn health_check() -> BoxedFuture<()> {
    Box::pin(async { Ok(()) })
}

/// Shutdown the broker.
pub(crate) fn shutdown() -> BoxedFuture<()> {
    Box::pin(async { Ok(()) })
}
