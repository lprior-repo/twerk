//! Mount abstraction trait

use crate::mount::Mount;
use std::future::Future;
use std::pin::Pin;

/// Trait for filesystem mounters
pub trait Mounter: Send + Sync {
    /// Mount the given mount specification
    fn mount(
        &self,
        ctx: Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>;

    /// Unmount the given mount specification
    fn unmount(
        &self,
        ctx: Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>;
}

/// Errors that can occur during mount operations
#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("mount ID is required")]
    MissingMountId,

    #[error("unknown mount type: {0}")]
    UnknownMountType(String),

    #[error("mounter not found for mount: {0:?}")]
    MounterNotFound(Mount),

    #[error("mount operation failed: {0}")]
    MountFailed(String),

    #[error("unmount operation failed: {0}")]
    UnmountFailed(String),
}
