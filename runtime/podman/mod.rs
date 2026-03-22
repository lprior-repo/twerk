//! Podman runtime module

mod volume;

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use futures_util::StreamExt;
use itertools::Itertools;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

pub use volume::VolumeMounter;

const DEFAULT_WORKDIR: &str = "/tork/workdir";
const HOST_NETWORK_NAME: &str = "host";
const PROGRESS_POLL_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Error, Debug)]
pub enum PodmanError {
    #[error("task id is required")]
    TaskIdRequired,

    #[error("task image is required")]
    ImageRequired,

    #[error("task name is required")]
    NameRequired,

    #[error("sidecars are not supported in podman runtime")]
    SidecarsNotSupported,

    #[error("host networking is not enabled")]
    HostNetworkingDisabled,

    #[error("failed to create workdir: {0}")]
    WorkdirCreation(String),

    #[error("failed to write file: {0}")]
    FileWrite(String),

    #[error("failed to create container: {0}")]
    ContainerCreation(String),

    #[error("failed to start container: {0}")]
    ContainerStart(String),

    #[error("failed to read logs: {0}")]
    LogsRead(String),

    #[error("container exited with code: {0}")]
    ContainerExitCode(String),

    #[error("failed to read output: {0}")]
    OutputRead(String),

    #[error("failed to pull image: {0}")]
    ImagePull(String),

    #[error("unknown mount type: {0}")]
    UnknownMountType(String),

    #[error("context cancelled")]
    ContextCancelled,
}

#[derive(Default)]
pub struct PodmanConfig {
    pub broker: Option<Box<dyn Broker + Send + Sync>>,
    pub privileged: bool,
    pub host_network: bool,
    pub mounter: Option<Box<dyn Mounter + Send + Sync>>,
}

impl std::fmt::Debug for PodmanConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodmanConfig")
            .field("broker", &"<broker>")
            .field("privileged", &self.privileged)
            .field("host_network", &self.host_network)
            .field("mounter", &"<mounter>")
            .finish()
    }
}

/// Broker trait for streaming logs and progress
pub trait Broker: Send + Sync {
    fn clone_box(&self) -> Box<dyn Broker + Send + Sync>;
    fn ship_log(&self, task_id: &str, line: &str);
    fn publish_task_progress(&self, task_id: &str, progress: f64);
}

impl Clone for Box<dyn Broker + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait Mounter: Send + Sync {
    fn mount(&self, mount: &mut Mount) -> Result<(), anyhow::Error>;
    fn unmount(&self, mount: &Mount) -> Result<(), anyhow::Error>;
}

pub struct PodmanRuntime {
    broker: Option<Box<dyn Broker + Send + Sync>>,
    pullq: mpsc::Sender<PullRequest>,
    images: RwLock<HashMap<String, bool>>,
    tasks: Arc<RwLock<HashMap<String, String>>>,
    mounter: Box<dyn Mounter + Send + Sync>,
    privileged: bool,
    host_network: bool,
}

impl std::fmt::Debug for PodmanRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PodmanRuntime")
            .field("broker", &self.broker)
            .field("privileged", &self.privileged)
            .field("host_network", &self.host_network)
            .field("mounter", &"<mounter>")
            .finish()
    }
}

struct PullRequest {
    ctx: tokio::sync::oneshot::Sender<Result<(), PodmanError>>,
    image: String,
}

impl PodmanRuntime {
    pub fn new(config: PodmanConfig) -> Self {
        let (tx, rx) = mpsc::channel::<PullRequest>(100);
        let mounter = config
            .mounter
            .unwrap_or_else(|| Box::new(VolumeMounter::new()));

        let rt = Self {
            broker: config.broker,
            pullq: tx,
            images: RwLock::new(HashMap::new()),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            mounter,
            privileged: config.privileged,
            host_network: config.host_network,
        };

        // Start the puller background task
        rt.start_puller(rx);

        rt
    }

    fn start_puller(&self, mut rx: mpsc::Receiver<PullRequest>) {
        let broker = self.broker.clone();
        tokio::spawn(async move {
            while let Some(pr) = rx.recv().await {
                let image = pr.image.clone();
                let result = Self::pull_image(&image, broker.as_ref()).await;
                let _ = pr.ctx.send(result.map_err(|e| PodmanError::ImagePull(e.to_string())));
            }
        });
    }

    async fn pull_image(image: &str, broker: Option<&Box<dyn Broker + Send + Sync>>) -> Result<(), anyhow::Error> {
        // Check if image exists locally first
        if Self::image_exists_locally(image).await {
            debug!("Image {} already exists locally, skipping pull", image);
            return Ok(());
        }

        debug!("Pulling image {}", image);
        let mut cmd = Command::new("podman");
        cmd.arg("pull").arg(image);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd.output().await.context("failed to execute podman pull")?;

        if !output.status.success() {
            return Err(anyhow!(
                "podman pull failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    async fn image_exists_locally(image: &str) -> bool {
        let mut cmd = Command::new("podman");
        cmd.arg("inspect").arg(image);
        cmd.output().await.ok();
        // If inspect succeeds, image exists (exit code 0)
        // If it fails, image doesn't exist
        // We check by running the command and seeing if it succeeds
        let output = Command::new("podman")
            .arg("inspect")
            .arg(image)
            .output()
            .await;
        match output {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    pub async fn run(&self, task: &mut Task) -> Result<(), PodmanError> {
        // Validate task
        if task.id.is_empty() {
            return Err(PodmanError::TaskIdRequired);
        }
        if task.image.is_empty() {
            return Err(PodmanError::ImageRequired);
        }
        if task.name.as_ref().is_none_or(|n| n.is_empty()) {
            return Err(PodmanError::NameRequired);
        }
        if !task.sidecars.is_empty() {
            return Err(PodmanError::SidecarsNotSupported);
        }

        // Mount volumes with deferred unmounting
        let mut mounted_mounts: Vec<Mount> = Vec::new();
        for mut mount in task.mounts.clone() {
            if let Err(e) = self.mounter.mount(&mut mount) {
                error!("error mounting: {}", e);
                return Err(PodmanError::WorkdirCreation(e.to_string()));
            }
            mounted_mounts.push(mount);
        }

        // Deferred unmount on exit
        let mounter = &self.mounter;
        let unmount_result = (async {
            let result = self.run_inner(task, &mounted_mounts).await;
            // Unmount all mounted volumes
            for mount in &mounted_mounts {
                if let Err(e) = mounter.unmount(mount) {
                    error!("error unmounting volume {}: {}", mount.target, e);
                }
            }
            result
        }).await;

        unmount_result
    }

    async fn run_inner(&self, task: &mut Task, mounts: &[Mount]) -> Result<(), PodmanError> {
        // Use the provided mounts
        let task_mounts = mounts.to_vec();

        // Execute pre-tasks
        for pre in task.pre.iter_mut() {
            pre.mounts = task_mounts.clone();
            pre.networks = task.networks.clone();
            pre.limits = task.limits.clone();
            self.do_run(pre).await?;
        }

        // Run the actual task
        self.do_run(task).await?;

        // Execute post tasks
        for post in task.post.iter_mut() {
            post.mounts = task_mounts.clone();
            post.networks = task.networks.clone();
            post.limits = task.limits.clone();
            self.do_run(post).await?;
        }

        Ok(())
    }

    async fn do_run(&self, task: &mut Task) -> Result<(), PodmanError> {
        let task_id = task.id.clone();
        let workdir = tempfile::tempdir()
            .map_err(|e| PodmanError::WorkdirCreation(e.to_string()))?
            .keep();

        let workdir_for_child = workdir.clone();

        // Create output and progress files
        let output_file = workdir.join("output");
        let progress_file = workdir.join("progress");

        tokio::fs::File::create(&output_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
        tokio::fs::File::create(&progress_file)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        // Write entrypoint script
        let entrypoint_path = workdir.join("entrypoint.sh");
        let run_script = if !task.r#run.is_empty() {
            task.r#run.clone()
        } else {
            task.cmd.join(" ")
        };

        tokio::fs::write(&entrypoint_path, &run_script)
            .await
            .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

        // Pull image
        self.image_pull(&task.image).await?;

        // Create container
        let entrypoint = if task.entrypoint.is_empty() {
            vec!["sh".to_string()]
        } else {
            task.entrypoint.clone()
        };

        let mut create_cmd = Command::new("podman");
        create_cmd.arg("create");
        create_cmd.arg("-v").arg(format!("{}:/tork", workdir_for_child.display()));
        create_cmd.arg("--entrypoint").arg(&entrypoint[0]);

        // Set environment variables
        let env_vars: Vec<String> = task
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .chain([
                "TORK_OUTPUT=/tork/output".to_string(),
                "TORK_PROGRESS=/tork/progress".to_string(),
            ])
            .collect();

        for env in &env_vars {
            create_cmd.arg("-e").arg(env);
        }

        // Add networks
        for network in &task.networks {
            if network == HOST_NETWORK_NAME {
                if !self.host_network {
                    return Err(PodmanError::HostNetworkingDisabled);
                }
                create_cmd.arg("--network").arg(network);
            } else {
                let alias = slug::make(&task.name.clone().unwrap_or_default());
                create_cmd
                    .arg("--network")
                    .arg(network)
                    .arg("--network-alias")
                    .arg(alias);
            }
        }

        // Add mounts
        for mount in &task.mounts {
            match mount.mount_type {
                MountType::Volume | MountType::Bind => {
                    create_cmd
                        .arg("-v")
                        .arg(format!("{}:{}", mount.source, mount.target));
                }
                _ => {
                    return Err(PodmanError::UnknownMountType(mount.mount_type.to_string()));
                }
            }
        }

        // Set workdir
        let workdir_arg = if let Some(wd) = &task.workdir {
            wd.clone()
        } else if !task.files.is_empty() {
            DEFAULT_WORKDIR.to_string()
        } else {
            String::new()
        };

        if !workdir_arg.is_empty() {
            create_cmd.arg("-w").arg(&workdir_arg);
        }

        // Write task files
        if !task.files.is_empty() {
            let workdir_dir = workdir.join("workdir");
            tokio::fs::create_dir_all(&workdir_dir)
                .await
                .map_err(|e| PodmanError::FileWrite(e.to_string()))?;

            for (filename, contents) in &task.files {
                let file_path = workdir_dir.join(filename);
                tokio::fs::write(&file_path, contents)
                    .await
                    .map_err(|e| PodmanError::FileWrite(e.to_string()))?;
            }
        }

        if self.privileged {
            create_cmd.arg("--privileged");
        }

        create_cmd.arg(&task.image);
        for arg in entrypoint.iter().skip(1) {
            create_cmd.arg(arg);
        }
        create_cmd.arg("/tork/entrypoint.sh");

        create_cmd.stdout(Stdio::piped());
        create_cmd.stderr(Stdio::piped());

        let create_output = create_cmd
            .output()
            .await
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
            return Err(PodmanError::ContainerCreation("empty container ID".to_string()));
        }

        debug!("created container {}", container_id);

        // Store task -> container mapping
        self.tasks
            .write()
            .await
            .insert(task.id.clone(), container_id.clone());

        // Ensure container is removed on drop
        let tasks = Arc::clone(&self.tasks);
        let container_id_for_cleanup = container_id.clone();
        tokio::spawn(async move {
            let _ = Self::stop_container(&container_id_for_cleanup).await;
            tasks.write().await.remove(&container_id_for_cleanup);
        });

        // Start container
        let mut start_cmd = Command::new("podman");
        start_cmd.arg("start").arg(&container_id);
        start_cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerStart(e.to_string()))?;

        // Start background progress reporting
        let progress_task_id = task.id.clone();
        let progress_file_path = progress_file.clone();
        let broker = self.broker.clone();
        let progress_cancel = tokio::spawn(async move {
            Self::report_progress(&progress_task_id, progress_file_path, broker.as_ref()).await;
        });

        // Read logs with broker integration
        let logs_task_id = task.id.clone();
        let logs_broker = self.broker.clone();
        let mut logs_cmd = Command::new("podman");
        logs_cmd.arg("logs").arg("--follow").arg(&container_id);
        logs_cmd.stdout(Stdio::piped());
        logs_cmd.stderr(Stdio::piped());

        let mut child = logs_cmd
            .spawn()
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                debug!("[podman] {}", line);
                // Ship log to broker if available
                if let Some(ref broker) = logs_broker {
                    broker.ship_log(&logs_task_id, &line);
                }
            }
        }

        child
            .wait()
            .await
            .map_err(|e| PodmanError::LogsRead(e.to_string()))?;

        // Cancel progress reporting
        progress_cancel.abort();

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
            .map_err(|e| PodmanError::ContainerCreation(e.to_string()))?;

        let exit_code = String::from_utf8_lossy(&inspect_output.stdout).trim().to_string();
        if exit_code != "0" {
            return Err(PodmanError::ContainerExitCode(exit_code));
        }

        // Read output
        let output = tokio::fs::read_to_string(&output_file)
            .await
            .map_err(|e| PodmanError::OutputRead(e.to_string()))?;
        task.result = output;

        // Cleanup workdir
        if let Err(e) = std::fs::remove_dir_all(&workdir) {
            warn!("error removing workdir {:?}: {}", workdir, e);
        }

        Ok(())
    }

    async fn report_progress(
        task_id: &str,
        progress_file: std::path::PathBuf,
        broker: Option<&Box<dyn Broker + Send + Sync>>,
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

            if let Some(ref b) = broker {
                b.publish_task_progress(task_id, progress);
            }
        }
    }

    async fn stop_container(container_id: &str) -> Result<(), PodmanError> {
        let mut cmd = Command::new("podman");
        cmd.arg("rm").arg("-f").arg("-t").arg("0").arg(container_id);
        cmd.output()
            .await
            .map_err(|e| PodmanError::ContainerCreation(e.to_string()))?;
        Ok(())
    }

    async fn image_pull(&self, image: &str) -> Result<(), PodmanError> {
        // Check if image exists
        let images = self.images.read().await;
        if images.contains_key(image) {
            return Ok(());
        }
        drop(images);

        // Request pull
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pullq
            .send(PullRequest {
                ctx: tx,
                image: image.to_string(),
            })
            .await
            .map_err(|_| PodmanError::ImagePull("channel closed".to_string()))?;

        rx.await.map_err(|_| PodmanError::ImagePull("cancelled".to_string()))??;

        // Mark as pulled
        self.images.write().await.insert(image.to_string(), true);

        Ok(())
    }

    pub async fn health_check(&self) -> Result<(), PodmanError> {
        let mut cmd = Command::new("podman");
        cmd.arg("version");
        let output = cmd
            .output()
            .await
            .map_err(|e| PodmanError::ContainerCreation(e.to_string()))?;

        if !output.status.success() {
            return Err(PodmanError::ContainerCreation("podman not running".to_string()));
        }

        Ok(())
    }
}

// Task is a simplified representation of tork.Task
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub run: String,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub env: HashMap<String, String>,
    pub mounts: Vec<Mount>,
    pub files: HashMap<String, String>,
    pub networks: Vec<String>,
    pub limits: Option<TaskLimits>,
    pub registry: Option<String>,
    pub sidecars: Vec<Task>,
    pub pre: Vec<Task>,
    pub post: Vec<Task>,
    pub workdir: Option<String>,
    pub result: String,
    pub progress: f64,
}

#[derive(Debug, Clone)]
pub struct Mount {
    pub id: String,
    pub mount_type: MountType,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MountType {
    Volume,
    Bind,
}

impl std::fmt::Display for MountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountType::Volume => write!(f, "Volume"),
            MountType::Bind => write!(f, "Bind"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskLimits {
    pub cpus: String,
    pub memory: String,
}

// Minimal slug implementation
mod slug {
    pub fn make(s: &str) -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>()
    }
}
