//! Runtime execution support
//!
//! This module provides abstractions for mounting filesystems
//! and managing runtime execution contexts.

pub mod mount;
pub mod multi;

pub use mount::Mounter;
pub use multi::MultiMounter;

use crate::task::Task;
use std::pin::Pin;

/// Docker runtime engine
pub const RUNTIME_DOCKER: &str = "docker";
/// Podman runtime engine
pub const RUNTIME_PODMAN: &str = "podman";
/// Shell runtime engine
pub const RUNTIME_SHELL: &str = "shell";

/// Boxed future type for runtime operations
pub type BoxedFuture<T> =
    Pin<Box<dyn std::future::Future<Output = Result<T, anyhow::Error>> + Send>>;

/// Runtime is the actual runtime environment that executes a task.
pub trait Runtime: Send + Sync {
    /// Runs a task to completion
    fn run(&self, ctx: std::sync::Arc<tokio::sync::RwLock<()>>, task: &mut Task)
        -> BoxedFuture<()>;

    /// Performs a health check on the runtime
    fn health_check(&self) -> BoxedFuture<()>;
}
