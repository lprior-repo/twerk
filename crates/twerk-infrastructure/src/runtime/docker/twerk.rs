//! Docker module for twerk-runtime.
//!
//! This module provides Docker container runtime support for executing tasks.

use std::collections::HashMap as PersistentHashMap;

/// Mount type constants (mirroring twerk package).
pub mod mount_type {
    /// Volume mount type.
    pub const VOLUME: &str = "volume";
    /// Bind mount type.
    pub const BIND: &str = "bind";
    /// Tmpfs mount type.
    pub const TMPFS: &str = "tmpfs";
}

/// Represents a mount point for a task.
#[derive(Debug, Clone)]
pub struct Mount {
    /// Unique identifier for the mount.
    pub id: Option<String>,
    /// Type of mount (volume, bind, tmpfs).
    pub mount_type: String,
    /// Source path (for bind mounts).
    pub source: Option<String>,
    /// Target path in container.
    pub target: Option<String>,
    /// Mount options.
    pub opts: PersistentHashMap<String, String>,
}

impl Default for Mount {
    fn default() -> Self {
        Self {
            id: None,
            mount_type: mount_type::VOLUME.to_string(),
            source: None,
            target: None,
            opts: PersistentHashMap::new(),
        }
    }
}

impl Mount {
    /// Creates a new mount with the given type and target.
    #[must_use]
    pub fn new(mount_type: &str, target: &str) -> Self {
        Self {
            mount_type: mount_type.to_string(),
            target: Some(target.to_string()),
            ..Default::default()
        }
    }

    /// Sets the source for the mount.
    #[must_use]
    pub fn with_source(mut self, source: &str) -> Self {
        self.source = Some(source.to_string());
        self
    }

    /// Sets the ID for the mount.
    #[must_use]
    pub fn with_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }
}

/// Task limits (CPU, memory, etc.).
#[derive(Debug, Clone, Default)]
pub struct TaskLimits {
    /// CPU limit string (e.g., "1", "0.5").
    pub cpus: Option<String>,
    /// Memory limit string (e.g., "1GB").
    pub memory: Option<String>,
}

impl TaskLimits {
    /// Creates new task limits with the given CPU and memory strings.
    #[must_use]
    pub fn new(cpus: Option<&str>, memory: Option<&str>) -> Self {
        Self {
            cpus: cpus.map(String::from),
            memory: memory.map(String::from),
        }
    }
}

/// Registry credentials for image pulls.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Registry {
    /// Creates a new registry with the given credentials.
    #[must_use]
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            username: Some(username.to_string()),
            password: Some(password.to_string()),
        }
    }
}

/// Health check probe configuration.
#[derive(Debug, Clone)]
pub struct Probe {
    /// URL path for the probe.
    pub path: Option<String>,
    /// Port number for the probe.
    pub port: Option<u16>,
    /// Timeout duration string.
    pub timeout: Option<String>,
}

impl Default for Probe {
    fn default() -> Self {
        Self {
            path: Some("/".to_string()),
            port: None,
            timeout: Some("1m".to_string()),
        }
    }
}

impl Probe {
    /// Creates a new probe with the given port.
    #[must_use]
    pub fn new(port: u16) -> Self {
        Self {
            path: Some("/".to_string()),
            port: Some(port),
            timeout: Some("1m".to_string()),
        }
    }

    /// Sets the path for the probe.
    #[must_use]
    pub fn with_path(mut self, path: &str) -> Self {
        self.path = Some(path.to_string());
        self
    }
}
