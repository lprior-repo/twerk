//! Domain types and runtime configuration for Podman runtime

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot, RwLock};

use super::errors::PodmanError;

// ── Domain types ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub run: String,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub env: HashMap<String, String>,
    pub mounts: Vec<Mount>,
    pub files: HashMap<String, String>,
    pub networks: Vec<String>,
    pub limits: Option<TaskLimits>,
    pub registry: Option<Registry>,
    pub gpus: Option<String>,
    pub probe: Option<Probe>,
    pub sidecars: Vec<Task>,
    pub pre: Vec<Task>,
    pub post: Vec<Task>,
    pub workdir: Option<String>,
    pub result: String,
    pub progress: f64,
}

#[derive(Debug, Clone)]
pub struct Mount {
    pub id: String,
    pub mount_type: MountType,
    pub source: String,
    pub target: String,
    /// Driver options (e.g., for volume mounts: `{"type": "tmpfs"}`).
    pub opts: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MountType {
    Volume,
    Bind,
    Tmpfs,
}

impl std::fmt::Display for MountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountType::Volume => write!(f, "volume"),
            MountType::Bind => write!(f, "bind"),
            MountType::Tmpfs => write!(f, "tmpfs"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskLimits {
    pub cpus: String,
    pub memory: String,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct Probe {
    pub path: String,
    pub port: i64,
    pub timeout: String,
}

// ── Runtime config ──────────────────────────────────────────────

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

// ── Traits ──────────────────────────────────────────────────────

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

pub trait Mounter: Send + Sync {
    fn mount(&self, mount: &mut Mount) -> Result<(), anyhow::Error>;
    fn unmount(&self, mount: &Mount) -> Result<(), anyhow::Error>;
}

// ── Pull request ────────────────────────────────────────────────

pub(crate) struct PullRequest {
    pub(crate) respond_to: oneshot::Sender<Result<(), PodmanError>>,
    pub(crate) image: String,
    pub(crate) registry: Option<RegistryCredentials>,
}

#[derive(Debug, Clone)]
pub(crate) struct RegistryCredentials {
    pub(crate) username: String,
    pub(crate) password: String,
}

// ── Constants ───────────────────────────────────────────────────

pub(crate) const DEFAULT_WORKDIR: &str = "/twerk/workdir";
pub(crate) const HOST_NETWORK_NAME: &str = "host";
pub(crate) const PROGRESS_POLL_INTERVAL: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_IMAGE_TTL: Duration = Duration::from_secs(72 * 3600);
pub(crate) const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);
pub(crate) const CREATE_TIMEOUT: Duration = Duration::from_secs(30);

// ── Runtime struct ──────────────────────────────────────────────

pub struct PodmanRuntime {
    pub(crate) broker: Option<Box<dyn Broker + Send + Sync>>,
    pub(crate) pullq: mpsc::Sender<PullRequest>,
    pub(crate) images: Arc<RwLock<HashMap<String, Instant>>>,
    pub(crate) tasks: Arc<RwLock<HashMap<String, String>>>,
    pub(crate) active_tasks: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) mounter: Box<dyn Mounter + Send + Sync>,
    pub(crate) privileged: bool,
    pub(crate) host_network: bool,
    pub(crate) image_verify: bool,
    pub(crate) image_ttl: Duration,
}

impl std::fmt::Debug for PodmanRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodmanRuntime")
            .field("broker", &"<broker>")
            .field("privileged", &self.privileged)
            .field("host_network", &self.host_network)
            .field("mounter", &"<mounter>")
            .field("image_verify", &self.image_verify)
            .field("image_ttl", &self.image_ttl)
            .finish()
    }
}

// Helper methods for cross-module access
#[allow(dead_code)]
impl PodmanRuntime {
    pub(crate) fn get_privileged() -> bool {
        false // Placeholder - these will be called on instances
    }

    pub(crate) fn get_host_network() -> bool {
        false // Placeholder - these will be called on instances
    }
}
