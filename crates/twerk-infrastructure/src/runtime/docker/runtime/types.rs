//! Type definitions and constants for Docker runtime.

/// Default working directory inside containers.
pub const DEFAULT_WORKDIR: &str = "/workspace";

/// Default command for containers without explicit cmd.
pub const DEFAULT_CMD: &[&str] = &["/bin/sh", "-c"];

/// Entrypoint used when `run` is specified.
pub const RUN_ENTRYPOINT: &[&str] = &["sh", "-c"];

/// Default path for HTTP probes.
pub const DEFAULT_PROBE_PATH: &str = "/";

/// Default timeout for HTTP probes.
pub const DEFAULT_PROBE_TIMEOUT: &str = "1m";

use tokio::sync::oneshot;
use twerk_core::task::Registry;

use crate::runtime::docker::error::DockerError;

/// Request to pull an image, sent through the serialized pull queue.
pub(super) struct PullRequest {
    pub image: String,
    pub registry: Option<Registry>,
    #[allow(dead_code)]
    pub logger: Box<dyn std::io::Write + Send>,
    pub result_tx: oneshot::Sender<Result<(), DockerError>>,
}
