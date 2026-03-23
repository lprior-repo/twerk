//! Shell runtime module — executes tasks via a local shell command.
//!
//! # Architecture (Data → Calc → Actions)
//!
//! - **Data**: `Task`, `ShellConfig`, `ShellError`, file permissions constants
//! - **Calc**: validation logic, env building (`build_env`), progress parsing
//! - **Actions**: `do_run` — file I/O, process spawning, broker publishing
//!
//! # Go Parity Notes
//!
//! - File permissions match Go: entrypoint=0o555, task files=0o444, stdout/progress=0o606
//! - Env is replaced (not inherited) matching Go's `cmd.Env = env`
//! - Progress publishes via `Broker::publish_task_progress` (Go: `PublishTaskProgress`)
//! - Args layout: `["shell", "-uid", UID, "-gid", GID, <shell...>, <entrypoint>]`

mod reexec;
#[cfg(unix)]
mod setid_unix;
#[cfg(not(unix))]
mod setid_unsupported;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::broker::Broker;
use crate::runtime::shell::reexec::ReexecCommand;
use tork::task::{Task as TorkTask, TaskLogPart};

pub const DEFAULT_UID: &str = "-";
pub const DEFAULT_GID: &str = "-";
const ENV_VAR_PREFIX: &str = "REEXEC_";

/// File permission modes matching Go's os.WriteFile modes.
#[cfg(unix)]
mod perms {
    use std::os::unix::fs::PermissionsExt;

    pub const ENTRYPOINT: u32 = 0o555;
    pub const TASK_FILE: u32 = 0o444;
    pub const STDOUT: u32 = 0o606;
    pub const PROGRESS: u32 = 0o606;

    pub fn set(path: &std::path::Path, mode: u32) -> Result<(), String> {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .map_err(|e| format!("failed to set permissions on {}: {}", path.display(), e))
    }
}

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
        // Validate task (pure calculation — no I/O)
        validate_task(task)?;

        // Clone Arc for each task — Go code shares context across all tasks
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

        // === ACTION: Create temp workdir ===
        let workdir = tempfile::tempdir()
            .map_err(|e| ShellError::WorkdirCreation(e.to_string()))?
            .keep();

        debug!("Created workdir {:?}", workdir);

        // === ACTION: Create stdout and progress files with correct permissions ===
        let stdout_path = workdir.join("stdout");
        std::fs::write(&stdout_path, b"")
            .map_err(|e| ShellError::FileWrite(e.to_string()))?;
        #[cfg(unix)]
        perms::set(&stdout_path, perms::STDOUT).map_err(ShellError::FileWrite)?;

        let progress_path = workdir.join("progress");
        std::fs::write(&progress_path, b"")
            .map_err(|e| ShellError::FileWrite(e.to_string()))?;
        #[cfg(unix)]
        perms::set(&progress_path, perms::PROGRESS)
            .map_err(ShellError::FileWrite)?;

        // === ACTION: Write task files with 0444 permissions ===
        for (filename, contents) in &task.files {
            let file_path = workdir.join(filename);
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ShellError::FileWrite(e.to_string()))?;
            }
            std::fs::write(&file_path, contents)
                .map_err(|e| ShellError::FileWrite(e.to_string()))?;
            #[cfg(unix)]
            perms::set(&file_path, perms::TASK_FILE)
                .map_err(ShellError::FileWrite)?;
        }

        // === CALC: Build environment (matching Go exactly) ===
        // Go only passes: REEXEC_{task_env}, REEXEC_TORK_OUTPUT, REEXEC_TORK_PROGRESS,
        // WORKDIR, PATH, HOME — then replaces cmd.Env entirely.
        let env_vars = build_task_env(&task.env, &stdout_path, &progress_path, &workdir);

        // === ACTION: Write entrypoint with 0555 permissions ===
        let entrypoint_path = workdir.join("entrypoint");
        std::fs::write(&entrypoint_path, &task.r#run)
            .map_err(|e| ShellError::FileWrite(e.to_string()))?;
        #[cfg(unix)]
        perms::set(&entrypoint_path, perms::ENTRYPOINT)
            .map_err(ShellError::FileWrite)?;

        // === CALC: Build command arguments (matching Go layout exactly) ===
        // Go: ["shell", "-uid", UID, "-gid", GID, <shell...>, <entrypoint>]
        // Args layout: [0]="shell" [1]="-uid" [2]=UID [3]="-gid" [4]=GID [5..]=shell [N]=entrypoint
        let mut args: Vec<String> = vec![
            "shell".to_string(),
            "-uid".to_string(),
            self.uid.clone(),
            "-gid".to_string(),
            self.gid.clone(),
        ];
        args.extend_from_slice(&self.shell);
        args.push(entrypoint_path.to_string_lossy().into_owned());

        // === ACTION: Build and spawn command ===
        let mut cmd = (self.reexec)(&args);
        // Go: cmd.Env = env (replaces entire environment, not additive).
        // In Rust, we set env vars individually. The PATH var is explicitly
        // included in env_vars, so the child process can find binaries.
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

        // === ACTION: Set up log shipping via broker ===
        let log_tx: Option<Sender<Vec<u8>>> = self.broker.as_ref().map(|b| {
            let (tx, mut rx): (Sender<Vec<u8>>, _) = channel(1000);
            let broker = b.clone();
            let tid = task_id.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                let mut part_num = 0i64;
                let mut buffer: Vec<u8> = Vec::new();
                loop {
                    tokio::select! {
                        Some(data) = rx.recv() => {
                            buffer.extend_from_slice(&data);
                        }
                        _ = interval.tick() => {
                            if !buffer.is_empty() {
                                part_num += 1;
                                let contents = String::from_utf8_lossy(&buffer).to_string();
                                let part = TaskLogPart {
                                    id: None,
                                    number: part_num,
                                    task_id: Some(tid.clone()),
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

        // === ACTION: Spawn stdout reader ===
        let stdout_handle = child.stdout.take().map(|stdout| {
            let log_tx_clone = log_tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim_end();
                    debug!("[shell] {}", trimmed);
                    if let Some(ref tx) = log_tx_clone {
                        let _ = tx.send(format!("{}\n", trimmed).into_bytes()).await;
                    }
                }
            })
        });

        // === ACTION: Spawn progress tracker (publishes to broker) ===
        let progress_task = spawn_progress_tracker(
            self.broker.clone(),
            task_id.clone(),
            progress_path.clone(),
        );

        // === ACTION: Wait for completion with cancellation support ===
        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::channel::<()>(1);
        let cancel_watcher = {
            let flag = cancel.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if flag.load(Ordering::SeqCst) {
                        let _ = cancel_tx.send(()).await;
                        break;
                    }
                }
            })
        };

        let status = tokio::select! {
            result = child.wait() => {
                cancel_watcher.abort();
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

        // === ACTION: Read task output ===
        let output = std::fs::read_to_string(&stdout_path)
            .map_err(|e| ShellError::OutputRead(e.to_string()))?;
        task.result = output;

        // === ACTION: Cleanup workdir (matching Go's defer os.RemoveAll) ===
        if let Err(e) = std::fs::remove_dir_all(&workdir) {
            warn!("error removing workdir {:?}: {}", workdir, e);
        }

        Ok(())
    }

    pub async fn health_check(&self) -> Result<(), ShellError> {
        Ok(())
    }
}

// ── Pure calculations (no I/O) ─────────────────────────────────────────

/// Validates that the task is compatible with the shell runtime.
/// Returns an error for any unsupported feature (matching Go's checks exactly).
fn validate_task(task: &Task) -> Result<(), ShellError> {
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
    Ok(())
}

/// Builds the environment variables for the child process.
///
/// Matches Go's env building exactly:
/// - Task env vars prefixed with `REEXEC_`
/// - `REEXEC_TORK_OUTPUT` pointing to stdout file
/// - `REEXEC_TORK_PROGRESS` pointing to progress file
/// - `WORKDIR`, `PATH`, `HOME`
fn build_task_env(
    task_env: &HashMap<String, String>,
    stdout_path: &std::path::Path,
    progress_path: &std::path::Path,
    workdir: &std::path::Path,
) -> Vec<(String, String)> {
    task_env
        .iter()
        .map(|(k, v)| (format!("{}{}", ENV_VAR_PREFIX, k), v.clone()))
        .chain([
            (
                format!("{}TORK_OUTPUT", ENV_VAR_PREFIX),
                stdout_path.to_string_lossy().to_string(),
            ),
            (
                format!("{}TORK_PROGRESS", ENV_VAR_PREFIX),
                progress_path.to_string_lossy().to_string(),
            ),
            (
                "WORKDIR".to_string(),
                workdir.to_string_lossy().to_string(),
            ),
            (
                "PATH".to_string(),
                std::env::var("PATH").unwrap_or_default(),
            ),
            (
                "HOME".to_string(),
                std::env::var("HOME").unwrap_or_default(),
            ),
        ])
        .collect()
}

/// Spawns a background task that reads the progress file every 10 seconds
/// and publishes changes via `Broker::publish_task_progress`.
///
/// Matches Go's progress goroutine which reads the file, compares with
/// previous value, updates `t.Progress`, and calls `PublishTaskProgress`.
fn spawn_progress_tracker(
    broker: Option<Arc<dyn Broker + Send + Sync>>,
    task_id: String,
    progress_path: std::path::PathBuf,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_progress = 0.0f64;
        let interval = tokio::time::interval(std::time::Duration::from_secs(10));
        tokio::pin!(interval);

        loop {
            interval.tick().await;

            let broker = match &broker {
                Some(b) => b,
                None => return,
            };

            match read_progress_sync(&progress_path) {
                Ok(progress) => {
                    if (progress - last_progress).abs() > f64::EPSILON {
                        last_progress = progress;
                        let tork_task = TorkTask {
                            id: Some(task_id.clone()),
                            progress,
                            ..Default::default()
                        };
                        if let Err(e) = broker.publish_task_progress(&tork_task).await {
                            warn!("error publishing task progress: {}", e);
                        }
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("NotFound") || err_str.contains("os error 2") {
                        return; // progress file removed
                    }
                    warn!("error reading progress: {}", e);
                }
            }
        }
    })
}

/// Reads and parses the progress file (pure file I/O, no async needed).
fn read_progress_sync(progress_path: &std::path::Path) -> Result<f64, ShellError> {
    let contents = std::fs::read_to_string(progress_path)
        .map_err(|e| ShellError::ProgressRead(e.to_string()))?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return Ok(0.0);
    }
    trimmed
        .parse::<f64>()
        .map_err(|e| ShellError::ProgressRead(e.to_string()))
}

// ── Default reexec function ────────────────────────────────────────────

/// Default reexec function for shell runtime.
///
/// Uses the reexec pattern: spawns `/proc/self/exe` with argv[0]="shell"
/// and all arguments. The child process detects "shell" mode via
/// `reexec::init()` and runs the shell initializer.
///
/// The args layout is: ["shell", "-uid", UID, "-gid", GID, <shell>, <shell_args...>]
/// which matches Go's reexec.Command("shell", "-uid", uid, "-gid", gid, shell, shell_args...)
fn reexec_default(args: &[String]) -> Command {
    // Use the reexec pattern: spawn /proc/self/exe with argv[0]="shell"
    // The child process will detect shell mode and run appropriately
    crate::runtime::shell::reexec::reexec_from_std(args)
}

// ── Public helper: build_env ───────────────────────────────────────────

/// Strips the `REEXEC_` prefix from environment variables.
///
/// Matches Go's `buildEnv()`: iterates `os.Environ()`, filters for
/// `REEXEC_` prefix, strips it, and returns `[(KEY, VALUE)]` pairs.
pub fn build_env() -> Vec<(String, String)> {
    std::env::vars()
        .filter(|(k, _)| k.starts_with(ENV_VAR_PREFIX))
        .map(|(k, v)| {
            let key = k.trim_start_matches(ENV_VAR_PREFIX).to_string();
            (key, v)
        })
        .collect()
}

// ── Task type (shell-local representation) ─────────────────────────────

/// Simplified Task representation used internally by the shell runtime.
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
