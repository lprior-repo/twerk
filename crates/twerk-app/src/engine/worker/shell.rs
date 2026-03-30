use anyhow::anyhow;
use dashmap::DashMap;
use std::process::{ExitCode, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::interval;
use twerk_core::task::{Task, TASK_STATE_ACTIVE};
use twerk_infrastructure::broker::Broker;
use twerk_infrastructure::runtime::{
    BoxedFuture, Runtime as RuntimeTrait, ShutdownError, ShutdownResult,
};

// Module-level function to avoid lifetime issues with associated functions
// Module-level function to avoid lifetime issues with associated functions
fn terminate_process(
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
    let start = std::time::Instant::now();
    let graceful_duration = Duration::from_secs(graceful_timeout);

    loop {
        // Check if process is still running
        if signal::kill(pid, Signal::SIGTERM).is_err() {
            // Process is gone
            break;
        }

        // Check if we've exceeded graceful timeout
        if start.elapsed() >= graceful_duration {
            // Send SIGKILL (force termination)
            if let Err(e) = signal::kill(pid, Signal::SIGKILL) {
                return Err(ShutdownError::TerminationFailed(format!(
                    "failed to send SIGKILL: {}",
                    e
                )));
            }
            break;
        }

        // Wait a bit before checking again
        std::thread::sleep(Duration::from_millis(100));
    }

    // Wait for process to fully exit and get exit code
    match std::process::Command::new("wait")
        .arg(pid.to_string())
        .output()
    {
        Ok(out) => {
            if let Some(code) = out.status.code() {
                return Ok(ExitCode::from(code as u8));
            }
        }
        Err(_) => {
            // If wait fails, assume SIGKILL exit code
            return Ok(ExitCode::from(137)); // SIGKILL = 137
        }
    }

    Ok(ExitCode::SUCCESS)
}

// Module-level function to avoid lifetime issues with associated functions
fn cleanup_temp_dir(temp_dirs: &DashMap<String, String>, task_id: &str) -> ShutdownResult<()> {
    if let Some((_, path)) = temp_dirs.remove(task_id) {
        match std::fs::remove_dir_all(&path) {
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
                graceful_timeout: Self::read_timeout_env("TASK_STOP_GRACEFUL_TIMEOUT", 30),
                force_timeout: Self::read_timeout_env("TASK_STOP_FORCE_TIMEOUT", 5),
                enable_cleanup: Self::read_cleanup_env("TASK_STOP_ENABLE_CLEANUP", true),
            },
            active_processes: Arc::new(DashMap::new()),
            temp_dirs: Arc::new(DashMap::new()),
            broker,
        }
    }

    fn read_timeout_env(key: &str, default: u64) -> u64 {
        match std::env::var(key).ok().and_then(|v| v.parse().ok()) {
            Some(val) => val,
            None => default,
        }
    }

    fn read_cleanup_env(key: &str, default: bool) -> bool {
        match std::env::var(key)
            .ok()
            .map(|v| v.to_lowercase() == "true" || v == "1")
        {
            Some(val) => val,
            None => default,
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
        if !TASK_STATE_ACTIVE.contains(&task.state.as_str()) {
            return Err(ShutdownError::TaskNotRunning(task.state.clone()));
        }

        Ok(())
    }
}

impl RuntimeTrait for ShellRuntimeAdapter {
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

            let pid = child.id().unwrap_or(0);
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
                                                    tracing::warn!(task_id = %task_id_clone, error = %e, "error publishing task progress");
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
            task.state.clone(),
            self.config.clone(),
            self.active_processes.clone(),
            self.temp_dirs.clone(),
        );

        Box::pin(async move {
            // Validate task first (precondition check)
            if task_id_str.is_empty() {
                return Ok(Err(ShutdownError::InvalidTaskId(task_id_str.clone())));
            }
            if !TASK_STATE_ACTIVE.contains(&task_state.as_str()) {
                return Ok(Err(ShutdownError::TaskNotRunning(task_state.clone())));
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
                terminate_process(handle.pid, config.graceful_timeout, config.force_timeout)?;

            // Remove from active processes map
            active_processes.remove(task_id_str.as_str());

            // Cleanup temp files (postcondition)
            if config.enable_cleanup {
                let _ = cleanup_temp_dir(&temp_dirs, task_id_str.as_str());
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
    use twerk_core::task::Task;

    fn create_test_task(id: &str, state: &str) -> Task {
        Task {
            id: Some(id.to_string().into()),
            state: state.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_task_empty_id() {
        let task = create_test_task("", "RUNNING");
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::InvalidTaskId(_))));
    }

    #[test]
    fn test_validate_task_completed_state() {
        let task = create_test_task("task-1", "COMPLETED");
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::TaskNotRunning(_))));
    }

    #[test]
    fn test_validate_task_running_state() {
        let task = create_test_task("task-1", "RUNNING");
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_task_stopped_state() {
        let task = create_test_task("task-1", "STOPPED");
        let result = ShellRuntimeAdapter::validate_task(&task);
        assert!(matches!(result, Err(ShutdownError::TaskNotRunning(_))));
    }
}
