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
pub mod credential_helper;
pub mod docker;
pub mod reference;
pub mod tmpfs;
pub mod tork;
pub mod volume;

pub use archive::{Archive, ArchiveEntry, ArchiveError};
pub use auth::{
    config_path, decode_base64_auth, get_registry_credentials, AuthConfig, AuthError, Config,
    KubernetesConfig, ProxyConfig,
};
pub use bind::{BindConfig, BindMounter, BindMounterError};
pub use reference::{parse, Reference, ReferenceError};
pub use tmpfs::{TmpfsMounter, TmpfsMounterError};
pub use volume::{VolumeMounter, VolumeMounterError};

// Re-export tork types for convenience
pub use tork::{mount_type, Mount, Probe, Registry, TaskLimits};
