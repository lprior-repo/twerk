//! MultiMounter - manages multiple mounters

use crate::mount::Mount;
use crate::runtime::mount::{Mounter, MountError};
use std::collections::HashMap;
use std::pin::Pin;
use std::future::Future;
use tokio::sync::RwLock;

/// A mounter that routes to different mounters based on mount type
pub struct MultiMounter {
    mounters: HashMap<String, Box<dyn Mounter>>,
    /// Maps mount_id -> mount_type for routing unmount to correct mounter
    mapping: RwLock<HashMap<String, String>>,
}

impl Default for MultiMounter {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiMounter {
    /// Creates a new MultiMounter
    #[must_use]
    pub fn new() -> Self {
        Self {
            mounters: HashMap::new(),
            mapping: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a mounter for a specific mount type.
    ///
    /// Returns `MountError::DuplicateMounter` if a mounter is already
    /// registered for the given type, matching Go's panic-on-duplicate
    /// behavior but expressed as a type-safe error.
    pub fn register_mounter(
        &mut self,
        mtype: &str,
        mounter: Box<dyn Mounter>,
    ) -> Result<(), MountError> {
        if self.mounters.contains_key(mtype) {
            return Err(MountError::DuplicateMounter(mtype.to_string()));
        }
        self.mounters.insert(mtype.to_string(), mounter);
        Ok(())
    }

    /// Mounts a mount specification, routing to the appropriate mounter
    pub async fn mount(
        &self,
        ctx: Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Result<(), MountError> {
        let mount_id = mnt.id.as_ref().ok_or(MountError::MissingMountId)?;
        let mount_type = mnt.mount_type.clone();

        let mounter = self.mounters.get(&mount_type).ok_or_else(|| {
            MountError::UnknownMountType(mount_type.clone())
        })?;

        // Store mount_id -> mount_type mapping for routing unmount
        {
            let mut mapping = self.mapping.write().await;
            mapping.insert(mount_id.clone(), mount_type.clone());
        }

        // Call the actual mounter
        mounter.mount(ctx, mnt).await
    }

    /// Unmounts a mount specification, routing to the appropriate mounter
    pub async fn unmount(
        &self,
        ctx: Pin<Box<dyn Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Result<(), MountError> {
        let mount_id = mnt.id.as_ref().ok_or(MountError::MissingMountId)?;

        // Look up the mount_type for this mount_id
        let mount_type = {
            let mut mapping = self.mapping.write().await;
            mapping.remove(mount_id).ok_or_else(|| {
                MountError::MounterNotFound(mnt.clone())
            })?
        };

        // Find the mounter for this mount_type
        let mounter = self.mounters.get(&mount_type).ok_or_else(|| {
            MountError::UnknownMountType(mount_type.clone())
        })?;

        // Call the actual mounter
        mounter.unmount(ctx, mnt).await
    }

    /// Gets the current mapping count (for testing)
    #[allow(dead_code)]
    pub async fn mapping_len(&self) -> usize {
        let mapping = self.mapping.read().await;
        mapping.len()
    }
}

// Note: Clone is not implemented for MultiMounter because
// Box<dyn Mounter> does not implement Clone
