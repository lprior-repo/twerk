use anyhow::{anyhow, Result};
use tokio::process::Command;
use twerk_core::task::Task;
use twerk_infrastructure::runtime::{BoxedFuture, Runtime as RuntimeTrait};

#[derive(Debug)]
pub struct PodmanRuntimeAdapter {
    privileged: bool,
    host_network: bool,
}

impl PodmanRuntimeAdapter {
    #[must_use]
    pub fn new(privileged: bool, host_network: bool) -> Self {
        Self { privileged, host_network }
    }
}

impl RuntimeTrait for PodmanRuntimeAdapter {
    fn run(&self, task: &Task) -> BoxedFuture<()> {
        let (p, h, tid, img, cmd, wd, env) = (
            self.privileged,
            self.host_network,
            task.id.clone().unwrap_or_default(),
            task.image.clone().unwrap_or_default(),
            task.cmd.clone(),
            task.workdir.clone(),
            task.env.clone(),
        );
        Box::pin(async move {
            if tid.as_str().is_empty() || img.is_empty() { return Err(anyhow!("id and image required")); }
            let mut c = Command::new("podman");
            c.arg("run");
            if p { c.arg("--privileged"); }
            if h { c.arg("--network").arg("host"); }
            c.arg(&img);
            if let Some(ref a) = cmd { for arg in a { c.arg(arg); } }
            if let Some(ref w) = wd { c.arg("--workdir").arg(w); }
            if let Some(ref e) = env { for (k, v) in e { c.env(k, v); } }
            let out = c.output().await?;
            if !out.status.success() { return Err(anyhow!("podman failed: {}", String::from_utf8_lossy(&out.stderr))); }
            Ok(())
        })
    }

    fn stop(&self, _task: &Task) -> BoxedFuture<()> { Box::pin(async { Ok(()) }) }
    fn health_check(&self) -> BoxedFuture<()> { Box::pin(async { Ok(()) }) }
}
