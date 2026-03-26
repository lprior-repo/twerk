//! Podman error types

use thiserror::Error;

// ── Error taxonomy ──────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum PodmanError {
    #[error("task id is required")]
    TaskIdRequired,

    #[error("task image is required")]
    ImageRequired,

    #[error("task name is required")]
    NameRequired,

    #[error("task name is required when networks are specified")]
    NameRequiredForNetwork,

    #[error("sidecars are not supported in podman runtime")]
    SidecarsNotSupported,

    #[error("host networking is not enabled")]
    HostNetworkingDisabled,

    #[error("failed to create workdir: {0}")]
    WorkdirCreation(String),

    #[error("failed to write file: {0}")]
    FileWrite(String),

    #[error("failed to create container: {0}")]
    ContainerCreation(String),

    #[error("failed to start container: {0}")]
    ContainerStart(String),

    #[error("failed to read logs: {0}")]
    LogsRead(String),

    #[error("container exited with code: {0}")]
    ContainerExitCode(String),

    #[error("failed to read output: {0}")]
    OutputRead(String),

    #[error("failed to pull image: {0}")]
    ImagePull(String),

    #[error("unknown mount type: {0}")]
    UnknownMountType(String),

    #[error("context cancelled")]
    ContextCancelled,

    #[error("failed to inspect container: {0}")]
    ContainerInspect(String),

    #[error("invalid CPUs limit: {0}")]
    InvalidCpusLimit(String),

    #[error("invalid memory limit: {0}")]
    InvalidMemoryLimit(String),

    #[error("image verification failed: {0}")]
    ImageVerification(String),

    #[error("probe timed out after {0}")]
    ProbeTimeout(String),

    #[error("probe failed: {0}")]
    ProbeFailed(String),

    #[error("registry login failed: {0}")]
    RegistryLogin(String),

    #[error("podman is not running")]
    PodmanNotRunning,
}
