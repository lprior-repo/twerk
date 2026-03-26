//! Docker runtime utilities following functional-rust conventions.
//!
//! This crate provides utilities for working with Docker registries,
//! including reference parsing, authentication, archive creation, and mounters.
//!
//! # Architecture
//!
//! - **Data**: Immutable structs hold parsed configuration and references
//! - **Calc**: Pure functions for parsing, encoding, and credential resolution
//! - **Actions**: File I/O and subprocess execution pushed to boundary
//!
//! # Crate Features
//!
//! - `reference`: Docker image reference parsing
//! - `auth`: Registry authentication and credentials
//! - `archive`: Tar archive creation for Docker layers
//! - `bind`: Bind mount support
//! - `tmpfs`: Tmpfs mount support
//! - `volume`: Volume mount support

pub mod archive;
pub mod auth;
pub mod bind;
pub mod config;
pub mod container;
pub mod credential_helper;
pub mod error;
pub mod helpers;
pub mod mounters;
pub mod mount;
pub mod network;
pub mod pull;
pub mod reference;
pub mod runtime;
pub mod tmpfs;
pub mod twerk;
pub mod volume;

// Re-export public types
pub use archive::{Archive, ArchiveEntry, ArchiveError};
pub use auth::{
    decode_base64_auth, get_registry_credentials, AuthConfig, AuthError, Config,
    KubernetesConfig, ProxyConfig,
};
pub use auth::config_path;
pub use bind::{BindConfig, BindMounter, BindMounterError};
pub use config::{DockerConfig, DockerConfigBuilder};
pub use container::Container;
pub use error::DockerError;
pub use mounters::{CompositeMounter, Mounter};
pub use reference::{parse, Reference, ReferenceError};
pub use runtime::DockerRuntime;
pub use tmpfs::{TmpfsMounter, TmpfsMounterError};
pub use volume::{VolumeMounter, VolumeMounterError};

// Re-export twerk types for convenience
pub use twerk_core::id::TaskId;
pub use twerk_core::mount::{Mount, mount_type};
pub use twerk_core::task::{Probe, Registry, Task, TaskLimits};
