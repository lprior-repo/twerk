//! Worker initialization module
//!
//! This module handles worker and runtime creation based on configuration.
//! Go parity with `engine/worker.go`.
//!
//! # Architecture
//!
//! - **Data**: `RuntimeConfig`, `BindConfig` — pure configuration types
//! - **Calc**: `init_runtime`, config reader functions — pure selection logic
//! - **Actions**: `create_runtime_from_config`, `create_worker` — I/O at boundary

use crate::broker::BrokerProxy;
use anyhow::{anyhow, Result};
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, RemoveContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;
use std::process::Stdio;
use tokio::process::Command;
use tork::mount::Mount;
use tork::runtime::mount::{MountError, Mounter};
use tork::runtime::multi::MultiMounter;
use tork::runtime::Runtime as RuntimeTrait;

use tracing::{debug, warn};

// =============================================================================
// Option helpers — functional, no unwrap
// =============================================================================

/// Returns true if the option is `None` or the inner string is empty.
fn is_none_or_empty(opt: &Option<String>) -> bool {
    opt.as_ref().is_none_or(|s| s.is_empty())
}

/// Returns true if the option is `None` or the inner vec is empty.
fn is_none_or_empty_vec<T>(opt: &Option<Vec<T>>) -> bool {
    opt.as_ref().is_none_or(|v| v.is_empty())
}

/// Parses a timeout duration string (e.g., "5s", "1m", "1h", "500ms").
/// Matches Go's `time.ParseDuration` behavior.
/// Returns None if the string is empty or has invalid format.
fn parse_timeout_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if s.is_empty() {
        return Some(std::time::Duration::from_secs(0));
    }

    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len() - 2], "ms")
    } else if s.ends_with('s') {
        (&s[..s.len() - 1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], "h")
    } else {
        return None; // Invalid unit
    };

    let num: u64 = num_str.parse().ok()?;

    Some(match unit {
        "ms" => std::time::Duration::from_millis(num),
        "s" => std::time::Duration::from_secs(num),
        "m" => std::time::Duration::from_secs(num * 60),
        "h" => std::time::Duration::from_secs(num * 3600),
        _ => return None,
    })
}

// =============================================================================
// Configuration helpers — local implementation matching Go conf module
// =============================================================================

/// Get config string value from env (TORK_ prefix)
fn config_string(key: &str) -> String {
    let env_key = format!("TORK_{}", key.to_uppercase().replace('.', "_"));
    std::env::var(&env_key).unwrap_or_default()
}

/// Get config string with default
fn config_string_default(key: &str, default: &str) -> String {
    let value = config_string(key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

/// Get config boolean
fn config_bool(key: &str) -> bool {
    let value = config_string(key);
    value.to_lowercase() == "true" || value == "1"
}

/// Get config strings (comma-separated or array)
fn config_strings(key: &str) -> Vec<String> {
    let value = config_string(key);
    if value.is_empty() {
        Vec::new()
    } else if value.starts_with('[') {
        value
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        value.split(',').map(|s| s.trim().to_string()).collect()
    }
}

// =============================================================================
// =============================================================================
// Limits configuration
// =============================================================================

/// Limits for task execution resources.
///
/// Go parity: `type Limits struct { Cpus string; Memory string; Timeout string }`
/// with constants `DefaultCPUsLimit`, `DefaultMemoryLimit`, `DefaultTimeout`.
#[derive(Debug, Clone, Default)]
pub struct Limits {
    /// CPU limit (e.g., "1", "2", "0.5")
    pub cpus: String,
    /// Memory limit (e.g., "512m", "1g")
    pub memory: String,
    /// Timeout duration (e.g., "5m", "1h")
    pub timeout: String,
}

/// Default CPU limit — matches Go's `DefaultCPUsLimit`.
pub const DEFAULT_CPUS_LIMIT: &str = "1";

/// Default memory limit — matches Go's `DefaultMemoryLimit`.
pub const DEFAULT_MEMORY_LIMIT: &str = "512m";

/// Default timeout — matches Go's `DefaultTimeout`.
pub const DEFAULT_TIMEOUT: &str = "5m";

/// Read limits from environment variables.
///
/// Go parity: reads `TORK_WORKER_LIMITS_CPUS`, `TORK_WORKER_LIMITS_MEMORY`, `TORK_WORKER_LIMITS_TIMEOUT`.
#[must_use]
pub fn read_limits() -> Limits {
    let cpus = std::env::var("TORK_WORKER_LIMITS_CPUS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_CPUS_LIMIT.to_string());
    let memory = std::env::var("TORK_WORKER_LIMITS_MEMORY")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_MEMORY_LIMIT.to_string());
    let timeout = std::env::var("TORK_WORKER_LIMITS_TIMEOUT")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TIMEOUT.to_string());
    Limits {
        cpus,
        memory,
        timeout,
    }
}

// Boxed future type
// =============================================================================

/// Boxed future type for worker operations
pub type BoxedFuture<T> = Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>;

/// Worker trait for task execution
pub trait Worker: Send + Sync {
    fn start(&self) -> BoxedFuture<()>;
    fn stop(&self) -> BoxedFuture<()>;
}

// Re-export Worker as WorkerTrait for backwards compatibility
pub use Worker as WorkerTrait;

// =============================================================================
// Runtime type constants
// =============================================================================

/// Runtime constants matching Go implementation
pub mod runtime_type {
    /// Docker runtime type
    pub const DOCKER: &str = "docker";
    /// Shell runtime type
    pub const SHELL: &str = "shell";
    /// Podman runtime type
    pub const PODMAN: &str = "podman";

    /// Default runtime is Docker
    pub const DEFAULT: &str = DOCKER;
}

// =============================================================================
// Mount configuration
// =============================================================================

/// Configuration for bind mount operations.
///
/// Go parity: `type BindConfig struct { Allowed bool; Sources []string }`
#[derive(Debug, Clone, Default)]
pub struct BindConfig {
    /// Whether bind mounts are allowed
    pub allowed: bool,
    /// Allowed source directories (empty = all)
    pub sources: Vec<String>,
}

// =============================================================================
// Mounter implementations
// =============================================================================
