//! Version information for the Tork runtime.

/// Tork version string (set at compile time via Cargo).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tork package name.
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");

/// Tork repository URL.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
