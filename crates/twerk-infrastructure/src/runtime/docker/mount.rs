//! Mounter implementations for Docker runtime.

use crate::runtime::docker::bind::{BindConfig, BindMounter};
use crate::runtime::docker::tmpfs::TmpfsMounter;
use crate::runtime::docker::volume::VolumeMounter;
use std::pin::Pin;
use std::sync::Arc;
use twerk_core::mount::{mount_type, Mount};

pub trait Mounter: Send + Sync {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>>;
    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>>;
}

impl Mounter for VolumeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move {
            match self.mount(&mnt).await {
                Ok(mounted) => {
                    let mut result = mnt;
                    result.source = mounted.source;
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            }
        })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        Box::pin(async move { self.unmount(&mnt).await.map_err(|e| e.to_string()) })
    }
}

impl Mounter for BindMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = BindMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = BindMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

impl Mounter for TmpfsMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::mount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let result = TmpfsMounter::unmount(self, mnt);
        Box::pin(async move { result.map_err(|e| e.to_string()) })
    }
}

#[allow(clippy::struct_field_names)]
pub struct CompositeMounter {
    volume: Arc<VolumeMounter>,
    bind: Arc<BindMounter>,
    tmpfs: Arc<TmpfsMounter>,
}

impl CompositeMounter {
    #[must_use]
    pub fn new(client: bollard::Docker) -> Self {
        Self {
            volume: Arc::new(VolumeMounter::with_client(client)),
            bind: Arc::new(BindMounter::new(BindConfig {
                allowed: true,
                sources: Vec::new(),
            })),
            tmpfs: Arc::new(TmpfsMounter::new()),
        }
    }

    fn mounter_for(&self, mount_type: &str) -> Arc<dyn Mounter> {
        match mount_type {
            mount_type::BIND => self.bind.clone(),
            mount_type::TMPFS => self.tmpfs.clone(),
            _ => self.volume.clone(),
        }
    }
}

impl Mounter for CompositeMounter {
    fn mount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mount_type = mnt.mount_type.as_deref().unwrap_or("volume");
        let mounter = self.mounter_for(mount_type);
        Box::pin(async move { mounter.mount(&mnt).await })
    }

    fn unmount(
        &self,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        let mnt = mnt.clone();
        let mount_type = mnt.mount_type.as_deref().unwrap_or("volume");
        let mounter = self.mounter_for(mount_type);
        Box::pin(async move { mounter.unmount(&mnt).await })
    }
}
