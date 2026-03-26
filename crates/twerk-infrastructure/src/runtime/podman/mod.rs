//! Podman runtime module
//!
//! Implements the Go podman runtime at full parity, including:
//! - Registry credentials (login before pull)
//! - Resource limits (CPUs, memory)
//! - GPU support (--gpus flag)
//! - Probe support (HTTP health check after start)
//! - Image verify option (create + remove test container)
//! - Image TTL pruning (periodic background pruner)
//! - Tmpfs mount type
//! - Mount driver options
//! - Stderr log forwarding to broker

#[cfg(test)]
mod tests;
mod volume;

// Re-export public APIs
pub use errors::PodmanError;
pub use types::{
    Broker, Mount, MountType, Mounter, PodmanConfig, PodmanRuntime, Probe, Registry, Task, TaskLimits,
};
pub use volume::VolumeMounter;

// Internal modules - organized by functional concern
mod container_execute; // Container execution and lifecycle
mod container_setup;   // Container setup and command building
mod errors;            // Error types
mod executor;          // Execution coordinator
mod helpers;           // Additional helpers (image pull via queue, progress, health check)
mod image_ops;         // Image operations (verify, prune, check existence)
mod parsing;           // Resource limit and duration parsing
mod probe;             // Probe container logic
mod runtime;           // Runtime and ContainerGuard
mod slug;              // Slug generation helpers
mod types;             // Domain types and traits
