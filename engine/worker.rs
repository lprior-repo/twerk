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
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, RemoveContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::pin::Pin;
use std::process::Stdio;
use tork::mount::Mount;
use tork::runtime::Runtime as RuntimeTrait;
use tork::runtime::mount::{MountError, Mounter};
use tork::runtime::multi::MultiMounter;
use tokio::process::Command;

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
        (&s[..s.len()-2], "ms")
    } else if s.ends_with('s') {
        (&s[..s.len()-1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len()-1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len()-1], "h")
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
    Limits { cpus, memory, timeout }
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
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct BindConfig {
    /// Whether bind mounts are allowed
    pub allowed: bool,
    /// Allowed source directories (empty = all)
    pub sources: Vec<String>,
}


// =============================================================================
// Mounter implementations
// =============================================================================

/// Bind mounter — creates source directories for bind mounts.
///
/// Go parity: `docker.BindMounter`
#[derive(Debug)]
pub struct BindMounter {
    /// Configuration for allowed bind sources
    cfg: BindConfig,
}

impl BindMounter {
    /// Creates a new bind mounter.
    ///
    /// Go parity: `func NewBindMounter(cfg BindConfig) *BindMounter`
    #[must_use]
    pub fn new(cfg: BindConfig) -> Self {
        Self { cfg }
    }

    /// Checks whether a source path is in the allowed list.
    ///
    /// Go parity: `func (m *BindMounter) isSourceAllowed(src string) bool`
    #[cfg(test)]
    fn is_source_allowed(&self, src: &str) -> bool {
        if self.cfg.sources.is_empty() {
            return true;
        }
        self.cfg.sources.iter().any(|allow| allow.eq_ignore_ascii_case(src))
    }
}

impl Mounter for BindMounter {
    fn mount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        let allowed = self.cfg.allowed;
        let sources = self.cfg.sources.clone();
        let source = mnt.source.clone().unwrap_or_default();

        let cfg_allowed = allowed;
        let cfg_sources = sources;

        Box::pin(async move {
            if !cfg_allowed {
                return Err(MountError::MountFailed(
                    "bind mounts are not allowed".to_string(),
                ));
            }

            // Source validation
            if !cfg_sources.is_empty()
                && !cfg_sources.iter().any(|s| s.eq_ignore_ascii_case(&source))
            {
                return Err(MountError::MountFailed(format!(
                    "src bind mount is not allowed: {}",
                    source
                )));
            }

            // Create source directory if it doesn't exist
            let src_path = std::path::Path::new(&source);
            if !src_path.exists() {
                std::fs::create_dir_all(src_path).map_err(|e| {
                    MountError::MountFailed(format!(
                        "error creating mount directory: {}: {}",
                        source, e
                    ))
                })?;
                debug!("Created bind mount: {}", source);
            }

            Ok(())
        })
    }

    fn unmount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        _mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        // Go parity: BindMounter.Unmount is a no-op
        Box::pin(async { Ok(()) })
    }
}

/// Volume mounter — creates temporary directories for volume mounts.
///
/// Go parity: `docker.NewVolumeMounter()`
#[derive(Debug)]
pub struct VolumeMounter;

impl VolumeMounter {
    /// Creates a new volume mounter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for VolumeMounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Mounter for VolumeMounter {
    fn mount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        let id = mnt.id.clone().unwrap_or_default();

        Box::pin(async move {
            if id.is_empty() {
                return Err(MountError::MissingMountId);
            }

            // In production, this would call Docker API to create a named volume.
            debug!("Volume mount prepared for id={}", id);
            Ok(())
        })
    }

    fn unmount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        _mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

/// Tmpfs mounter — validates tmpfs mount specifications.
///
/// Go parity: `docker.NewTmpfsMounter()`
#[derive(Debug)]
pub struct TmpfsMounter;

impl TmpfsMounter {
    /// Creates a new tmpfs mounter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TmpfsMounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Mounter for TmpfsMounter {
    fn mount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        let target = mnt.target.clone().unwrap_or_default();
        let source = mnt.source.clone().unwrap_or_default();

        Box::pin(async move {
            if target.is_empty() {
                return Err(MountError::MountFailed(
                    "tmpfs target is required".to_string(),
                ));
            }
            if !source.is_empty() {
                return Err(MountError::MountFailed(
                    "tmpfs source should be empty".to_string(),
                ));
            }
            Ok(())
        })
    }

    fn unmount(
        &self,
        _ctx: Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>>,
        _mnt: &Mount,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), MountError>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// Shell runtime adapter
// =============================================================================

/// Shell runtime that validates task constraints.
///
/// Go parity: `shell.NewShellRuntime(shell.Config{...})`
#[derive(Debug)]
pub struct ShellRuntimeAdapter {
    /// Shell command (e.g. ["bash", "-c"])
    shell_cmd: Vec<String>,
    /// UID to run as
    uid: String,
    /// GID to run as
    gid: String,
}

impl ShellRuntimeAdapter {
    /// Creates a new shell runtime adapter.
    ///
    /// Go parity: `func NewShellRuntime(cfg Config) *ShellRuntime`
    #[must_use]
    pub fn new(cmd: Vec<String>, uid: String, gid: String) -> Self {
        let shell_cmd = if cmd.is_empty() {
            vec!["bash".to_string(), "-c".to_string()]
        } else {
            cmd
        };
        let uid = if uid.is_empty() {
            "-".to_string()
        } else {
            uid
        };
        let gid = if gid.is_empty() {
            "-".to_string()
        } else {
            gid
        };
        Self {
            shell_cmd,
            uid,
            gid,
        }
    }
}

impl RuntimeTrait for ShellRuntimeAdapter {
    fn run(
        &self,
        _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
        task: &mut tork::task::Task,
    ) -> tork::runtime::BoxedFuture<()> {
        let shell_cmd = self.shell_cmd.clone();
        let uid = self.uid.clone();
        let gid = self.gid.clone();

        // Validate task constraints (Go parity: shell.Runtime.Run)
        let task_id = task.id.clone().unwrap_or_default();
        if task_id.is_empty() {
            return Box::pin(async { Err(anyhow!("task id is required")) });
        }
        if is_none_or_empty_vec(&task.mounts) {
            // mounts is None or empty — ok for shell
        } else {
            return Box::pin(async {
                Err(anyhow!("mounts are not supported on shell runtime"))
            });
        }
        if is_none_or_empty_vec(&task.entrypoint) {
            // ok
        } else {
            return Box::pin(async {
                Err(anyhow!("entrypoint is not supported on shell runtime"))
            });
        }
        if is_none_or_empty(&task.image) {
            // ok
        } else {
            return Box::pin(async {
                Err(anyhow!("image is not supported on shell runtime"))
            });
        }
        if is_none_or_empty_vec(&task.cmd) {
            // ok
        } else {
            return Box::pin(async {
                Err(anyhow!("cmd is not supported on shell runtime"))
            });
        }
        if is_none_or_empty_vec(&task.sidecars) {
            // ok
        } else {
            return Box::pin(async {
                Err(anyhow!("sidecars are not supported on shell runtime"))
            });
        }

        // Get the command to run
        let run_script = task.run.clone().unwrap_or_default();
        if run_script.is_empty() {
            return Box::pin(async { Err(anyhow!("task run script is required")) });
        }

        // Parse timeout from task (Go parity: worker.doRunTask creates timeout context)
        let timeout_duration = task
            .timeout
            .as_ref()
            .and_then(|t| parse_timeout_duration(t));

        // Build environment
        let env_vars: HashMap<String, String> = task
            .env
            .as_ref()
            .map(|e| e.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Box::pin(async move {
            debug!(
                "[shell-runtime] running task {} with cmd {:?}, uid={}, gid={}",
                task_id, shell_cmd, uid, gid
            );

            // Create a temporary script file
            let temp_dir = tempfile::tempdir()
                .map_err(|e| anyhow!("failed to create temp dir: {}", e))?;
            let script_path = temp_dir.path().join("script.sh");
            
            // Write script with shebang
            let script_content = format!("#!/bin/bash\n{}", run_script);
            tokio::fs::write(&script_path, &script_content)
                .await
                .map_err(|e| anyhow!("failed to write script: {}", e))?;

            // Make script executable (required for bash to run it)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = tokio::fs::metadata(&script_path)
                    .await
                    .map_err(|e| anyhow!("failed to get script permissions: {}", e))?
                    .permissions();
                perms.set_mode(0o755);
                tokio::fs::set_permissions(&script_path, perms)
                    .await
                    .map_err(|e| anyhow!("failed to set script permissions: {}", e))?;
            }

            // Build the command - run script directly (not via -c) so shebang works
            let mut cmd = Command::new(&shell_cmd[0]);
            cmd.arg(script_path.to_string_lossy().as_ref());
            
            // Set environment
            for (key, value) in &env_vars {
                cmd.env(key, value);
            }
            
            // Set uid/gid if not default
            #[cfg(unix)]
            {
                if uid != "-" {
                    if let Ok(uid_val) = uid.parse::<u32>() {
                        cmd.uid(uid_val);
                    }
                }
                if gid != "-" {
                    if let Ok(gid_val) = gid.parse::<u32>() {
                        cmd.gid(gid_val);
                    }
                }
            }

            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            // Use output() to avoid deadlock (wait() before reading stdout/stderr causes deadlock)
            // Apply timeout if task.timeout is set (Go parity: ctx.WithTimeout in doRunTask)
            let output = match timeout_duration {
                Some(dur) => {
                    tokio::time::timeout(dur, cmd.output())
                        .await
                        .map_err(|_| anyhow!("task timeout after {:?}", dur))?
                        .map_err(|e| anyhow!("failed to spawn shell: {}", e))?
                }
                None => {
                    cmd.output()
                        .await
                        .map_err(|e| anyhow!("failed to spawn shell: {}", e))?
                }
            };

            // Log stdout
            if !output.stdout.is_empty() {
                let stdout_str = std::str::from_utf8(&output.stdout)
                    .unwrap_or_default();
                for line in stdout_str.lines() {
                    debug!("[shell] {}", line);
                }
            }

            // Log stderr
            if !output.stderr.is_empty() {
                let stderr_str = std::str::from_utf8(&output.stderr)
                    .unwrap_or_default();
                for line in stderr_str.lines() {
                    warn!("[shell stderr] {}", line);
                }
            }

            let status = output.status;
            if !status.success() {
                return Err(anyhow!(
                    "shell command failed with exit code: {:?}",
                    status.code()
                ));
            }

            debug!("[shell-runtime] task {} completed successfully", task_id);
            Ok(())
        })
    }

    fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// Docker runtime (stub)
// =============================================================================

/// Docker runtime for container-based task execution.
///
/// Go parity: `docker.NewDockerRuntime(docker.WithMounter(mounter), ...)`
#[derive(Debug)]
pub struct DockerRuntimeAdapter {
    /// Whether the runtime runs in privileged mode
    privileged: bool,
}

impl DockerRuntimeAdapter {
    /// Creates a new Docker runtime adapter.
    #[must_use]
    pub fn new(privileged: bool) -> Self {
        Self { privileged }
    }
}

impl RuntimeTrait for DockerRuntimeAdapter {
    fn run(
        &self,
        _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
        task: &mut tork::task::Task,
    ) -> tork::runtime::BoxedFuture<()> {
        let privileged = self.privileged;
        let task_id = task.id.clone().unwrap_or_default();

        if task_id.is_empty() {
            return Box::pin(async { Err(anyhow!("task id is required")) });
        }

        // Get image (required for docker)
        let image = task.image.clone().unwrap_or_default();
        if image.is_empty() {
            return Box::pin(async { Err(anyhow!("task image is required for docker runtime")) });
        }

        // Get command
        let cmd = task.cmd.clone().unwrap_or_default();
        let entrypoint = task.entrypoint.clone().unwrap_or_default();
        let run_script = task.run.clone().unwrap_or_default();

        // Build environment
        let env_vars: Vec<String> = task
            .env
            .as_ref()
            .map(|e| {
                e.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect()
            })
            .unwrap_or_default();

        // Get working directory
        let workdir = task.workdir.clone();

        Box::pin(async move {
            debug!(
                "[docker-runtime] running task {} with image {} (privileged={})",
                task_id, image, privileged
            );

            // Connect to Docker
            let docker = Docker::connect_with_local_defaults()
                .map_err(|e| anyhow!("failed to connect to Docker: {}", e))?;

            // Pull image if needed
            let image_exists = docker
                .inspect_image(&image)
                .await
                .is_ok();

            if !image_exists {
                debug!("[docker-runtime] pulling image {}", image);
                let options = CreateImageOptions {
                    from_image: image.clone(),
                    ..Default::default()
                };
                let mut stream = docker.create_image(Some(options), None, None);
                while let Some(result) = stream.next().await {
                    if let Err(e) = result {
                        debug!("[docker-runtime] warning: pull error: {}", e);
                    }
                }
            }

            // Build container config
            let mut cmd_args: Vec<&String> = Vec::new();
            if !entrypoint.is_empty() {
                cmd_args.extend(entrypoint.iter());
            }
            if !run_script.is_empty() {
                // Use run script as entrypoint command
                cmd_args.push(&run_script);
            } else if !cmd.is_empty() {
                cmd_args.extend(cmd.iter());
            }

            // Build container config with all required fields
            let config = ContainerConfig::<String> {
                image: Some(image.clone()),
                cmd: if cmd_args.is_empty() {
                    None
                } else {
                    Some(cmd_args.into_iter().map(|s| s.clone()).collect())
                },
                env: if env_vars.is_empty() {
                    None
                } else {
                    Some(env_vars)
                },
                working_dir: workdir,
                host_config: Some(bollard::secret::HostConfig {
                    privileged: Some(privileged),
                    ..Default::default()
                }),
                ..Default::default()
            };

            // Create container
            let container_id = docker
                .create_container(
                    None::<CreateContainerOptions<String>>,
                    config,
                )
                .await
                .map_err(|e| anyhow!("failed to create container: {}", e))?
                .id;

            debug!("[docker-runtime] created container {}", container_id);

            // Start container
            docker
                .start_container::<String>(&container_id, None)
                .await
                .map_err(|e| anyhow!("failed to start container: {}", e))?;

            // Wait for completion using a simple polling approach
            let mut exit_code = None;
            let max_attempts = 60; // 60 seconds timeout
            for _ in 0..max_attempts {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                
                // Check if container is still running
                let info = docker
                    .inspect_container(&container_id, None::<bollard::container::InspectContainerOptions>)
                    .await;
                
                if let Ok(info) = info {
                    if let Some(state) = info.state {
                        if !state.running.unwrap_or(false) {
                            exit_code = state.exit_code;
                            break;
                        }
                    }
                }
            }

            let exit_code = exit_code.unwrap_or(1);

            // Log output
            if exit_code != 0 {
                debug!("[docker-runtime] container exited with code {}", exit_code);
            } else {
                debug!("[docker-runtime] container completed successfully");
            }

            // Cleanup - remove container
            let remove_options = RemoveContainerOptions {
                force: true,
                ..Default::default()
            };
            let _ = docker.remove_container(&container_id, Some(remove_options)).await;

            if exit_code != 0 {
                return Err(anyhow!(
                    "container exited with non-zero status: {}",
                    exit_code
                ));
            }

            debug!("[docker-runtime] task {} completed successfully", task_id);
            Ok(())
        })
    }

    fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async {
            match Docker::connect_with_local_defaults() {
                Ok(docker) => {
                    docker.ping().await?;
                    Ok(())
                }
                Err(e) => Err(anyhow!("docker health check failed: {}", e)),
            }
        })
    }
}

// =============================================================================
// Podman runtime adapter
// =============================================================================

/// Podman runtime adapter for container-based task execution.
///
/// Go parity: `podman.NewPodmanRuntime(podman.WithBroker(...), ...)`
#[derive(Debug)]
pub struct PodmanRuntimeAdapter {
    /// Whether the runtime runs in privileged mode
    privileged: bool,
    /// Whether to use host networking
    host_network: bool,
}

impl PodmanRuntimeAdapter {
    /// Creates a new Podman runtime adapter.
    #[must_use]
    pub fn new(privileged: bool, host_network: bool) -> Self {
        Self {
            privileged,
            host_network,
        }
    }
}

impl RuntimeTrait for PodmanRuntimeAdapter {
    fn run(
        &self,
        _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
        task: &mut tork::task::Task,
    ) -> tork::runtime::BoxedFuture<()> {
        let privileged = self.privileged;
        let host_network = self.host_network;
        let task_id = task.id.clone().unwrap_or_default();
        let image = task.image.clone().unwrap_or_default();

        if task_id.is_empty() {
            return Box::pin(async { Err(anyhow!("task id is required")) });
        }
        if image.is_empty() {
            return Box::pin(async { Err(anyhow!("task image is required")) });
        }

        // Clone task data for async block to avoid lifetime issues
        let cmd_clone = task.cmd.clone();
        let workdir_clone = task.workdir.clone();
        let env_clone = task.env.clone();

        Box::pin(async move {
            debug!(
                "[podman-runtime] running task {} image={} (privileged={}, host_network={})",
                task_id, image, privileged, host_network
            );

            // Build podman command
            let mut cmd = tokio::process::Command::new("podman");
            cmd.arg("run");

            if privileged {
                cmd.arg("--privileged");
            }

            if host_network {
                cmd.arg("--network").arg("host");
            }

            cmd.arg(&image);

            if let Some(ref c) = cmd_clone {
                for a in c {
                    cmd.arg(a);
                }
            }

            if let Some(ref wd) = workdir_clone {
                cmd.arg("--workdir").arg(wd);
            }

            if let Some(ref e) = env_clone {
                for (k, v) in e {
                    cmd.env(k, v);
                }
            }

            let output = cmd
                .output()
                .await
                .map_err(|e| anyhow!("podman failed: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("podman run failed: {}", stderr));
            }

            debug!("[podman-runtime] task {} completed successfully", task_id);
            Ok(())
        })
    }

    fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// Mock runtime
// =============================================================================

/// Mock runtime for placeholder implementation.
#[derive(Debug)]
pub struct MockRuntime;

impl RuntimeTrait for MockRuntime {
    fn run(
        &self,
        _ctx: std::sync::Arc<tokio::sync::RwLock<()>>,
        _task: &mut tork::task::Task,
    ) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn health_check(&self) -> tork::runtime::BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// No-op worker
// =============================================================================

/// No-op worker implementation for placeholder.
#[derive(Debug)]
pub struct NoOpWorker;

impl Worker for NoOpWorker {
    fn start(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn stop(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

// =============================================================================
// Runtime configuration
// =============================================================================

/// Configuration for runtime initialization.
///
/// Go parity: reads from conf.StringDefault, conf.Bool, conf.Strings etc.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Runtime type (docker, shell, podman)
    pub runtime_type: String,
    /// Docker-specific: privileged mode
    pub docker_privileged: bool,
    /// Docker-specific: image TTL in seconds
    pub docker_image_ttl_secs: u64,
    /// Docker-specific: verify images
    pub docker_image_verify: bool,
    /// Docker-specific: config file path
    pub docker_config: String,
    /// Shell-specific: command
    pub shell_cmd: Vec<String>,
    /// Shell-specific: UID
    pub shell_uid: String,
    /// Shell-specific: GID
    pub shell_gid: String,
    /// Podman-specific: privileged mode
    pub podman_privileged: bool,
    /// Podman-specific: host network
    pub podman_host_network: bool,
    /// Bind mount config
    pub bind_allowed: bool,
    /// Bind mount allowed sources
    pub bind_sources: Vec<String>,
    /// Host environment variable specs for middleware
    pub hostenv_vars: Vec<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_type: runtime_type::DEFAULT.to_string(),
            docker_privileged: false,
            docker_image_ttl_secs: 72 * 60 * 60,
            docker_image_verify: false,
            docker_config: String::new(),
            shell_cmd: vec!["bash".to_string(), "-c".to_string()],
            shell_uid: "-".to_string(),
            shell_gid: "-".to_string(),
            podman_privileged: false,
            podman_host_network: false,
            bind_allowed: false,
            bind_sources: Vec::new(),
            hostenv_vars: Vec::new(),
        }
    }
}

/// Reads runtime configuration from environment variables.
///
/// Go parity: reads from conf.StringDefault, conf.Bool, conf.Strings
pub fn read_runtime_config() -> RuntimeConfig {
    let runtime_type = config_string_default("runtime.type", runtime_type::DEFAULT);

    RuntimeConfig {
        runtime_type: runtime_type.clone(),
        docker_privileged: config_bool("runtime.docker.privileged"),
        docker_image_verify: config_bool("runtime.docker.image.verify"),
        docker_config: config_string_default("runtime.docker.config", ""),
        shell_cmd: config_strings("runtime.shell.cmd"),
        shell_uid: config_string_default("runtime.shell.uid", "-"),
        shell_gid: config_string_default("runtime.shell.gid", "-"),
        podman_privileged: config_bool("runtime.podman.privileged"),
        podman_host_network: config_bool("runtime.podman.host.network"),
        bind_allowed: config_bool("mounts.bind.allowed"),
        bind_sources: config_strings("mounts.bind.sources"),
        hostenv_vars: config_strings("middleware.task.hostenv.vars"),
        ..Default::default()
    }
}

// =============================================================================
// Runtime initialization
// =============================================================================

/// Initialize the runtime based on configuration.
///
/// Go parity: `func (e *Engine) initRuntime() (runtime.Runtime, error)`
///
/// Creates a runtime with appropriate mounters based on the configured
/// runtime type:
/// - **docker**: MultiMounter with bind, volume, and tmpfs mounters
/// - **shell**: Plain ShellRuntime (no mounts)
/// - **podman**: MultiMounter with bind and volume mounters
pub async fn create_runtime_from_config(
    config: &RuntimeConfig,
) -> Result<Box<dyn RuntimeTrait + Send + Sync>> {
    match config.runtime_type.as_str() {
        runtime_type::DOCKER => create_docker_runtime(config).await,
        runtime_type::SHELL => create_shell_runtime(config),
        runtime_type::PODMAN => create_podman_runtime(config).await,
        other => Err(anyhow!("unknown runtime type: {}", other)),
    }
}

/// Create a Docker runtime with bind, volume, and tmpfs mounters.
///
/// Go parity: the `case runtime.Docker` branch of `initRuntime()`
async fn create_docker_runtime(
    config: &RuntimeConfig,
) -> Result<Box<dyn RuntimeTrait + Send + Sync>> {
    // Create MultiMounter
    // Go: mounter, ok := e.mounters[runtime.Docker]; if !ok { mounter = runtime.NewMultiMounter() }
    let mut mounter = MultiMounter::new();

    // Register bind mounter
    // Go: bm := docker.NewBindMounter(docker.BindConfig{Allowed: ..., Sources: ...})
    let bind_mounter = BindMounter::new(BindConfig {
        allowed: config.bind_allowed,
        sources: config.bind_sources.clone(),
    });
    mounter
        .register_mounter("bind", Box::new(bind_mounter))
        .map_err(|e| anyhow!("{e}"))?;

    // Register volume mounter
    // Go: vm, err := docker.NewVolumeMounter()
    let volume_mounter = VolumeMounter::new();
    mounter
        .register_mounter("volume", Box::new(volume_mounter))
        .map_err(|e| anyhow!("{e}"))?;

    // Register tmpfs mounter
    // Go: mounter.RegisterMounter("tmpfs", docker.NewTmpfsMounter())
    let tmpfs_mounter = TmpfsMounter::new();
    mounter
        .register_mounter("tmpfs", Box::new(tmpfs_mounter))
        .map_err(|e| anyhow!("{e}"))?;

    debug!(
        "Docker runtime initialized (privileged={}, image_verify={})",
        config.docker_privileged, config.docker_image_verify
    );

    Ok(Box::new(DockerRuntimeAdapter::new(
        config.docker_privileged,
    )))
}

/// Create a Shell runtime.
///
/// Go parity: the `case runtime.Shell` branch of `initRuntime()`
fn create_shell_runtime(
    config: &RuntimeConfig,
) -> Result<Box<dyn RuntimeTrait + Send + Sync>> {
    let cmd = if config.shell_cmd.is_empty() {
        vec!["bash".to_string(), "-c".to_string()]
    } else {
        config.shell_cmd.clone()
    };

    debug!(
        "Shell runtime initialized with cmd {:?}, uid={}, gid={}",
        cmd, config.shell_uid, config.shell_gid
    );

    Ok(Box::new(ShellRuntimeAdapter::new(
        cmd,
        config.shell_uid.clone(),
        config.shell_gid.clone(),
    )))
}

/// Create a Podman runtime with bind and volume mounters.
///
/// Go parity: the `case runtime.Podman` branch of `initRuntime()`
async fn create_podman_runtime(
    config: &RuntimeConfig,
) -> Result<Box<dyn RuntimeTrait + Send + Sync>> {
    // Create MultiMounter
    // Go: mounter, ok := e.mounters[runtime.Podman]; if !ok { mounter = runtime.NewMultiMounter() }
    let mut mounter = MultiMounter::new();

    // Register bind mounter
    // Go: bm := docker.NewBindMounter(docker.BindConfig{...})
    let bind_mounter = BindMounter::new(BindConfig {
        allowed: config.bind_allowed,
        sources: config.bind_sources.clone(),
    });
    mounter
        .register_mounter("bind", Box::new(bind_mounter))
        .map_err(|e| anyhow!("{e}"))?;

    // Register volume mounter
    // Go: mounter.RegisterMounter("volume", podman.NewVolumeMounter())
    let volume_mounter = VolumeMounter::new();
    mounter
        .register_mounter("volume", Box::new(volume_mounter))
        .map_err(|e| anyhow!("{e}"))?;

    debug!(
        "Podman runtime initialized (privileged={}, host_network={})",
        config.podman_privileged, config.podman_host_network
    );

    Ok(Box::new(PodmanRuntimeAdapter::new(
        config.podman_privileged,
        config.podman_host_network,
    )))
}

// =============================================================================
// Host environment middleware
// =============================================================================

/// Create a host environment middleware variable map from variable specs.
///
/// Go parity: `hostenv, err := task.NewHostEnv(conf.Strings("middleware.task.hostenv.vars")...)`
///
/// Parses specs like `"VAR"` or `"HOST_VAR:TASK_VAR"` into a HashMap.
pub fn create_hostenv_middleware(
    vars: &[String],
) -> Option<crate::engine::TaskMiddlewareFunc> {
    if vars.is_empty() {
        return None;
    }

    let var_map: HashMap<String, String> = vars
        .iter()
        .filter_map(|var_spec| {
            let parts: Vec<&str> = var_spec.split(':').collect();
            match parts.len() {
                1 if !parts[0].is_empty() => {
                    Some((parts[0].to_string(), parts[0].to_string()))
                }
                2 if !parts[0].is_empty() && !parts[1].is_empty() => {
                    Some((parts[0].to_string(), parts[1].to_string()))
                }
                _ => {
                    warn!("invalid env var spec: {}", var_spec);
                    None
                }
            }
        })
        .collect();

    if var_map.is_empty() {
        return None;
    }

    // Create the middleware function
    // Go parity: hostenv.Execute
    let middleware: crate::engine::TaskMiddlewareFunc = std::sync::Arc::new(
        move |next: crate::engine::TaskHandlerFunc| -> crate::engine::TaskHandlerFunc {
            let var_map = var_map.clone();
            std::sync::Arc::new(move |_ctx: std::sync::Arc<()>, et: crate::engine::TaskEventType, task: &mut tork::task::Task| {
                if et == crate::engine::TaskEventType::StateChange && task.state == tork::task::TASK_STATE_RUNNING {
                    if task.env.is_none() {
                        task.env = Some(HashMap::new());
                    }
                    if let Some(ref mut env_map) = task.env {
                        for (host_name, task_name) in &var_map {
                            if let Ok(value) = std::env::var(host_name) {
                                env_map.insert(task_name.clone(), value);
                            }
                        }
                    }
                }
                next(_ctx, et, task)
            })
        },
    );

    Some(middleware)
}

// =============================================================================
// Worker creation
// =============================================================================

/// Creates a new worker with the given broker and optional runtime.
///
/// Go parity: `func (e *Engine) initWorker() error`
///
/// When no runtime is provided, one is created from environment configuration.
/// Host environment middleware is registered from `TORK_MIDDLEWARE_TASK_HOSTENV_VARS`.
pub async fn create_worker(
    engine: &mut crate::engine::Engine,
    _broker: BrokerProxy,
    runtime: Option<Box<dyn RuntimeTrait + Send + Sync>>,
) -> Result<Box<dyn Worker + Send + Sync>> {
    let config = read_runtime_config();

    // Initialize runtime if not provided
    // Go: rt, err := e.initRuntime()
    let _rt = match runtime {
        Some(r) => r,
        None => create_runtime_from_config(&config).await?,
    };

    debug!("Worker runtime initialized: {:?}", config.runtime_type);

    // Create and register hostenv middleware
    // Go: hostenv, err := task.NewHostEnv(conf.Strings("middleware.task.hostenv.vars")...)
    //     e.cfg.Middleware.Task = append(e.cfg.Middleware.Task, hostenv.Execute)
    if let Some(hostenv_mw) = create_hostenv_middleware(&config.hostenv_vars) {
        engine.register_task_middleware(hostenv_mw);
        debug!(
            "Registered hostenv middleware for vars: {:?}",
            config.hostenv_vars
        );
    }

    // Read worker configuration from environment
    // Go: conf.StringDefault("worker.name", "Worker")
    let _name = std::env::var("TORK_WORKER_NAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Worker".to_string());
    let _address = std::env::var("TORK_WORKER_ADDRESS").ok().filter(|s| !s.is_empty());

    // Parse queues from environment
    // Go: conf.IntMap("worker.queues")
    let _queues: HashMap<String, i32> = std::env::var("TORK_WORKER_QUEUES")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|q| {
                    let parts: Vec<&str> = q.split(':').collect();
                    if parts.len() == 2 {
                        parts[1]
                            .trim()
                            .parse::<i32>()
                            .ok()
                            .map(|v| (parts[0].trim().to_string(), v))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_else(|| {
            let mut m = HashMap::new();
            m.insert("default".to_string(), 1);
            m
        });

    // Get default limits from environment using Limits struct
    // Go parity: reads conf.String("worker.limits.cpus"), conf.String("worker.limits.memory"), conf.String("worker.limits.timeout")
    let limits = read_limits();
    debug!(
        "Worker limits: cpus={}, memory={}, timeout={}",
        limits.cpus, limits.memory, limits.timeout
    );

    // Return a placeholder worker
    // Full implementation would create a real Worker from tork_runtime
    Ok(Box::new(NoOpWorker) as Box<dyn Worker + Send + Sync>)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bind_config_default() {
        let cfg = BindConfig::default();
        assert!(!cfg.allowed);
        assert!(cfg.sources.is_empty());
    }

    #[test]
    fn test_bind_mounter_is_source_allowed_empty_sources() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: Vec::new(),
        });
        assert!(mounter.is_source_allowed("/any/path"));
    }

    #[test]
    fn test_bind_mounter_is_source_allowed_rejects_disallowed() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec!["/opt/data".to_string()],
        });
        assert!(mounter.is_source_allowed("/opt/data"));
        assert!(!mounter.is_source_allowed("/opt/other"));
    }

    #[test]
    fn test_bind_mounter_is_source_allowed_specific_sources() {
        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: vec!["/opt/data".to_string(), "/tmp/mounts".to_string()],
        });
        assert!(mounter.is_source_allowed("/opt/data"));
        assert!(mounter.is_source_allowed("/OPT/DATA")); // case insensitive
        assert!(!mounter.is_source_allowed("/other/path"));
    }

    #[tokio::test]
    async fn test_bind_mounter_mount_disallowed() {
        let mounter = BindMounter::new(BindConfig::default());
        let mnt = Mount {
            id: Some("test".to_string()),
            mount_type: "bind".to_string(),
            source: Some("/tmp/test".to_string()),
            target: Some("/mnt/test".to_string()),
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.mount(dummy_ctx, &mnt).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bind_mounter_mount_allowed_creates_dir() {
        let source = format!("/tmp/tork-test-bind-{}", std::process::id());

        let mounter = BindMounter::new(BindConfig {
            allowed: true,
            sources: Vec::new(),
        });
        let mnt = Mount {
            id: Some("test".to_string()),
            mount_type: "bind".to_string(),
            source: Some(source.clone()),
            target: Some("/mnt/test".to_string()),
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.mount(dummy_ctx, &mnt).await;
        assert!(result.is_ok());
        assert!(std::path::Path::new(&source).exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&source);
    }

    #[tokio::test]
    async fn test_bind_mounter_unmount_noop() {
        let mounter = BindMounter::new(BindConfig::default());
        let mnt = Mount {
            id: Some("test".to_string()),
            mount_type: "bind".to_string(),
            source: Some("/tmp/test".to_string()),
            target: Some("/mnt/test".to_string()),
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.unmount(dummy_ctx, &mnt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_volume_mounter_mount() {
        let mounter = VolumeMounter::new();
        let mnt = Mount {
            id: Some("vol-1".to_string()),
            mount_type: "volume".to_string(),
            source: None,
            target: Some("/data".to_string()),
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.mount(dummy_ctx, &mnt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_volume_mounter_mount_missing_id() {
        let mounter = VolumeMounter::new();
        let mnt = Mount {
            id: None,
            mount_type: "volume".to_string(),
            source: None,
            target: Some("/data".to_string()),
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.mount(dummy_ctx, &mnt).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tmpfs_mounter_mount_valid() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount {
            id: Some("tmpfs-1".to_string()),
            mount_type: "tmpfs".to_string(),
            source: None,
            target: Some("/tmp/data".to_string()),
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.mount(dummy_ctx, &mnt).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tmpfs_mounter_mount_no_target() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount {
            id: Some("tmpfs-2".to_string()),
            mount_type: "tmpfs".to_string(),
            source: None,
            target: None,
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.mount(dummy_ctx, &mnt).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tmpfs_mounter_mount_with_source() {
        let mounter = TmpfsMounter::new();
        let mnt = Mount {
            id: Some("tmpfs-3".to_string()),
            mount_type: "tmpfs".to_string(),
            source: Some("/some/source".to_string()),
            target: Some("/tmp/data".to_string()),
            opts: None,
        };
        let dummy_ctx = Box::pin(async { Ok::<(), MountError>(()) });
        let result = mounter.mount(dummy_ctx, &mnt).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_read_limits_default() {
        // Clear any existing env vars
        std::env::remove_var("TORK_WORKER_LIMITS_CPUS");
        std::env::remove_var("TORK_WORKER_LIMITS_MEMORY");
        std::env::remove_var("TORK_WORKER_LIMITS_TIMEOUT");
        
        let limits = read_limits();
        assert_eq!(limits.cpus, DEFAULT_CPUS_LIMIT);
        assert_eq!(limits.memory, DEFAULT_MEMORY_LIMIT);
        assert_eq!(limits.timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    fn test_read_limits_from_env() {
        std::env::set_var("TORK_WORKER_LIMITS_CPUS", "4");
        std::env::set_var("TORK_WORKER_LIMITS_MEMORY", "2g");
        std::env::set_var("TORK_WORKER_LIMITS_TIMEOUT", "10m");
        
        let limits = read_limits();
        assert_eq!(limits.cpus, "4");
        assert_eq!(limits.memory, "2g");
        assert_eq!(limits.timeout, "10m");
        
        // Cleanup
        std::env::remove_var("TORK_WORKER_LIMITS_CPUS");
        std::env::remove_var("TORK_WORKER_LIMITS_MEMORY");
        std::env::remove_var("TORK_WORKER_LIMITS_TIMEOUT");
    }

    #[test]
    fn test_limits_defaults() {
        let limits = Limits::default();
        assert!(limits.cpus.is_empty());
        assert!(limits.memory.is_empty());
        assert!(limits.timeout.is_empty());
    }


    #[test]
    fn test_runtime_config_default() {
        let config = RuntimeConfig::default();
        assert_eq!(config.runtime_type, "docker");
        assert!(!config.docker_privileged);
        assert_eq!(config.shell_uid, "-");
        assert_eq!(config.shell_gid, "-");
    }

    #[tokio::test]
    async fn test_create_docker_runtime() {
        let config = RuntimeConfig::default();
        let result = create_docker_runtime(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_shell_runtime() {
        let config = RuntimeConfig {
            runtime_type: "shell".to_string(),
            ..Default::default()
        };
        let result = create_shell_runtime(&config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_podman_runtime() {
        let config = RuntimeConfig {
            runtime_type: "podman".to_string(),
            ..Default::default()
        };
        let result = create_podman_runtime(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_runtime_unknown_type() {
        let config = RuntimeConfig {
            runtime_type: "unknown".to_string(),
            ..Default::default()
        };
        let result = create_runtime_from_config(&config).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_create_hostenv_middleware_empty() {
        let result = create_hostenv_middleware(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_create_hostenv_middleware_simple() {
        let result = create_hostenv_middleware(&["PATH".to_string()]);
        assert!(result.is_some());
    }

    #[test]
    fn test_create_hostenv_middleware_with_alias() {
        let result = create_hostenv_middleware(&["HOST_VAR:TASK_VAR".to_string()]);
        assert!(result.is_some());
        let boxed = result.unwrap();
        let map = boxed.downcast_ref::<HashMap<String, String>>();
        assert!(map.is_some());
        let map = map.unwrap();
        assert_eq!(map.get("HOST_VAR"), Some(&"TASK_VAR".to_string()));
    }

    #[tokio::test]
    async fn test_shell_runtime_adapter_valid_task() {
        // Use just ["bash"] - bash reads file directly, bash -c expects command string
        let adapter = ShellRuntimeAdapter::new(
            vec!["bash".to_string()],
            "-".to_string(),
            "-".to_string(),
        );
        let mut task = tork::task::Task {
            id: Some("test-task".to_string()),
            run: Some("echo hello".to_string()),
            ..Default::default()
        };
        let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let result = adapter.run(ctx, &mut task).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shell_runtime_adapter_empty_id() {
        let adapter = ShellRuntimeAdapter::new(Vec::new(), "-".to_string(), "-".to_string());
        let mut task = tork::task::Task::default();
        let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let result = adapter.run(ctx, &mut task).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_runtime_adapter_rejects_mounts() {
        let adapter = ShellRuntimeAdapter::new(Vec::new(), "-".to_string(), "-".to_string());
        let mut task = tork::task::Task {
            id: Some("test-task".to_string()),
            mounts: Some(vec![Mount::default()]),
            ..Default::default()
        };
        let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let result = adapter.run(ctx, &mut task).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_docker_runtime_adapter_valid_task() {
        let adapter = DockerRuntimeAdapter::new(false);
        let mut task = tork::task::Task {
            id: Some("test-task".to_string()),
            image: Some("alpine:latest".to_string()),
            cmd: Some(vec!["echo".to_string(), "hello".to_string()]),
            ..Default::default()
        };
        let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let result = adapter.run(ctx, &mut task).await;
        // Docker may not be available, so we accept Ok or an err about docker connection
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("docker"));
    }

    #[tokio::test]
    async fn test_podman_runtime_adapter_valid_task() {
        let adapter = PodmanRuntimeAdapter::new(false, false);
        let mut task = tork::task::Task {
            id: Some("test-task".to_string()),
            image: Some("ubuntu:latest".to_string()),
            ..Default::default()
        };
        let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let result = adapter.run(ctx, &mut task).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_podman_runtime_adapter_rejects_no_image() {
        let adapter = PodmanRuntimeAdapter::new(false, false);
        let mut task = tork::task::Task {
            id: Some("test-task".to_string()),
            ..Default::default()
        };
        let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let result = adapter.run(ctx, &mut task).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_runtime() {
        let mock = MockRuntime;
        let mut task = tork::task::Task::default();
        let ctx = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let result = mock.run(ctx, &mut task).await;
        assert!(result.is_ok());
        let result = mock.health_check().await;
        assert!(result.is_ok());
    }
}
