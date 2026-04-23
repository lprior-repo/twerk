use dashmap::DashMap;
use std::process::{ExitCode, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::interval;
use twerk_core::env::{read_cleanup_env, read_timeout_env};
use twerk_core::task::{is_task_state_active, Task, TaskLogPart};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::docker::config::ERROR_PUBLISHING_TASK_PROGRESS;
use twerk_infrastructure::runtime::{
    BoxedFuture, Runtime as RuntimeTrait, ShutdownError, ShutdownResult,
};

use tracing::instrument;

// ── Typed error for shell runtime ──────────────────────────────────

#[derive(Debug, thiserror::Error)]
enum ShellError {
    #[error("id and run script required")]
    IdAndRunScriptRequired,
    #[error("shell command required")]
    ShellCommandRequired,
    #[error("child process spawned but PID unavailable")]
    PidUnavailable,
    #[error("process exited with code {0}")]
    ExitFailed(String),
}

// Module-level function to avoid lifetime issues with associated functions
async fn terminate_process(
    pid: u32,
    graceful_timeout: u64,
    _force_timeout: u64,
) -> ShutdownResult<ExitCode> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    if pid == 0 {
        return Err(ShutdownError::ProcessNotFound(format!(
            "invalid pid: {}",
            pid
        )));
    }

    let pid = Pid::from_raw(pid as i32);

    // Send SIGTERM (graceful termination)
    if let Err(e) = signal::kill(pid, Signal::SIGTERM) {
        // If process doesn't exist, it's already terminated
        if e == nix::errno::Errno::ESRCH {
            return Ok(ExitCode::SUCCESS);
        }
        return Err(ShutdownError::SignalError(format!(
            "failed to send SIGTERM: {}",
            e
        )));
    }

    // Wait for graceful termination with timeout
    let graceful_duration = Duration::from_secs(graceful_timeout);
    let wait_future = async {
        tokio::process::Command::new("wait")
            .arg(pid.to_string())
            .output()
            .await
    };

    match tokio::time::timeout(graceful_duration, wait_future).await {
        Ok(Ok(out)) => {
            if let Some(code) = out.status.code() {
                return Ok(ExitCode::from(code as u8));
            }
            Ok(ExitCode::SUCCESS)
        }
        Ok(Err(_)) => {
            // Wait command failed
            Ok(ExitCode::from(137))
        }
        Err(_) => {
            // Timeout exceeded, force kill
            if let Err(e) = signal::kill(pid, Signal::SIGKILL) {
                return Err(ShutdownError::TerminationFailed(format!(
                    "failed to send SIGKILL: {}",
                    e
                )));
            }
            Ok(ExitCode::from(137))
        }
    }
}

// Module-level function to avoid lifetime issues with associated functions
async fn cleanup_temp_dir(
    temp_dirs: &DashMap<String, String>,
    task_id: &str,
) -> ShutdownResult<()> {
    if let Some((_, path)) = temp_dirs.remove(task_id) {
        match tokio::fs::remove_dir_all(&path).await {
            Ok(()) => Ok(()),
            Err(e) => Err(ShutdownError::CleanupFailed(format!(
                "failed to remove temp dir {}: {}",
                path, e
            ))),
        }
    } else {
        Ok(())
    }
}

async fn publish_log_parts(
    broker: Option<&Arc<dyn Broker>>,
    task_id: &twerk_core::id::TaskId,
    stdout: &[u8],
    stderr: &[u8],
) {
    let Some(broker) = broker else {
        return;
    };

    for (index, contents) in [stdout, stderr]
        .into_iter()
        .map(|bytes| String::from_utf8_lossy(bytes).to_string())
        .filter(|contents| !contents.trim().is_empty())
        .enumerate()
    {
        let number = i64::try_from(index + 1).map_or(i64::MAX, std::convert::identity);
        let part = TaskLogPart {
            id: None,
            number,
            task_id: Some(task_id.clone()),
            contents: Some(contents),
            created_at: None,
        };
        if let Err(error) = broker.publish_task_log_part(&part).await {
            tracing::warn!(task_id = %task_id, %error, "error publishing task log part");
        }
    }
}

// Process handle to track running shell processes
#[derive(Debug, Clone)]
pub struct ProcessHandle {
    pub pid: u32,
}

// Config for shell runtime adapter
#[derive(Debug, Clone)]
pub struct ShellRuntimeConfig {
    pub cmd: Vec<String>,
    pub uid: String,
    pub gid: String,
    pub graceful_timeout: u64,
    pub force_timeout: u64,
    pub enable_cleanup: bool,
}

impl Default for ShellRuntimeConfig {
    fn default() -> Self {
        Self {
            cmd: vec!["bash".to_string(), "-c".to_string()],
            uid: "-".to_string(),
            gid: "-".to_string(),
            graceful_timeout: 30,
            force_timeout: 5,
            enable_cleanup: true,
        }
    }
}

pub struct ShellRuntimeAdapter {
    config: ShellRuntimeConfig,
    active_processes: Arc<DashMap<String, ProcessHandle>>,
    temp_dirs: Arc<DashMap<String, String>>,
    broker: Option<Arc<dyn Broker>>,
}

impl ShellRuntimeAdapter {
    #[must_use]
    pub fn new(
        cmd: Vec<String>,
        uid: String,
        gid: String,
        broker: Option<Arc<dyn Broker>>,
    ) -> Self {
        let default_cmd = ShellRuntimeConfig::default().cmd;
        let resolved_cmd = if cmd.is_empty() { default_cmd } else { cmd };
        Self {
            config: ShellRuntimeConfig {
                cmd: resolved_cmd,
                uid,
                gid,
                graceful_timeout: read_timeout_env("TASK_STOP_GRACEFUL_TIMEOUT", 30),
                force_timeout: read_timeout_env("TASK_STOP_FORCE_TIMEOUT", 5),
                enable_cleanup: read_cleanup_env("TASK_STOP_ENABLE_CLEANUP", true),
            },
            active_processes: Arc::new(DashMap::new()),
            temp_dirs: Arc::new(DashMap::new()),
            broker,
        }
    }

    #[allow(dead_code)]
    fn validate_task(task: &Task) -> ShutdownResult<()> {
        // Check for empty task ID (precondition)
        if task.id.as_ref().is_none_or(|id| id.is_empty()) {
            return Err(ShutdownError::InvalidTaskId(
                task.id
                    .clone()
                    .map_or_else(String::new, |id| id.to_string()),
            ));
        }

        // Check if task is in active state (precondition)
        if !is_task_state_active(task.state) {
            return Err(ShutdownError::TaskNotRunning(task.state.to_string()));
        }

        Ok(())
    }
}

impl RuntimeTrait for ShellRuntimeAdapter {
    #[instrument(name = "shell_run", skip_all)]
    fn run(&self, task: &Task) -> BoxedFuture<()> {
        let (sc, tid, rs, env, active_processes, temp_dirs, broker, enable_cleanup) = (
            self.config.cmd.clone(),
            task.id
                .clone()
                .map_or_else(String::new, |id| id.to_string()),
            task.run.clone().unwrap_or_default(),
            task.env.clone(),
            self.active_processes.clone(),
            self.temp_dirs.clone(),
            self.broker.clone(),
            self.config.enable_cleanup,
        );

        Box::pin(async move {
            if tid.is_empty() || rs.is_empty() {
                return Err(ShellError::IdAndRunScriptRequired.into());
            }

            // Create temp directory for script
            let td = tempfile::tempdir()?;
            let sp = td.path().join("script.sh");
            let progress_path = td.path().join("progress");

            // Create empty progress file
            tokio::fs::write(&progress_path, "").await?;

            // Write script content
            tokio::fs::write(&sp, format!("#!/bin/bash\n{}", rs)).await?;

            // Make script executable (Unix only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(&sp, std::fs::Permissions::from_mode(0o755)).await?;
            }

            // Store temp dir path for cleanup
            let td_path = td.path().to_string_lossy().to_string();
            temp_dirs.insert(tid.to_string(), td_path.clone());

            // Spawn process
            let (program, base_args) = sc.split_first().ok_or(ShellError::ShellCommandRequired)?;
            let mut cmd = Command::new(program);
            cmd.args(base_args)
                .arg(sp.to_string_lossy().as_ref())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // `TWERK_PROGRESS` is the canonical env var; keep `TORK_PROGRESS`
            // for older scripts that still rely on the historical name.
            cmd.env("TWERK_PROGRESS", progress_path.to_string_lossy().as_ref());
            cmd.env("TORK_PROGRESS", progress_path.to_string_lossy().as_ref());

            if let Some(ref e) = env {
                for (k, v) in e {
                    cmd.env(k, v);
                }
            }

            // Track the child handle for potential future use
            let child = cmd.spawn()?;

            let pid = child.id().ok_or(ShellError::PidUnavailable)?;
            let handle = ProcessHandle { pid };

            // Store handle for stop() to use
            active_processes.insert(tid.to_string(), handle);

            // Spawn background task for progress monitoring
            let progress_path_clone = progress_path.clone();
            let task_id_clone = tid.clone();
            let broker_clone = broker.clone();
            tokio::spawn(async move {
                let mut tick = interval(Duration::from_secs(10));
                let mut prev: Option<f64> = None;
                loop {
                    tokio::select! {
                        _ = tick.tick() => {
                            match tokio::fs::read_to_string(&progress_path_clone).await {
                                Ok(contents) => {
                                    let s = contents.trim();
                                    if s.is_empty() {
                                        continue;
                                    }
                                    match s.parse::<f64>() {
                                        Ok(p) if prev.is_none_or(|old| (old - p).abs() > 0.001) => {
                                            prev = Some(p);
                                            if let Some(ref b) = broker_clone {
                                                let twerk_task = twerk_core::task::Task {
                                                     id: Some(task_id_clone.clone().into()),
                                                    progress: p,
                                                    ..Default::default()
                                                };
                                                if let Err(e) = b.publish_task_progress(&twerk_task).await {
                                                    tracing::warn!(task_id = %task_id_clone, error = %e, ERROR_PUBLISHING_TASK_PROGRESS);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!(task_id = %task_id_clone, error = %e, "error parsing progress value");
                                        }
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    if e.kind() == std::io::ErrorKind::NotFound {
                                        return;
                                    }
                                    tracing::warn!(task_id = %task_id_clone, error = %e, "error reading progress file");
                                }
                            }
                        }
                    }
                }
            });

            // Wait for completion and publish captured output as task logs.
            let wait_result = child.wait_with_output().await;
            active_processes.remove(tid.as_str());
            if enable_cleanup {
                let _ = cleanup_temp_dir(&temp_dirs, tid.as_str()).await;
            }

            let output = wait_result?;
            publish_log_parts(broker.as_ref(), &tid.into(), &output.stdout, &output.stderr).await;

            if !output.status.success() {
                return Err(ShellError::ExitFailed(
                    output
                        .status
                        .code()
                        .map_or_else(|| "unknown (signal)".to_string(), |c| c.to_string()),
                )
                .into());
            }

            Ok(())
        })
    }

    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>> {
        let (task_id_str, task_state, config, active_processes, temp_dirs) = (
            task.id
                .clone()
                .map_or_else(String::new, |id| id.to_string()),
            task.state,
            self.config.clone(),
            self.active_processes.clone(),
            self.temp_dirs.clone(),
        );

        Box::pin(async move {
            // Validate task first (precondition check)
            if task_id_str.is_empty() {
                return Ok(Err(ShutdownError::InvalidTaskId(task_id_str.clone())));
            }
            if !is_task_state_active(task_state) {
                return Ok(Err(ShutdownError::TaskNotRunning(task_state.to_string())));
            }

            // Check if process is tracked (precondition check)
            let handle = match active_processes.get(task_id_str.as_str()) {
                Some(h) => h.clone(),
                None => {
                    // Process not tracked - could be already stopped or never started
                    // Return Ok with exit code 0 for idempotency
                    return Ok(Ok(ExitCode::SUCCESS));
                }
            };

            // Terminate the process
            let exit_code =
                terminate_process(handle.pid, config.graceful_timeout, config.force_timeout)
                    .await?;

            // Remove from active processes map
            active_processes.remove(task_id_str.as_str());

            // Cleanup temp files (postcondition)
            if config.enable_cleanup {
                let _ = cleanup_temp_dir(&temp_dirs, task_id_str.as_str()).await;
            }

            Ok(Ok(exit_code))
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use twerk_core::task::{Task, TaskState};
    use twerk_infrastructure::broker::inmemory::InMemoryBroker;

    fn create_test_task(id: &str, state: TaskState) -> Task {
        Task {
            id: Some(id.to_string().into()),
            state,
            ..Default::default()
        }
    }

    #[test]
    #[should_panic(expected = "ID cannot be empty")]
    fn validate_task_returns_error_when_id_is_empty() {
        let task = create_test_task("", TaskState::Running);
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::InvalidTaskId(_))));
    }

    #[test]
    fn validate_task_returns_error_when_state_is_completed() {
        let task = create_test_task("task-1", TaskState::Completed);
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::TaskNotRunning(_))));
    }

    #[test]
    fn validate_task_returns_ok_when_state_is_running() {
        let task = create_test_task("task-1", TaskState::Running);
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn validate_task_returns_error_when_state_is_stopped() {
        let task = create_test_task("task-1", TaskState::Stopped);
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::TaskNotRunning(_))));
    }

    #[tokio::test]
    async fn shell_runtime_defaults_command_when_config_is_empty() {
        let runtime = ShellRuntimeAdapter::new(vec![], "-".to_string(), "-".to_string(), None);
        let task = Task {
            id: Some("shell-defaults".into()),
            run: Some("echo hello from shell runtime".to_string()),
            ..Default::default()
        };

        let result = runtime.run(&task).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shell_runtime_publishes_stdout_to_task_logs() {
        let broker: Arc<dyn Broker> = Arc::new(InMemoryBroker::new());
        let log_parts = Arc::new(RwLock::new(Vec::new()));
        let captured_parts = log_parts.clone();

        broker
            .subscribe_for_task_log_part(Arc::new(move |part| {
                let captured_parts = captured_parts.clone();
                Box::pin(async move {
                    captured_parts.write().await.push(part);
                    Ok(())
                })
            }))
            .await
            .expect("subscribing task log handler should succeed");

        let runtime = ShellRuntimeAdapter::new(
            vec!["bash".to_string(), "-c".to_string()],
            "-".to_string(),
            "-".to_string(),
            Some(broker),
        );
        let task = Task {
            id: Some("shell-log-publish".into()),
            run: Some("echo hello from task logs".to_string()),
            ..Default::default()
        };

        let result = runtime.run(&task).await;
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if !log_parts.read().await.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("task log part should be published");
        let published_parts = log_parts.read().await.clone();

        assert!(result.is_ok());
        assert_eq!(published_parts.len(), 1);
        assert_eq!(
            published_parts[0].task_id.as_deref(),
            Some("shell-log-publish")
        );
        assert_eq!(
            published_parts[0].contents.as_deref(),
            Some("hello from task logs\n")
        );
    }
}
