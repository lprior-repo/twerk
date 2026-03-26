//! Runtime module for task execution.
//!
//! This module provides runtime implementations for executing tasks
//! in different environments (Docker, Podman, Shell).

use std::pin::Pin;
use std::future::Future;
use anyhow::Result;

use twerk_core::task::Task;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Clone)]
pub enum MountError {
    #[error("mount failed: {0}")]
    MountFailed(String),
    #[error("missing mount ID")]
    MissingMountId,
    #[error("duplicate mounter: {0}")]
    DuplicateMounter(String),
}

pub type BoxedFuture<T> = Pin<Box<dyn Future<Output = Result<T>> + Send>>;

pub const RUNTIME_DOCKER: &str = "docker";
pub const RUNTIME_PODMAN: &str = "podman";
pub const RUNTIME_SHELL: &str = "shell";

pub trait Runtime: Send + Sync {
    fn run(&self, task: &Task) -> BoxedFuture<()>;
    fn stop(&self, task: &Task) -> BoxedFuture<()>;
    fn health_check(&self) -> BoxedFuture<()>;
}

pub trait Mounter: Send + Sync {
    fn mount(&self, m: &twerk_core::mount::Mount) -> BoxedFuture<()>;
    fn unmount(&self, m: &twerk_core::mount::Mount) -> BoxedFuture<()>;
}

pub struct MultiMounter {
    mounters: Vec<std::sync::Arc<dyn Mounter>>,
}

impl MultiMounter {
    #[must_use]
    pub fn new(mounters: Vec<std::sync::Arc<dyn Mounter>>) -> Self {
        Self { mounters }
    }

    pub fn register_mounter(&mut self, _name: &str, mounter: Box<dyn Mounter>) -> Result<(), MountError> {
        self.mounters.push(std::sync::Arc::from(mounter));
        Ok(())
    }
}

impl Default for MultiMounter {
    fn default() -> Self {
        Self { mounters: Vec::new() }
    }
}

impl Mounter for MultiMounter {
    fn mount(&self, m: &twerk_core::mount::Mount) -> BoxedFuture<()> {
        let mounters = self.mounters.clone();
        let m = m.clone();
        Box::pin(async move {
            for mounter in mounters {
                mounter.mount(&m).await?;
            }
            Ok(())
        })
    }

    fn unmount(&self, m: &twerk_core::mount::Mount) -> BoxedFuture<()> {
        let mounters = self.mounters.clone();
        let m = m.clone();
        Box::pin(async move {
            for mounter in mounters {
                mounter.unmount(&m).await?;
            }
            Ok(())
        })
    }
}

pub mod docker;
pub mod podman;
