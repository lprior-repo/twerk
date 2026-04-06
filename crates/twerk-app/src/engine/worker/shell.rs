use anyhow::anyhow;
use dashmap::DashMap;
use std::process::{ExitCode, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::interval;
use twerk_core::env::{read_cleanup_env, read_timeout_env};
use twerk_core::task::{is_task_state_active, Task};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::docker::config::ERROR_PUBLISHING_TASK_PROGRESS;
use twerk_infrastructure::runtime::{
    BoxedFuture, Runtime as RuntimeTrait, ShutdownError, ShutdownResult,
};

use tracing::instrument;

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
        Self {
            config: ShellRuntimeConfig {
                cmd,
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
                task.id.clone().unwrap_or_default().to_string(),
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
        let (sc, tid, rs, env, active_processes, temp_dirs, broker) = (
            self.config.cmd.clone(),
            task.id.clone().unwrap_or_default(),
            task.run.clone().unwrap_or_default(),
            task.env.clone(),
            self.active_processes.clone(),
            self.temp_dirs.clone(),
            self.broker.clone(),
        );

        Box::pin(async move {
            if tid.is_empty() || rs.is_empty() {
                return Err(anyhow!("id and run script required"));
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
            let mut cmd = Command::new(&sc[0]);
            cmd.arg(sp.to_string_lossy().as_ref())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // Set TORK_PROGRESS env var for progress file location
            cmd.env("TORK_PROGRESS", progress_path.to_string_lossy().as_ref());

            if let Some(ref e) = env {
                for (k, v) in e {
                    cmd.env(k, v);
                }
            }

            // Track the child handle for potential future use
            let mut child = cmd.spawn()?;

            let pid = child
                .id()
                .ok_or_else(|| anyhow::anyhow!("child process spawned but PID unavailable"))?;
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
                                                    id: Some(task_id_clone.clone()),
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

            // Wait for completion
            let status = child.wait().await?;
            if !status.success() {
                return Err(anyhow!("failed with {:?}", status.code()));
            }

            Ok(())
        })
    }

    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>> {
        let (task_id_str, task_state, config, active_processes, temp_dirs) = (
            task.id.clone().unwrap_or_default().to_string(),
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
    use twerk_core::task::{Task, TaskState};

    fn create_test_task(id: &str, state: TaskState) -> Task {
        Task {
            id: Some(id.to_string().into()),
            state,
            ..Default::default()
        }
    }

    #[test]
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
        assert!(result.is_ok());
    }

    #[test]
    fn validate_task_returns_error_when_state_is_stopped() {
        let task = create_test_task("task-1", TaskState::Stopped);
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::TaskNotRunning(_))));
    }
}
