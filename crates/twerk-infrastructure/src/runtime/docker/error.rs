//! Docker runtime errors.

use thiserror::Error;

/// Errors from Docker runtime operations.
#[derive(Debug, Error)]
pub enum DockerError {
    #[error("failed to create Docker client: {0}")]
    ClientCreate(String),

    #[error("task ID is required")]
    TaskIdRequired,

    #[error("image is required")]
    ImageRequired,

    #[error("volume target is required")]
    VolumeTargetRequired,

    #[error("bind target is required")]
    BindTargetRequired,

    #[error("bind source is required")]
    BindSourceRequired,

    #[error("unknown mount type: {0}")]
    UnknownMountType(String),

    #[error("error pulling image: {0}")]
    ImagePull(String),

    #[error("error creating container: {0}")]
    ContainerCreate(String),

    #[error("error starting container: {0}")]
    ContainerStart(String),

    #[error("error waiting for container: {0}")]
    ContainerWait(String),

    #[error("error getting logs: {0}")]
    ContainerLogs(String),

    #[error("error removing container: {0}")]
    ContainerRemove(String),

    #[error("error mounting: {0}")]
    Mount(String),

    #[error("error unmounting: {0}")]
    Unmount(String),

    #[error("error creating network: {0}")]
    NetworkCreate(String),

    #[error("error removing network: {0}")]
    NetworkRemove(String),

    #[error("error creating volume: {0}")]
    VolumeCreate(String),

    #[error("error removing volume: {0}")]
    VolumeRemove(String),

    #[error("error copying files to container: {0}")]
    CopyToContainer(String),

    #[error("error copying files from container: {0}")]
    CopyFromContainer(String),

    #[error("error inspecting container: {0}")]
    ContainerInspect(String),

    #[error("invalid CPUs value: {0}")]
    InvalidCpus(String),

    #[error("invalid memory value: {0}")]
    InvalidMemory(String),

    #[error("image verification failed: {0}")]
    ImageVerifyFailed(String),

    #[error("image {0} is invalid or corrupted")]
    CorruptedImage(String),

    #[error("image {0} not found")]
    ImageNotFound(String),

    #[error("exit code {0}: {1}")]
    NonZeroExit(i64, String),

    #[error("probe timed out after {0}")]
    ProbeTimeout(String),

    #[error("probe error: {0}")]
    ProbeError(String),

    #[error("error parsing GPU options: {0}")]
    InvalidGpuOptions(String),

    #[error("host networking is not enabled")]
    HostNetworkDisabled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Docker API error: {0}")]
    Api(#[from] bollard::errors::Error),
}

impl DockerError {
    /// Converts this error to an `anyhow::Error`.
    #[must_use]
    pub fn to_anyhow(self) -> anyhow::Error {
        anyhow::anyhow!(self)
    }

    /// Creates a `CopyToContainer` error from any error implementing `ToString`.
    #[must_use]
    pub fn copy_to_container<E>(e: &E) -> Self
    where
        E: ToString,
    {
        Self::CopyToContainer(e.to_string())
    }

    /// Creates an `ImagePull` error from any error implementing `ToString`.
    #[must_use]
    pub fn image_pull<E>(e: &E) -> Self
    where
        E: ToString,
    {
        Self::ImagePull(e.to_string())
    }
}
