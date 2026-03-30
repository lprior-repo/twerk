//! Podman runtime implementation
//!
//! Implements the Runtime trait for PodmanRuntime, providing
//! task execution using podman CLI.

pub use super::types::{Broker, MountType, PodmanConfig};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, warn};

use twerk_core::id::TaskId;
use twerk_core::uuid::new_uuid;

use super::errors::PodmanError;
use super::slug::make as slugify;
use super::types::{
    CoreTask, Mount, Mounter, PullRequest, RegistryCredentials, DEFAULT_WORKDIR, HOST_NETWORK_NAME,
    PROGRESS_POLL_INTERVAL,
};

// ── Runtime struct ────────────────────────────────────────────────

pub struct PodmanRuntime {
    pub(crate) broker: Option<Box<dyn Broker + Send + Sync>>,
    pub(crate) pullq: mpsc::Sender<PullRequest>,
    pub(crate) images: Arc<RwLock<HashMap<String, Instant>>>,
    pub(crate) tasks: Arc<RwLock<HashMap<String, String>>>,
    pub(crate) active_tasks: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) mounter: Arc<dyn Mounter + Send + Sync>,
    pub(crate) privileged: bool,
    pub(crate) host_network: bool,
    pub(crate) image_verify: bool,
    pub(crate) image_ttl: Duration,
}

impl std::fmt::Debug for PodmanRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodmanRuntime")
            .field("broker", &"<broker>")
            .field("privileged", &self.privileged)
            .field("host_network", &self.host_network)
            .field("mounter", &"<mounter>")
            .field("image_verify", &self.image_verify)
            .field("image_ttl", &self.image_ttl)
            .finish()
    }
}

impl PodmanRuntime {
    /// Creates a new PodmanRuntime from configuration.
    #[must_use]
    pub fn new(config: PodmanConfig) -> Self {
        let (tx, rx) = mpsc::channel::<PullRequest>(100);
        let mounter: Arc<dyn Mounter + Send + Sync> = config
            .mounter
            .map(|m| unsafe {
                let raw = Box::into_raw(m);
                Arc::from_raw(raw as *const (dyn Mounter + Send + Sync))
            })
            .unwrap_or_else(|| Arc::new(super::volume::VolumeMounter::new()));
        let image_ttl = config.image_ttl.unwrap_or(super::types::DEFAULT_IMAGE_TTL);

        let images = Arc::new(RwLock::new(HashMap::new()));
        let tasks = Arc::new(RwLock::new(HashMap::new()));
        let active_tasks = Arc::new(std::sync::atomic::AtomicU64::new(0));

        Self::start_puller(rx, config.broker.clone());
        Self::start_pruner(images.clone(), active_tasks.clone(), image_ttl);

        Self {
            broker: config.broker,
            pullq: tx,
            images,
            tasks,
            active_tasks,
            mounter,
            privileged: config.privileged,
            host_network: config.host_network,
            image_verify: config.image_verify,
            image_ttl,
        }
    }

    fn start_puller(
        mut rx: mpsc::Receiver<PullRequest>,
        broker: Option<Box<dyn Broker + Send + Sync>>,
    ) {
        tokio::spawn(async move {
            while let Some(pr) = rx.recv().await {
                let image = pr.image.clone();
                let registry = pr.registry.clone();
                let result = Self::do_pull_request(&image, registry, broker.as_deref()).await;
                let _ = pr.respond_to.send(result);
            }
        });
    }

    fn start_pruner(
        images: Arc<RwLock<HashMap<String, Instant>>>,
        active_tasks: Arc<std::sync::atomic::AtomicU64>,
        ttl: Duration,
    ) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(super::types::PRUNE_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(e) = Self::prune_images(&images, &active_tasks, ttl).await {
                    tracing::error!("error pruning images: {}", e);
                }
            }
        });
    }
}

impl Clone for PodmanRuntime {
    fn clone(&self) -> Self {
        Self {
            broker: self.broker.clone(),
            pullq: self.pullq.clone(),
            images: Arc::clone(&self.images),
            tasks: Arc::clone(&self.tasks),
            active_tasks: Arc::clone(&self.active_tasks),
            mounter: self.mounter.clone(),
            privileged: self.privileged,
            host_network: self.host_network,
            image_verify: self.image_verify,
            image_ttl: self.image_ttl,
        }
    }
}

// ── Runtime trait implementation ──────────────────────────────────

impl crate::runtime::Runtime for PodmanRuntime {
    fn run(&self, task: &CoreTask) -> crate::runtime::BoxedFuture<()> {
        let mut task_clone = task.clone();
        let broker = self.broker.clone();
        let pullq = self.pullq.clone();
        let images = Arc::clone(&self.images);
        let tasks = Arc::clone(&self.tasks);
        let active_tasks = Arc::clone(&self.active_tasks);
        let mounter = Arc::clone(&self.mounter);
        let privileged = self.privileged;
        let host_network = self.host_network;
        let image_verify = self.image_verify;
        let image_ttl = self.image_ttl;

        Box::pin(async move {
            let runtime = PodmanRuntime {
                broker,
                pullq,
                images,
                tasks,
                active_tasks,
                mounter,
                privileged,
                host_network,
                image_verify,
                image_ttl,
            };
            if let Err(e) = runtime.run_inner(&mut task_clone).await {
                tracing::error!(
                    "task {} failed: {}",
                    task_clone.id.as_ref().map_or("", |id| id.as_str()),
                    e
                );
            }
            Ok(())
        })
    }

    fn stop(
        &self,
        task: &CoreTask,
    ) -> crate::runtime::BoxedFuture<crate::runtime::ShutdownResult<std::process::ExitCode>> {
        let task_id = task.id.as_ref().map_or(String::new(), |id| id.to_string());
        let tasks = Arc::clone(&self.tasks);

        Box::pin(async move {
            let container_id = {
                let tasks_guard = tasks.read().await;
                tasks_guard.get(&task_id).cloned()
            };

            if let Some(cid) = container_id {
                if let Err(e) = PodmanRuntime::stop_container_static(&cid).await {
                    tracing::warn!("error stopping container {}: {}", cid, e);
                    return Err(anyhow::anyhow!(e));
                }
                let mut tasks_guard = tasks.write().await;
                tasks_guard.remove(&cid);
            }

            Ok(Ok(std::process::ExitCode::SUCCESS))
        })
    }

    fn health_check(&self) -> crate::runtime::BoxedFuture<()> {
        let pullq = self.pullq.clone();
        let images = Arc::clone(&self.images);
        let tasks = Arc::clone(&self.tasks);
        let active_tasks = Arc::clone(&self.active_tasks);
        let mounter = Arc::clone(&self.mounter);
        let privileged = self.privileged;
        let host_network = self.host_network;
        let image_verify = self.image_verify;
        let image_ttl = self.image_ttl;

        Box::pin(async move {
            let runtime = PodmanRuntime {
                broker: None,
                pullq,
                images,
                tasks,
                active_tasks,
                mounter,
                privileged,
                host_network,
                image_verify,
                image_ttl,
            };
            if let Err(e) = runtime.health_check_inner().await {
                tracing::error!("podman health check failed: {}", e);
            }
            Ok(())
        })
    }
}

// ── Implementation methods ───────────────────────────────────────

impl PodmanRuntime {
    /// Main run method - validates task and executes pre/main/post tasks
    pub(crate) async fn run_inner(&self, task: &mut CoreTask) -> Result<(), PodmanError> {
        // Validate task - must have ID, image, and name
        let _task_id = task.id.as_ref().ok_or(PodmanError::TaskIdRequired)?;
        let _task_name = task.name.as_ref().ok_or(PodmanError::NameRequired)?;
        let _task_image = task.image.as_ref().ok_or(PodmanError::ImageRequired)?;

        // Check for sidecars (not supported)
        if task.sidecars.as_ref().is_some_and(|s| !s.is_empty()) {
            return Err(PodmanError::SidecarsNotSupported);
        }

        // Check host network access
        if !self.host_network {
            if let Some(ref networks) = task.networks {
                if networks.iter().any(|n| n == HOST_NETWORK_NAME) {
                    return Err(PodmanError::HostNetworkingDisabled);
                }
            }
        }

        // Mount volumes and execute
        let mounted_mounts = self.prepare_mounts(task).await?;

        let result = self.execute_task_tree(task, &mounted_mounts).await;

        // Cleanup mounts
        self.cleanup_mounts(&mounted_mounts).await;

        result
    }

    /// Prepare mounts for task
    async fn prepare_mounts(&self, task: &CoreTask) -> Result<Vec<Mount>, PodmanError> {
        let mut mounted = Vec::new();
        if let Some(ref mounts) = task.mounts {
            for core_mnt in mounts {
                let mut mnt = Mount::from(core_mnt);
                mnt.id = core_mnt.id.clone().unwrap_or_else(new_uuid);
                if let Err(e) = self.mounter.mount(&mut mnt) {
                    error!("error mounting volume: {}", e);
                    return Err(PodmanError::WorkdirCreation(e.to_string()));
                }
                mounted.push(mnt);
            }
        }
        Ok(mounted)
    }

    /// Cleanup mounts after task execution
    async fn cleanup_mounts(&self, mounts: &[Mount]) {
        for mnt in mounts {
            if let Err(e) = self.mounter.unmount(mnt) {
                warn!("error unmounting volume {}: {}", mnt.target, e);
            }
        }
    }

    /// Execute pre tasks, main task, and post tasks
    async fn execute_task_tree(
        &self,
        task: &CoreTask,
        mounted_mounts: &[Mount],
    ) -> Result<(), PodmanError> {
        // Convert mounted mounts back to CoreMount format for task execution
        let task_mounts: Vec<twerk_core::mount::Mount> = mounted_mounts
            .iter()
            .map(|m| twerk_core::mount::Mount {
                id: Some(m.id.clone()),
                mount_type: Some(m.mount_type.as_str().to_string()),
                source: Some(m.source.clone()),
                target: Some(m.target.clone()),
                opts: m.opts.clone(),
            })
            .collect();

        // Execute pre tasks
        if let Some(ref pre_tasks) = task.pre {
            for pre in pre_tasks {
                let mut pre_clone = pre.clone();
                pre_clone.id = Some(TaskId::new(new_uuid()));
                pre_clone.mounts = Some(task_mounts.clone());
                pre_clone.networks = task.networks.clone();
                pre_clone.limits = task.limits.clone();
                self.do_run(&mut pre_clone).await?;
            }
        }

        // Execute main task
        let mut main_clone = task.clone();
        main_clone.mounts = Some(task_mounts.clone());
        self.do_run(&mut main_clone).await?;

        // Execute post tasks
        if let Some(ref post_tasks) = task.post {
            for post in post_tasks {
                let mut post_clone = post.clone();
                post_clone.id = Some(TaskId::new(new_uuid()));
                post_clone.mounts = Some(task_mounts.clone());
                post_clone.networks = task.networks.clone();
                post_clone.limits = task.limits.clone();
                self.do_run(&mut post_clone).await?;
            }
        }

        Ok(())
    }

    /// Execute a single task (main, pre, or post)
    async fn do_run(&self, task: &mut CoreTask) -> Result<(), PodmanError> {
        self.active_tasks
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let result = self.do_run_inner(task).await;

        self.active_tasks
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        result
    }

    /// Inner execution - setup workdir and run container
    async fn do_run_inner(&self, task: &mut CoreTask) -> Result<(), PodmanError> {
        // Setup work directory
        let task_id_str = task.id.as_ref().map_or("unknown", |id| id.as_str());
        let workdir = std::env::temp_dir().join("twerk").join(task_id_str);
        tokio::fs::create_dir_all(&workdir)
            .await
            .map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?;

        // Create output and progress files
        let output_file = workdir.join("stdout");
        let progress_file = workdir.join("progress");

        tokio::fs::File::create(&output_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&output_file, PermissionsExt::from_mode(0o777))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        tokio::fs::File::create(&progress_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&progress_file, PermissionsExt::from_mode(0o777))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        // Write entrypoint script
        let entrypoint_path = workdir.join("entrypoint.sh");
        let run_script = if let Some(ref run) = task.run {
            run.clone()
        } else {
            task.cmd.as_ref().map_or(String::new(), |cmd| cmd.join(" "))
        };

        tokio::fs::write(&entrypoint_path, &run_script)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::set_permissions(&entrypoint_path, PermissionsExt::from_mode(0o755))
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        // Write task files
        if let Some(ref files) = task.files {
            if !files.is_empty() {
                let files_dir = workdir.join("workdir");
                tokio::fs::create_dir_all(&files_dir)
                    .await
                    .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

                for (filename, contents) in files {
                    let file_path = files_dir.join(filename);
                    if let Some(parent) = file_path.parent() {
                        tokio::fs::create_dir_all(parent)
                            .await
                            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
                    }
                    tokio::fs::write(&file_path, contents)
                        .await
                        .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
                }
            }
        }

        // Execute container
        let result = self
            .execute_container(task, &workdir, &output_file, &progress_file)
            .await;

        // Cleanup workdir
        if let Err(e) = tokio::fs::remove_dir_all(&workdir).await {
            warn!("error removing workdir {:?}: {}", workdir, e);
        }

        result
    }

    /// Execute container and handle logs
    async fn execute_container(
        &self,
        task: &mut CoreTask,
        workdir: &Path,
        output_file: &Path,
        progress_file: &Path,
    ) -> Result<(), PodmanError> {
        // Convert to owned PathBuf for async tasks
        let progress_file_buf = progress_file.to_path_buf();
        let task_id_str = task.id.as_ref().map_or("unknown", |id| id.as_str());
        let image = task.image.as_ref().ok_or(PodmanError::ImageRequired)?;

        // Pull image
        let registry = task.registry.as_ref().and_then(|r| {
            let username = r.username.as_ref()?;
            let password = r.password.as_ref()?;
            if username.is_empty() {
                None
            } else {
                Some(RegistryCredentials {
                    username: username.clone(),
                    password: password.clone(),
                })
            }
        });

        self.image_pull(image, registry).await?;

        // Optional image verification
        if self.image_verify {
            if let Err(e) = Self::verify_image(image).await {
                error!("image {} is invalid or corrupted: {}", image, e);
                let mut rm_cmd = Command::new("podman");
                rm_cmd.arg("image").arg("rm").arg("-f").arg(image);
                let _ = rm_cmd.output().await;
                return Err(e);
            }
        }

        // Build entrypoint
        let entrypoint = if task.entrypoint.as_ref().is_some_and(|e| !e.is_empty()) {
            task.entrypoint.clone().unwrap()
        } else {
            vec!["sh".to_string()]
        };

        // Build podman create command
        let mut create_cmd = self.build_create_command(workdir, task, entrypoint.clone());

        if self.privileged {
            create_cmd.arg("--privileged");
        }

        // Create container
        let create_output = tokio::time::timeout(Duration::from_secs(30), create_cmd.output())
            .await
            .map_err(|_| {
                PodmanError::ContainerCreation("create timed out after 30 seconds".to_string())
            })?
            .map_err(|e| PodmanError::ContainerCreation(e.to_string()))?;

        if !create_output.status.success() {
            return Err(PodmanError::ContainerCreation(
                String::from_utf8_lossy(&create_output.stderr).to_string(),
            ));
        }

        let container_id = String::from_utf8_lossy(&create_output.stdout)
            .trim()
            .to_string();

        if container_id.is_empty() {
            return Err(PodmanError::ContainerCreation(
                "empty container ID".to_string(),
            ));
        }

        debug!("created container {}", container_id);

        self.tasks
            .write()
            .await
            .insert(task_id_str.to_string(), container_id.clone());

        // Ensure container is stopped on exit
        struct ContainerGuard {
            container_id: String,
            tasks: Arc<RwLock<HashMap<String, String>>>,
        }
        impl Drop for ContainerGuard {
            fn drop(&mut self) {
                let cid = self.container_id.clone();
                let tasks = self.tasks.clone();
                tokio::spawn(async move {
                    if let Err(e) = PodmanRuntime::stop_container_static(&cid).await {
                        warn!("error stopping container {}: {}", cid, e);
                    }
                    tasks.write().await.remove(&cid);
                });
            }
        }
        let _guard = ContainerGuard {
            container_id: container_id.clone(),
            tasks: Arc::clone(&self.tasks),
        };

        // Start progress reporting
        let progress_task_id = task_id_str.to_string();
        let broker = self.broker.clone();
        let progress_handle = tokio::spawn(async move {
            PodmanRuntime::report_progress(
                &progress_task_id,
                &progress_file_buf,
                broker.as_deref(),
            )
            .await;
        });

        // Start container
        let mut start_cmd = Command::new("podman");
        start_cmd.arg("start").arg(&container_id);
        start_cmd.stdout(std::process::Stdio::piped());
        start_cmd.stderr(std::process::Stdio::piped());

        let start_output = start_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerStart(e.to_string()))?;

        if !start_output.status.success() {
            return Err(PodmanError::ContainerStart(
                String::from_utf8_lossy(&start_output.stderr).to_string(),
            ));
        }

        // Read logs
        let logs_broker = self.broker.clone();
        let logs_task_id = task_id_str.to_string();
        let mut logs_cmd = Command::new("podman");
        logs_cmd.arg("logs").arg("--follow").arg(&container_id);
        logs_cmd.stdout(std::process::Stdio::piped());
        logs_cmd.stderr(std::process::Stdio::piped());

        let mut child = logs_cmd
            .spawn()
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        // Ship logs to broker
        if let Some(stdout) = child.stdout.take() {
            let broker_clone = logs_broker.clone();
            let tid = logs_task_id.clone();
            tokio::spawn(async move {
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    debug!("[podman:stdout] {}", line);
                    if let Some(ref b) = broker_clone {
                        b.ship_log(&tid, &line);
                    }
                }
            });
        }

        child
            .wait()
            .await
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        progress_handle.abort();

        // Check exit code
        let mut inspect_cmd = Command::new("podman");
        inspect_cmd
            .arg("inspect")
            .arg("--format")
            .arg("{{.State.ExitCode}}")
            .arg(&container_id);
        let inspect_output = inspect_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerInspect(e.to_string()))?;

        let exit_code = String::from_utf8_lossy(&inspect_output.stdout)
            .trim()
            .to_string();
        if exit_code != "0" {
            return Err(PodmanError::ContainerExitCode(exit_code));
        }

        // Read output
        let output = tokio::fs::read_to_string(&output_file)
            .await
            .map_err(|e| PodmanError::OutputRead(e.to_string()))?;
        task.result = Some(output);

        Ok(())
    }

    /// Build podman create command with all options
    fn build_create_command(
        &self,
        workdir: &Path,
        task: &CoreTask,
        entrypoint: Vec<String>,
    ) -> Command {
        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create");
        create_cmd
            .arg("-v")
            .arg(format!("{}:/twerk", workdir.display()));

        if !entrypoint.is_empty() {
            create_cmd.arg("--entrypoint").arg(&entrypoint[0]);
        }

        // Environment variables
        let env_vars: Vec<String> = task.env.as_ref().map_or(Vec::new(), |env| {
            env.iter().map(|(k, v)| format!("{}={}", k, v)).collect()
        });

        let mut all_env = env_vars;
        all_env.push("TWERK_OUTPUT=/twerk/stdout".to_string());
        all_env.push("TWERK_PROGRESS=/twerk/progress".to_string());

        for env in &all_env {
            create_cmd.arg("-e").arg(env);
        }

        // Networks
        if let Some(ref networks) = task.networks {
            let task_name = task.name.as_deref().unwrap_or("unknown");
            for network in networks {
                if network == HOST_NETWORK_NAME {
                    create_cmd.arg("--network").arg(network);
                } else {
                    let alias = slugify(task_name);
                    create_cmd.arg("--network").arg(network);
                    create_cmd.arg("--network-alias").arg(alias);
                }
            }
        }

        // Mounts
        if let Some(ref mounts) = task.mounts {
            for mnt in mounts {
                let mount_type_str = mnt.mount_type.as_deref().unwrap_or("volume");
                match mount_type_str {
                    "bind" | "volume" => {
                        let source = mnt.source.as_deref().unwrap_or("");
                        let target = mnt.target.as_deref().unwrap_or("");
                        create_cmd.arg("-v").arg(format!("{}:{}", source, target));
                    }
                    "tmpfs" => {
                        let target = mnt.target.as_deref().unwrap_or("");
                        create_cmd.arg("--tmpfs").arg(target);
                    }
                    _ => {
                        let source = mnt.source.as_deref().unwrap_or("");
                        let target = mnt.target.as_deref().unwrap_or("");
                        create_cmd.arg("-v").arg(format!("{}:{}", source, target));
                    }
                }
            }
        }

        // Resource limits
        if let Some(ref limits) = task.limits {
            if let Some(ref cpus) = limits.cpus {
                if !cpus.is_empty() {
                    create_cmd.arg("--cpus").arg(cpus);
                }
            }
            if let Some(ref memory) = limits.memory {
                if !memory.is_empty() {
                    let bytes = Self::parse_memory(memory).unwrap_or(0);
                    create_cmd.arg("--memory").arg(bytes.to_string());
                }
            }
        }

        // GPU support
        if let Some(ref gpus) = task.gpus {
            if !gpus.is_empty() {
                create_cmd.arg("--gpus").arg(gpus);
            }
        }

        // Workdir
        let effective_workdir = if task.workdir.is_some() {
            task.workdir.clone()
        } else if task.files.as_ref().is_some_and(|f| !f.is_empty()) {
            Some(DEFAULT_WORKDIR.to_string())
        } else {
            None
        };

        if let Some(ref wd) = effective_workdir {
            if !wd.is_empty() {
                create_cmd.arg("-w").arg(wd);
            }
        }

        // Image and entrypoint args
        if let Some(ref image) = task.image {
            create_cmd.arg(image);
        }
        for arg in entrypoint.iter().skip(1) {
            create_cmd.arg(arg);
        }
        create_cmd.arg("/twerk/entrypoint.sh");

        create_cmd.stdout(std::process::Stdio::piped());
        create_cmd.stderr(std::process::Stdio::piped());

        create_cmd
    }

    /// Parse memory string to bytes
    fn parse_memory(memory: &str) -> Option<u64> {
        let memory = memory.trim();
        let (num_str, multiplier) = if let Some(suffix) = memory.strip_suffix("gb") {
            (suffix.trim_end(), 1_073_741_824u64)
        } else if let Some(suffix) = memory.strip_suffix("g") {
            (suffix.trim_end(), 1_073_741_824u64)
        } else if let Some(suffix) = memory.strip_suffix("mb") {
            (suffix.trim_end(), 1_048_576u64)
        } else if let Some(suffix) = memory.strip_suffix("m") {
            (suffix.trim_end(), 1_048_576u64)
        } else if let Some(suffix) = memory.strip_suffix("kb") {
            (suffix.trim_end(), 1024u64)
        } else if let Some(suffix) = memory.strip_suffix("k") {
            (suffix.trim_end(), 1024u64)
        } else if let Some(suffix) = memory.strip_suffix("b") {
            (suffix.trim_end(), 1u64)
        } else {
            (memory, 1u64)
        };

        let value: f64 = num_str.parse().ok()?;
        Some((value * multiplier as f64) as u64)
    }

    /// Pull image via queue
    async fn image_pull(
        &self,
        image: &str,
        registry: Option<RegistryCredentials>,
    ) -> Result<(), PodmanError> {
        // Check cache
        {
            let images = self.images.read().await;
            if images.contains_key(image) {
                drop(images);
                self.images
                    .write()
                    .await
                    .insert(image.to_string(), Instant::now());
                return Ok(());
            }
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pullq
            .send(PullRequest {
                respond_to: tx,
                image: image.to_string(),
                registry,
            })
            .await
            .map_err(|_| PodmanError::ImagePull("channel closed".to_string()))?;

        rx.await
            .map_err(|_| PodmanError::ImagePull("cancelled".to_string()))??;

        self.images
            .write()
            .await
            .insert(image.to_string(), Instant::now());

        Ok(())
    }

    /// Verify image can be used
    async fn verify_image(image: &str) -> Result<(), PodmanError> {
        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create").arg(image).arg("true");
        create_cmd.stdout(std::process::Stdio::piped());
        create_cmd.stderr(std::process::Stdio::piped());

        let create_output = create_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ImageVerification(e.to_string()))?;

        if !create_output.status.success() {
            return Err(PodmanError::ImageVerification(format!(
                "image {} failed verification: {}",
                image,
                String::from_utf8_lossy(&create_output.stderr)
            )));
        }

        let container_id = String::from_utf8_lossy(&create_output.stdout)
            .trim()
            .to_string();

        if container_id.is_empty() {
            return Err(PodmanError::ImageVerification(
                "empty container ID during verification".to_string(),
            ));
        }

        let mut rm_cmd = Command::new("podman");
        rm_cmd.arg("rm").arg("-f").arg(&container_id);
        let _ = rm_cmd.output().await;

        Ok(())
    }

    /// Stop and remove container
    async fn stop_container_static(container_id: &str) -> Result<(), PodmanError> {
        debug!("Attempting to stop and remove container {}", container_id);
        let mut cmd = Command::new("podman");
        cmd.arg("rm").arg("-f").arg("-t").arg("0").arg(container_id);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            PodmanError::ContainerCreation(format!(
                "failed to remove container {}: {}",
                container_id, e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PodmanError::ContainerCreation(format!(
                "failed to stop container {}: {}",
                container_id, stderr
            )));
        }
        Ok(())
    }

    /// Report progress to broker
    async fn report_progress(
        task_id: &str,
        progress_file: &Path,
        broker: Option<&(dyn Broker + Send + Sync)>,
    ) {
        loop {
            tokio::time::sleep(PROGRESS_POLL_INTERVAL).await;

            let progress = match tokio::fs::read_to_string(&progress_file).await {
                Ok(content) => {
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        0.0
                    } else {
                        trimmed.parse().unwrap_or(0.0)
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
                Err(e) => {
                    error!("error reading progress file: {}", e);
                    continue;
                }
            };

            if let Some(b) = broker {
                b.publish_task_progress(task_id, progress);
            }
        }
    }

    /// Prune stale images
    async fn prune_images(
        images: &Arc<RwLock<HashMap<String, Instant>>>,
        active_tasks: &Arc<std::sync::atomic::AtomicU64>,
        ttl: Duration,
    ) -> Result<(), PodmanError> {
        if active_tasks.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            return Ok(());
        }

        let images_guard = images.read().await;
        let stale: Vec<String> = images_guard
            .iter()
            .filter(|(_img, last_used)| last_used.elapsed() > ttl)
            .map(|(img, _)| img.clone())
            .collect();
        drop(images_guard);

        for image in &stale {
            let mut cmd = Command::new("podman");
            cmd.arg("image").arg("rm").arg(image);
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            if let Ok(output) = cmd.output().await {
                if output.status.success() {
                    debug!("pruned image {}", image);
                    images.write().await.remove(image);
                }
            }
        }

        Ok(())
    }

    /// Health check - verify podman is running
    pub async fn health_check_inner(&self) -> Result<(), PodmanError> {
        let mut cmd = Command::new("podman");
        cmd.arg("version");
        let output = cmd
            .output()
            .await
            .map_err(|_| PodmanError::PodmanNotRunning)?;

        if !output.status.success() {
            return Err(PodmanError::PodmanNotRunning);
        }

        Ok(())
    }

    /// Internal pull implementation
    async fn do_pull_request(
        image: &str,
        registry: Option<RegistryCredentials>,
        _broker: Option<&(dyn Broker + Send + Sync)>,
    ) -> Result<(), PodmanError> {
        // Check if image exists locally
        if Self::image_exists_locally(image).await {
            debug!("image {} already exists locally, skipping pull", image);
            return Ok(());
        }

        // Login to registry if credentials provided
        if let Some(ref creds) = registry {
            if !creds.username.is_empty() {
                Self::registry_login(image, &creds.username, &creds.password).await?;
            }
        }

        // Pull image
        debug!("Pulling image {}", image);
        let mut cmd = Command::new("podman");
        cmd.arg("pull").arg(image);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| PodmanError::ImagePull(e.to_string()))?;

        if !output.status.success() {
            return Err(PodmanError::ImagePull(format!(
                "podman pull failed for {}: {}",
                image,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Check if image exists locally
    async fn image_exists_locally(image: &str) -> bool {
        let output = Command::new("podman")
            .arg("inspect")
            .arg(image)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .await;
        output.is_ok_and(|out| out.status.success())
    }

    /// Login to registry
    async fn registry_login(
        image: &str,
        username: &str,
        password: &str,
    ) -> Result<(), PodmanError> {
        let registry_host = Self::extract_registry_host(image);
        debug!(
            "Logging into registry {} for user {}",
            registry_host, username
        );

        let mut cmd = Command::new("podman");
        cmd.arg("login");
        cmd.arg("--username").arg(username);
        cmd.arg("--password-stdin");
        cmd.arg(&registry_host);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| PodmanError::RegistryLogin(e.to_string()))?;

        if let Some(ref mut stdin) = child.stdin {
            use tokio::io::AsyncWriteExt;
            if let Err(_e) = stdin.write_all(password.as_bytes()).await {
                return Err(PodmanError::RegistryLogin(
                    "failed to write password to stdin".to_string(),
                ));
            }
            if let Err(_e) = stdin.shutdown().await {
                return Err(PodmanError::RegistryLogin(
                    "failed to close stdin".to_string(),
                ));
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| PodmanError::RegistryLogin(e.to_string()))?;

        if !output.status.success() {
            return Err(PodmanError::RegistryLogin(format!(
                "podman login to {} failed: {}",
                registry_host,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Extract registry host from image name
    fn extract_registry_host(image: &str) -> String {
        match image.split_once('/') {
            Some((host, _rest)) if host.contains('.') || host.contains(':') => host.to_string(),
            _ => "docker.io".to_string(),
        }
    }
}
