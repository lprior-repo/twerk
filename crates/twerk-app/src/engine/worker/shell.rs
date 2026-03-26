use anyhow::{anyhow, Result};
use std::process::Stdio;
use tokio::process::Command;
use twerk_core::task::Task;
use twerk_infrastructure::runtime::{BoxedFuture, Runtime as RuntimeTrait};

#[derive(Debug)]
pub struct ShellRuntimeAdapter {
    cmd: Vec<String>,
    uid: String,
    gid: String,
}

impl ShellRuntimeAdapter {
    #[must_use]
    pub fn new(cmd: Vec<String>, uid: String, gid: String) -> Self {
        Self {
            cmd: if cmd.is_empty() { vec!["bash".to_string(), "-c".to_string()] } else { cmd },
            uid: if uid.is_empty() { "-".to_string() } else { uid },
            gid: if gid.is_empty() { "-".to_string() } else { gid },
        }
    }
}

impl RuntimeTrait for ShellRuntimeAdapter {
    fn run(&self, task: &Task) -> BoxedFuture<()> {
        let (sc, tid, rs, env) = (
            self.cmd.clone(),
            task.id.clone().unwrap_or_default(),
            task.run.clone().unwrap_or_default(),
            task.env.clone(),
        );
        let timeout = task.timeout.as_deref().and_then(|s| {
            let s = s.trim();
            if s.is_empty() { return Some(std::time::Duration::from_secs(0)); }
            let (ns, u) = if let Some(st) = s.strip_suffix("ms") { (st, "ms") }
            else if let Some(st) = s.strip_suffix('s') { (st, "s") }
            else if let Some(st) = s.strip_suffix('m') { (st, "m") }
            else if let Some(st) = s.strip_suffix('h') { (st, "h") }
            else { return None; };
            let n: u64 = ns.parse().ok()?;
            Some(match u {
                "ms" => std::time::Duration::from_millis(n),
                "s" => std::time::Duration::from_secs(n),
                "m" => std::time::Duration::from_secs(n * 60),
                "h" => std::time::Duration::from_secs(n * 3600),
                _ => return None,
            })
        });
        Box::pin(async move {
            if tid.as_str().is_empty() || rs.is_empty() { return Err(anyhow!("id and run script required")); }
            let td = tempfile::tempdir()?;
            let sp = td.path().join("script.sh");
            tokio::fs::write(&sp, format!("#!/bin/bash\n{rs}")).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(&sp, std::fs::Permissions::from_mode(0o755)).await?;
            }
            let mut c = Command::new(&sc[0]);
            c.arg(sp.to_string_lossy().as_ref()).stdout(Stdio::piped()).stderr(Stdio::piped());
            if let Some(ref e) = env { for (k, v) in e { c.env(k, v); } }
            let out = match timeout {
                Some(d) => tokio::time::timeout(d, c.output()).await.map_err(|_| anyhow!("timeout"))??,
                None => c.output().await?,
            };
            if !out.status.success() { return Err(anyhow!("failed with {:?}", out.status.code())); }
            Ok(())
        })
    }

    fn stop(&self, _task: &Task) -> BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn health_check(&self) -> BoxedFuture<()> { Box::pin(async { Ok(()) }) }
}
