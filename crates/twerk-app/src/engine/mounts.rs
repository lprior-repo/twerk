//! Twerk Engine - Mounter registration and management

use super::types::EndpointHandler;
use std::collections::HashMap;
use tracing::error;
use twerk_infrastructure::runtime::{Mounter, MultiMounter};

/// Mounter registry for engine
pub struct MountRegistry {
    mounters: HashMap<String, MultiMounter>,
}

impl MountRegistry {
    pub fn new() -> Self {
        Self {
            mounters: HashMap::new(),
        }
    }

    /// Register a mounter for a specific runtime.
    ///
    /// Matches Go's `RegisterMounter(rt, name, mounter)`:
    /// - Creates a new `MultiMounter` for the runtime if one doesn't exist yet.
    /// - Registers the named mounter into that runtime's `MultiMounter`.
    pub fn register_mounter(
        &mut self,
        runtime: &str,
        name: &str,
        mounter: Box<dyn Mounter>,
    ) -> Result<(), twerk_infrastructure::runtime::MountError> {
        let rt_key = runtime.to_string();
        let entry = self.mounters.entry(rt_key).or_default();
        // Silently ignore duplicate mounter registrations, matching Go's
        // behavior of creating a new MultiMounter per runtime key. The
        // underlying `MultiMounter::register_mounter` returns a
        // `MountError::DuplicateMounter` which we log if it's not expected.
        if let Err(e) = entry.register_mounter(name, mounter) {
            error!(
                "failed to register mounter {} for runtime {}: {}",
                name, runtime, e
            );
            return Err(e);
        }
        Ok(())
    }

    /// Get the mounter for a runtime
    pub fn get_mounter(&self, runtime: &str) -> Option<&MultiMounter> {
        self.mounters.get(runtime)
    }

    /// Get all registered runtimes
    pub fn runtimes(&self) -> impl Iterator<Item = &String> {
        self.mounters.keys()
    }
}

impl Default for MountRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for mounter registration
#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("mounter already registered: {0}")]
    DuplicateMounter(String),
    #[error("runtime not found: {0}")]
    RuntimeNotFound(String),
}
