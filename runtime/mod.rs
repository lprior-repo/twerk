//! Runtime module containing shell and podman execution runtimes
//!
//! Re-exports shared runtime abstractions from the `tork` crate:
//! - Runtime type constants (`RUNTIME_DOCKER`, `RUNTIME_PODMAN`, `RUNTIME_SHELL`)
//! - The `Runtime` trait (Run, HealthCheck)
//! - The `Mounter` trait and `MultiMounter` for mount management

pub mod podman;
pub mod shell;

// Re-export shared runtime abstractions for parity with Go's runtime package
pub use tork::runtime::Mounter;
pub use tork::runtime::MultiMounter;
pub use tork::runtime::Runtime;
pub use tork::runtime::RUNTIME_DOCKER;
pub use tork::runtime::RUNTIME_PODMAN;
pub use tork::runtime::RUNTIME_SHELL;
