//! Type definitions for Docker runtime.

use bollard::auth::DockerCredentials;
use bollard::container::Config as BollardConfig;
use bollard::models::NetworkCreateRequest as CreateNetworkOptions;
use bollard::models::{DeviceRequest, EndpointSettings, HealthConfig, PortBinding};
use bollard::models::{Mount as BollardMount, MountTypeEnum};
use bollard::query_parameters::RemoveImageOptions;
use bollard::query_parameters::RemoveVolumeOptions;
use std::collections::HashMap;

pub struct PullRequest {
    pub image: String,
    pub registry: Option<twerk_core::task::Registry>,
    pub logger: Box<dyn std::io::Write + Send>,
    pub result_tx: tokio::sync::oneshot::Sender<Result<(), super::error::DockerError>>,
}

pub type BollardNetworkingConfig = bollard::models::ConnectContainerToNetworkOptions;
pub type RemoveImageOptionsWithCreds = (Option<RemoveImageOptions>, Option<DockerCredentials>);
