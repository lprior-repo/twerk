//! Tmpfs mount support following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `TmpfsMounter` is a stateless mounter
//! - **Calc**: Pure validation logic
//! - **Actions**: None required for tmpfs (handled by Docker)

use crate::docker::tork::Mount;
use thiserror::Error;

/// Errors from tmpfs mount operations.
#[derive(Debug, Error)]
pub enum TmpfsMounterError {
    #[error("tmpfs target is required")]
    TargetRequired,

    #[error("tmpfs source should be empty, got: {0}")]
    SourceNotEmpty(String),
}

/// Tmpfs mounter for memory-based filesystem mounts.
#[derive(Debug, Default, Clone)]
pub struct TmpfsMounter;

impl TmpfsMounter {
    /// Creates a new tmpfs mounter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TmpfsMounter {
    /// Mounts a tmpfs mount.
    ///
    /// # Errors
    ///
    /// Returns `TmpfsMounterError` if the mount configuration is invalid.
    pub fn mount(&self, mnt: &Mount) -> Result<(), TmpfsMounterError> {
        // Target is required
        let target = mnt
            .target
            .as_ref()
            .ok_or(TmpfsMounterError::TargetRequired)?;

        if target.is_empty() {
            return Err(TmpfsMounterError::TargetRequired);
        }

        // Source should be empty for tmpfs
        if let Some(ref source) = mnt.source {
            if !source.is_empty() {
                return Err(TmpfsMounterError::SourceNotEmpty(source.clone()));
            }
        }

        Ok(())
    }

    /// Unmounts a tmpfs mount.
    ///
    /// This is a no-op since tmpfs unmounting is handled by the Docker runtime.
    #[must_use]
    pub fn unmount(&self, _mnt: &Mount) -> Result<(), TmpfsMounterError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::tork::mount_type;

    #[test]
    fn test_mount_tmpfs() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount::new(mount_type::TMPFS, "/target");

        let result = mounter.mount(&mnt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mount_tmpfs_with_source() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount::new(mount_type::TMPFS, "/target").with_source("/source");

        let result = mounter.mount(&mnt);
        assert!(result.is_err());
    }

    #[test]
    fn test_mount_tmpfs_no_target() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount {
            mount_type: mount_type::TMPFS.to_string(),
            target: None,
            ..Default::default()
        };

        let result = mounter.mount(&mnt);
        assert!(result.is_err());
    }

    #[test]
    fn test_mount_tmpfs_empty_target() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount::new(mount_type::TMPFS, "").with_source("");

        let result = mounter.mount(&mnt);
        assert!(result.is_err());
    }

    #[test]
    fn test_mount_tmpfs_empty_source_allowed() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount::new(mount_type::TMPFS, "/target");

        // source is None, which is fine
        let result = mounter.mount(&mnt);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unmount_tmpfs() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount::new(mount_type::TMPFS, "/target");

        let result = mounter.unmount(&mnt);
        assert!(result.is_ok());
    }
}
