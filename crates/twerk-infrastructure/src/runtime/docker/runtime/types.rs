//! Type definitions and constants for Docker runtime.

/// Default working directory inside containers.
pub const DEFAULT_WORKDIR: &str = "/workspace";

/// Default command for containers without explicit cmd.
pub const DEFAULT_CMD: &[&str] = &["/bin/sh", "-c"];

// Re-export from config for backwards compatibility
pub use crate::runtime::docker::config::{
    DEFAULT_PROBE_PATH, DEFAULT_PROBE_TIMEOUT, PROBE_TIMEOUT_SECS, RUN_ENTRYPOINT,
};

use tokio::sync::oneshot;
use twerk_core::task::Registry;

use crate::runtime::docker::error::DockerError;

/// Request to pull an image, sent through the serialized pull queue.
pub(super) struct PullRequest {
    pub image: String,
    pub registry: Option<Registry>,
    pub result_tx: oneshot::Sender<Result<(), DockerError>>,
}
