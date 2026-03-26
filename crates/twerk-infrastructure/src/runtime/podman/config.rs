//! Configuration and traits for podman runtime.

use std::time::Duration;

use super::domain::{Mount, RegistryCredentials};
use super::state::PodmanError;

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

pub struct PullRequest {
    pub respond_to: tokio::sync::oneshot::Sender<Result<(), PodmanError>>,
    pub image: String,
    pub registry: Option<RegistryCredentials>,
}

#[derive(Debug, Clone)]
pub struct RegistryCredentials {
    pub username: String,
    pub password: String,
}
