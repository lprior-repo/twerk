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