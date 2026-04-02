//! Runtime module for task execution.
//!
//! This module provides runtime implementations for executing tasks
//! in different environments (Docker, Podman, Shell).

use anyhow::Result;
use std::process::ExitCode;
use std::time::Duration;

use twerk_core::task::Task;

pub use crate::broker::BoxedFuture;

use thiserror::Error;

// ----------------------------------------------------------------------------
// Shutdown Error Types
// ----------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq, Clone)]
pub enum ShutdownError {
    #[error("task is not in a running state: {0}")]
    TaskNotRunning(String),

    #[error("process not found: task_id={0}")]
    ProcessNotFound(String),

    #[error("invalid task ID: {0}")]
    InvalidTaskId(String),

    #[error("timeout waiting for graceful shutdown: {0}s elapsed")]
    ShutdownTimeout(u64),

    #[error("failed to send termination signal: {0}")]
    SignalError(String),

    #[error("failed to terminate process: {0}")]
    TerminationFailed(String),

    #[error("cleanup failed: {0}")]
    CleanupFailed(String),

    #[error("resource not available: {0}")]
    ResourceUnavailable(String),

    #[error("exit code: {0}")]
    ExitCode(i32),

    #[error("runtime error: {0}")]
    RuntimeError(String),
}

pub type ShutdownResult<T> = Result<T, ShutdownError>;

// ----------------------------------------------------------------------------
// Mount Error Types
// ----------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq, Clone)]
pub enum MountError {
    #[error("mount failed: {0}")]
    MountFailed(String),
    #[error("missing mount ID")]
    MissingMountId,
    #[error("duplicate mounter: {0}")]
    DuplicateMounter(String),
}

pub const RUNTIME_DOCKER: &str = "docker";
pub const RUNTIME_PODMAN: &str = "podman";
pub const RUNTIME_SHELL: &str = "shell";

// Environment variable configuration
pub const ENV_TASK_STOP_GRACEFUL_TIMEOUT: &str = "TASK_STOP_GRACEFUL_TIMEOUT";
pub const ENV_TASK_STOP_FORCE_TIMEOUT: &str = "TASK_STOP_FORCE_TIMEOUT";
pub const ENV_TASK_STOP_ENABLE_CLEANUP: &str = "TASK_STOP_ENABLE_CLEANUP";

// Default timeout values (seconds)
pub const DEFAULT_GRACEFUL_TIMEOUT: u64 = 30;
pub const DEFAULT_FORCE_TIMEOUT: u64 = 5;

/// Default timeout duration (30 seconds) for container operations.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default image TTL (3 days / 72 hours) for image cache pruning.
pub const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 3600);

/// Buffer size for the image pull queue channel.
///
/// This controls how many concurrent pull requests can be buffered
/// while waiting to be processed by the pull worker.
pub const PULL_QUEUE_BUFFER_SIZE: usize = 100;

// ----------------------------------------------------------------------------
// Runtime Trait
// ----------------------------------------------------------------------------

pub trait Runtime: Send + Sync {
    fn run(&self, task: &Task) -> BoxedFuture<()>;
    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>>;
    fn health_check(&self) -> BoxedFuture<()>;
}

// ----------------------------------------------------------------------------
// Mounter Trait
// ----------------------------------------------------------------------------

pub trait Mounter: Send + Sync {
    fn mount(&self, m: &twerk_core::mount::Mount) -> BoxedFuture<()>;
    fn unmount(&self, m: &twerk_core::mount::Mount) -> BoxedFuture<()>;
}

// ----------------------------------------------------------------------------
// MultiMounter Implementation
// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct MultiMounter {
    mounters: Vec<std::sync::Arc<dyn Mounter>>,
}

impl MultiMounter {
    #[must_use]
    pub fn new(mounters: Vec<std::sync::Arc<dyn Mounter>>) -> Self {
        Self { mounters }
    }

    /// Registers a mounter.
    ///
    /// # Errors
    ///
    /// Returns `MountError` if the mounter cannot be registered.
    pub fn register_mounter(
        &mut self,
        _name: &str,
        mounter: Box<dyn Mounter>,
    ) -> Result<(), MountError> {
        self.mounters.push(std::sync::Arc::from(mounter));
        Ok(())
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
