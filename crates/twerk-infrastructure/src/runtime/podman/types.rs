//! Domain types for Podman runtime
//!
//! This module provides type definitions and constants for the Podman runtime.

use std::collections::HashMap;
use std::time::Duration;

pub use crate::runtime::ShutdownResult;
pub use crate::runtime::{BoxedFuture, Runtime, ShutdownError};
pub use twerk_core::mount::Mount as CoreMount;
pub use twerk_core::task::Task as CoreTask;

// Re-export for convenience
pub use super::errors::PodmanError;

// ── Mount type ────────────────────────────────────────────────────

/// Mount type enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MountType {
    Volume,
    Bind,
    Tmpfs,
}

impl From<&str> for MountType {
    fn from(s: &str) -> Self {
        match s {
            "bind" => MountType::Bind,
            "tmpfs" => MountType::Tmpfs,
            _ => MountType::Volume,
        }
    }
}

impl From<Option<&str>> for MountType {
    fn from(s: Option<&str>) -> Self {
        match s {
            Some("bind") => MountType::Bind,
            Some("tmpfs") => MountType::Tmpfs,
            _ => MountType::Volume,
        }
    }
}

impl MountType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            MountType::Volume => "volume",
            MountType::Bind => "bind",
            MountType::Tmpfs => "tmpfs",
        }
    }
}

/// Internal mount type
#[derive(Debug, Clone)]
pub struct Mount {
    pub id: String,
    pub mount_type: MountType,
    pub source: String,
    pub target: String,
    pub opts: Option<HashMap<String, String>>,
}

impl From<&CoreMount> for Mount {
    fn from(m: &CoreMount) -> Self {
        Mount {
            id: m.id.clone().unwrap_or_default(),
            mount_type: MountType::from(m.mount_type.as_deref()),
            source: m.source.clone().unwrap_or_default(),
            target: m.target.clone().unwrap_or_default(),
            opts: m.opts.clone(),
        }
    }
}

// ── Runtime config ────────────────────────────────────────────────

#[derive(Default)]
pub struct PodmanConfig {
    pub broker: Option<Box<dyn Broker + Send + Sync>>,
    pub privileged: bool,
    pub host_network: bool,
    pub mounter: Option<Box<dyn Mounter + Send + Sync>>,
    pub image_verify: bool,
    pub image_ttl: Option<Duration>,
}

impl std::fmt::Debug for PodmanConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodmanConfig")
            .field("broker", &"<broker>")
            .field("privileged", &self.privileged)
            .field("host_network", &self.host_network)
            .field("mounter", &"<mounter>")
            .field("image_verify", &self.image_verify)
            .field("image_ttl", &self.image_ttl)
            .finish()
    }
}

// ── Traits ───────────────────────────────────────────────────────

/// Broker trait for streaming logs and progress
pub trait Broker: Send + Sync {
    fn clone_box(&self) -> Box<dyn Broker + Send + Sync>;
    fn ship_log(&self, task_id: &str, line: &str);
    fn publish_task_progress(&self, task_id: &str, progress: f64);
}

impl Clone for Box<dyn Broker + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl Broker for () {
    fn clone_box(&self) -> Box<dyn Broker + Send + Sync> {
        Box::new(())
    }
    fn ship_log(&self, _task_id: &str, _line: &str) {}
    fn publish_task_progress(&self, _task_id: &str, _progress: f64) {}
}

/// Mounter trait for volume mounts
pub trait Mounter: Send + Sync {
    fn mount(&self, mount: &mut Mount) -> Result<(), PodmanError>;
    fn unmount(&self, mount: &Mount) -> Result<(), PodmanError>;
}

// ── Pull request ─────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) struct PullRequest {
    pub(crate) respond_to: tokio::sync::oneshot::Sender<Result<(), PodmanError>>,
    pub(crate) image: String,
    pub(crate) registry: Option<RegistryCredentials>,
}

#[derive(Debug, Clone)]
pub(crate) struct RegistryCredentials {
    pub(crate) username: String,
    pub(crate) password: String,
}

// ── Constants ────────────────────────────────────────────────────

pub(crate) const DEFAULT_WORKDIR: &str = "/twerk/workdir";
pub(crate) const HOST_NETWORK_NAME: &str = "host";
pub(crate) const PROGRESS_POLL_INTERVAL: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 3600);
pub(crate) const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);
