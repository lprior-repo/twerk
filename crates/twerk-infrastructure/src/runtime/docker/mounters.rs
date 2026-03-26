// ----------------------------------------------------------------------------
// Mounters
// ----------------------------------------------------------------------------

use std::pin::Pin;
use std::sync::Arc;



use crate::runtime::docker::bind::{BindConfig, BindMounter};
use crate::runtime::docker::tmpfs::TmpfsMounter;
use crate::runtime::docker::volume::VolumeMounter;
use twerk_core::mount::{MOUNT_TYPE_BIND, MOUNT_TYPE_TMPFS};
use twerk_core::mount::Mount;

/// Mounter trait for volume mounts. Must be dyn-compatible.
pub trait Mounter: Send + Sync {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>>;
    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>>;
}

impl Mounter for VolumeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move {
            match self.mount(&mnt).await {
                Ok(_) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move { self.unmount(&mnt).await.map_err(|e| e.to_string()) })
    }
}

impl Mounter for BindMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = BindMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = BindMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

impl Mounter for TmpfsMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

/// Composite mounter that dispatches to the appropriate mounter based on mount type.
pub struct CompositeMounter {
    volume_mounter: Arc<VolumeMounter>,
    bind_mounter: Arc<BindMounter>,
    tmpfs_mounter: Arc<TmpfsMounter>,
}

impl CompositeMounter {
    /// Creates a new composite mounter with all mounters initialized.
    pub fn new(client: bollard::Docker) -> Self {
        Self {
            volume_mounter: Arc::new(VolumeMounter::with_client(client)),
            bind_mounter: Arc::new(BindMounter::new(BindConfig {
                allowed: true,
                sources: Vec::new(),
            })),
            tmpfs_mounter: Arc::new(TmpfsMounter::new()),
        }
    }

    fn mounter_for(&self, mount_type: &str) -> Arc<dyn Mounter> {
        match mount_type {
            MOUNT_TYPE_BIND => self.bind_mounter.clone(),
            MOUNT_TYPE_TMPFS => self.tmpfs_mounter.clone(),
            _ => self.volume_mounter.clone(),
        }
    }
}

impl Mounter for CompositeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mounter = self.mounter_for(mnt.mount_type.as_deref().map_or("", |t| t));
        Box::pin(async move { mounter.mount(&mnt).await })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mounter = self.mounter_for(mnt.mount_type.as_deref().map_or("", |t| t));
        Box::pin(async move { mounter.unmount(&mnt).await })
    }
}
