//! Shell runtime module

mod reexec;
#[cfg(unix)]
mod setid_unix;
#[cfg(not(unix))]
mod setid_unsupported;

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use itertools::Itertools;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{Sender, channel};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::broker::Broker;
use crate::runtime::shell::reexec::ReexecCommand;
use tork::task::TaskLogPart;

pub const DEFAULT_UID: &str = "-";
pub const DEFAULT_GID: &str = "-";
const ENV_VAR_PREFIX: &str = "REEXEC_";

#[derive(Error, Debug)]
pub enum ShellError {
    #[error("task id is required")]
    TaskIdRequired,

    #[error("mounts are not supported on shell runtime")]
    MountsNotSupported,

    #[error("entrypoint is not supported on shell runtime")]
    EntrypointNotSupported,

    #[error("image is not supported on shell runtime")]
    ImageNotSupported,

    #[error("limits are not supported on shell runtime")]
    LimitsNotSupported,

    #[error("networks are not supported on shell runtime")]
    NetworksNotSupported,

    #[error("registry is not supported on shell runtime")]
    RegistryNotSupported,

    #[error("cmd is not supported on shell runtime")]
    CmdNotSupported,

    #[error("sidecars are not supported on shell runtime")]
    SidecarsNotSupported,

    #[error("failed to create workdir: {0}")]
    WorkdirCreation(String),

    #[error("failed to write file: {0}")]
    FileWrite(String),

    #[error("failed to read output: {0}")]
    OutputRead(String),

    #[error("failed to read progress: {0}")]
    ProgressRead(String),

    #[error("command execution failed: {0}")]
    CommandFailed(String),

    #[error("context cancelled")]
    ContextCancelled,
}

pub struct ShellConfig {
    pub cmd: Vec<String>,
    pub uid: String,
    pub gid: String,
    pub reexec: Option<ReexecCommand>,
    pub broker: Option<Arc<dyn Broker + Send + Sync>>,
}

impl std::fmt::Debug for ShellConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellConfig")
            .field("cmd", &self.cmd)
            .field("uid", &self.uid)
            .field("gid", &self.gid)
            .field("reexec", &"<fn>")
            .finish()
    }
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            cmd: vec!["bash".to_string(), "-c".to_string()],
            uid: DEFAULT_UID.to_string(),
            gid: DEFAULT_GID.to_string(),
            reexec: None,
            broker: None,
        }
    }
}

pub struct ShellRuntime {
    cmds: RwLock<HashMap<String, u32>>,
    shell: Vec<String>,
    uid: String,
    gid: String,
    reexec: ReexecCommand,
    broker: Option<Arc<dyn Broker + Send + Sync>>,
}

impl std::fmt::Debug for ShellRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellRuntime")
            .field("shell", &self.shell)
            .field("uid", &self.uid)
            .field("gid", &self.gid)
            .field("reexec", &"<fn>")
            .field("broker", &"<broker>")
            .finish()
    }
}

impl ShellRuntime {
    pub fn new(config: ShellConfig) -> Self {
        Self {
            cmds: RwLock::new(HashMap::new()),
            shell: config.cmd,
            uid: config.uid,
            gid: config.gid,
            reexec: config.reexec.unwrap_or_else(|| Box::new(reexec_default)),
            broker: config.broker,
        }
    }

    pub async fn run(&self, cancel: Arc<AtomicBool>, task: &mut Task) -> Result<(), ShellError> {
        // Validate task
        if task.id.is_empty() {
            return Err(ShellError::TaskIdRequired);
        }
        if !task.mounts.is_empty() {
            return Err(ShellError::MountsNotSupported);
        }
        if !task.entrypoint.is_empty() {
            return Err(ShellError::EntrypointNotSupported);
        }
        if !task.image.is_empty() {
            return Err(ShellError::ImageNotSupported);
        }
        if let Some(limits) = &task.limits {
            if !limits.cpus.is_empty() || !limits.memory.is_empty() {
                return Err(ShellError::LimitsNotSupported);
            }
        }
        if !task.networks.is_empty() {
            return Err(ShellError::NetworksNotSupported);
        }
        if task.registry.is_some() {
            return Err(ShellError::RegistryNotSupported);
        }
        if !task.cmd.is_empty() {
            return Err(ShellError::CmdNotSupported);
        }
        if !task.sidecars.is_empty() {
            return Err(ShellError::SidecarsNotSupported);
        }

        // Clone Arc for each task - Go code shares context across all tasks
        let cancel_main = cancel.clone();

        // Execute pre-tasks
        for pre in task.pre.iter_mut() {
            pre.id = uuid::Uuid::new_v4().to_string();
            self.do_run(cancel.clone(), pre).await?;
        }

        // Run the actual task
        self.do_run(cancel_main, task).await?;

        // Execute post tasks
        for post in task.post.iter_mut() {
            post.id = uuid::Uuid::new_v4().to_string();
            self.do_run(cancel.clone(), post).await?;
        }

        Ok(())
    }

    async fn do_run(&self, cancel: Arc<AtomicBool>, task: &mut Task) -> Result<(), ShellError> {
        let task_id = task.id.clone();
        let workdir = tempfile::tempdir()
            .map_err(|e| ShellError::WorkdirCreation(e.to_string()))?
            .keep();

        debug!("Created workdir {:?}", workdir);

        // Create stdout file
        let stdout_path = workdir.join("stdout");
        std::fs::write(&stdout_path, b"").map_err(|e| ShellError::FileWrite(e.to_string()))?;

        // Create progress file
        let progress_path = workdir.join("progress");
        std::fs::write(&progress_path, b"").map_err(|e| ShellError::FileWrite(e.to_string()))?;

        // Write task files
        for (filename, contents) in &task.files {
            let file_path = workdir.join(filename);
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ShellError::FileWrite(e.to_string()))?;
            }
            std::fs::write(&file_path, contents)
                .map_err(|e| ShellError::FileWrite(e.to_string()))?;
        }

        // Build environment variables
        let env_vars: Vec<(String, String)> = std::env::vars()
            .filter(|(k, _)| !k.starts_with(ENV_VAR_PREFIX))
            .chain(
                task.env
                    .iter()
                    .map(|(k, v)| (format!("{}{}", ENV_VAR_PREFIX, k), v.clone())),
            )
            .chain([
                (
                    format!("{}TORK_OUTPUT", ENV_VAR_PREFIX),
                    stdout_path.to_string_lossy().to_string(),
                ),
                (
                    format!("{}TORK_PROGRESS", ENV_VAR_PREFIX),
                    progress_path.to_string_lossy().to_string(),
                ),
                ("WORKDIR".to_string(), workdir.to_string_lossy().to_string()),
                ("PATH".to_string(), std::env::var("PATH").unwrap_or_default()),
                ("HOME".to_string(), std::env::var("HOME").unwrap_or_default()),
            ])
            .collect();

        // Write entrypoint
        let entrypoint_path = workdir.join("entrypoint");
        std::fs::write(&entrypoint_path, &task.r#run)
            .map_err(|e| ShellError::FileWrite(e.to_string()))?;

        // Build command arguments
        let shell_cmd = self.shell.join(" ");
        let entrypoint_str = entrypoint_path.to_string_lossy();
        let args: Vec<String> = vec![
            "shell".to_string(),
            "-uid".to_string(),
            self.uid.clone(),
            "-gid".to_string(),
            shell_cmd,
            entrypoint_str.to_string(),
        ];

        // Execute command
        let mut cmd = (self.reexec)(&args);
        for (k, v) in &env_vars {
            cmd.env(k, v);
        }
        cmd.current_dir(&workdir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| ShellError::CommandFailed(e.to_string()))?;

        let pid = child.id().unwrap_or(0);
        self.cmds.write().await.insert(task_id.clone(), pid);

        // Create log shipper if broker is available
        let log_tx: Option<Sender<Vec<u8>>> = self.broker.as_ref().map(|b| {
            let (tx, rx) = channel(1000);
            let broker = b.clone();
            let task_id_clone = task_id.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                let mut part_num = 0i64;
                let mut buffer = Vec::new();
                loop {
                    tokio::select! {
                        Some(data) = rx.recv() => {
                            buffer.extend_from_slice(&data);
                        }
                        _ = interval.tick() => {
                            if !buffer.is_empty() {
                                part_num += 1;
                                let contents = String::from_utf8(buffer.clone())
                                    .unwrap_or_default();
                                let part = TaskLogPart {
                                    id: None,
                                    number: part_num,
                                    task_id: Some(task_id_clone.clone()),
                                    contents: Some(contents),
                                    created_at: None,
                                };
                                let _ = broker.publish_task_log_part(&part).await;
                                buffer.clear();
                            }
                        }
                    }
                }
            });
            tx
        });

        // Spawn stdout reader with log shipping
        let stdout_handle = if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let log_tx_clone = log_tx.clone();
            Some(tokio::spawn(async move {
                let mut reader = reader;
                let mut line = String::new();
                loop {
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let trimmed = line.trim_end();
                            debug!("[shell] {}", trimmed);
                            // Ship to broker if available
                            if let Some(ref tx) = log_tx_clone {
                                let _ = tx.send(format!("{}\n", trimmed).into_bytes()).await;
                            }
                            line.clear();
                        }
                        Err(e) => {
                            if e.to_string().contains("closed") || e.to_string().contains("EOF") {
                                break;
                            }
                            warn!("error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            }))
        } else {
            None
        };

        // Spawn progress tracking task
        let progress_task = {
            let broker = self.broker.clone();
            let task_id_clone = task_id.clone();
            let progress_path = progress_path.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            if let Some(broker) = &broker {
                                match Self::read_progress_sync(&progress_path) {
                                    Ok(progress) => {
                                        let part = TaskLogPart {
                                            id: None,
                                            number: 0,
                                            task_id: Some(task_id_clone.clone()),
                                            contents: Some(format!("progress:{}", progress)),
                                            created_at: None,
                                        };
                                        let _ = broker.publish_task_log_part(&part).await;
                                    }
                                    Err(e) => {
                                        if e.to_string().contains("NotFound") || e.to_string().contains("os error 2") {
                                            return; // progress file removed
                                        }
                                        warn!("error reading progress: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            })
        };

        // Wait for command completion with cancellation support
        // Spawn a task to watch for cancellation
        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::channel::<()>(1);
        let cancel_task = {
            let cancel_flag = cancel.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if cancel_flag.load(Ordering::SeqCst) {
                        let _ = cancel_tx.send(()).await;
                        break;
                    }
                }
            })
        };

        let status = tokio::select! {
            result = child.wait() => {
                cancel_task.abort();
                result.map_err(|e| ShellError::CommandFailed(e.to_string()))
            }
            _ = cancel_rx.recv() => {
                let _ = child.kill().await;
                return Err(ShellError::ContextCancelled);
            }
        };

        // Clean up background tasks
        if let Some(handle) = stdout_handle {
            let _ = handle.await;
        }
        progress_task.abort();

        let status = status?;
        self.cmds.write().await.remove(&task_id);

        if !status.success() {
            return Err(ShellError::CommandFailed(format!(
                "exit code: {:?}",
                status.code()
            )));
        }

        // Read output
        let output =
            std::fs::read_to_string(&stdout_path).map_err(|e| ShellError::OutputRead(e.to_string()))?;
        task.result = output;

        // Cleanup workdir
        if let Err(e) = std::fs::remove_dir_all(&workdir) {
            warn!("error removing workdir {:?}: {}", workdir, e);
        }

        Ok(())
    }

    fn read_progress_sync(progress_path: &std::path::Path) -> Result<f64, ShellError> {
        let contents = std::fs::read_to_string(progress_path)
            .map_err(|e| ShellError::ProgressRead(e.to_string()))?;
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            return Ok(0.0);
        }
        trimmed.parse::<f64>()
            .map_err(|e| ShellError::ProgressRead(e.to_string()))
    }

    async fn read_progress(progress_path: &std::path::Path) -> Result<f64, ShellError> {
        let contents = std::fs::read_to_string(progress_path)
            .map_err(|e| ShellError::ProgressRead(e.to_string()))?;
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            return Ok(0.0);
        }
        trimmed.parse::<f64>()
            .map_err(|e| ShellError::ProgressRead(e.to_string()))
    }

    pub async fn health_check(&self) -> Result<(), ShellError> {
        // Shell runtime is always healthy
        Ok(())
    }
}

fn reexec_default(args: &[String]) -> Command {
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    #[cfg(unix)]
    {
        if args[2] != DEFAULT_UID {
            if let Ok(uid) = args[2].parse::<u32>() {
                cmd.uid(uid);
            }
        }
        if args[4] != DEFAULT_GID {
            if let Ok(gid) = args[4].parse::<u32>() {
                cmd.gid(gid);
            }
        }
    }
    cmd
}

pub fn build_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| k.starts_with(ENV_VAR_PREFIX))
        .map(|(k, v)| {
            let key = k.trim_start_matches(ENV_VAR_PREFIX).to_string();
            (key, v)
        })
        .collect()
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

#[derive(Debug, Clone)]
pub struct TaskLimits {
    pub cpus: String,
    pub memory: String,
}
