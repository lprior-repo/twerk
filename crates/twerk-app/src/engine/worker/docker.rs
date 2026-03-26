use anyhow::anyhow;
use bollard::models::ContainerCreateBody;
use bollard::query_parameters::{CreateContainerOptions, CreateImageOptions, RemoveContainerOptions};
use bollard::Docker;
use dashmap::DashMap;
use futures_util::StreamExt;
use std::sync::Arc;
use tracing::debug;
use twerk_core::id::TaskId;
use twerk_core::task::Task;
use std::process::ExitCode;
use twerk_infrastructure::runtime::{BoxedFuture, Runtime as RuntimeTrait, ShutdownResult};

#[derive(Debug, Default)]
pub struct DockerRuntimeAdapter {
    privileged: bool,
    active_tasks: Arc<DashMap<TaskId, String>>,
}

impl DockerRuntimeAdapter {
    #[must_use]
    pub fn new(privileged: bool) -> Self {
        Self { privileged, active_tasks: Arc::new(DashMap::new()) }
    }
}

impl RuntimeTrait for DockerRuntimeAdapter {
    fn run(&self, task: &Task) -> BoxedFuture<()> {
        let p = self.privileged;
        let tid = task.id.clone().unwrap_or_default();
        let img = task.image.clone().unwrap_or_default();
        let active = self.active_tasks.clone();
        let (cmd, env, wd) = (task.cmd.clone(), task.env.clone(), task.workdir.clone());
        Box::pin(async move {
            if tid.as_str().is_empty() || img.is_empty() { return Err(anyhow!("id and image required")); }
            let d = Docker::connect_with_local_defaults()?;
            if d.inspect_image(&img).await.is_err() {
                let mut s = d.create_image(Some(CreateImageOptions { from_image: Some(img.clone()), ..Default::default() }), None, None);
                while let Some(res) = s.next().await { if let Err(e) = res { debug!("pull error: {e}"); } }
            }
            let c_body = ContainerCreateBody {
                image: Some(img), cmd, working_dir: wd,
                env: env.map(|e| e.iter().map(|(k, v)| format!("{k}={v}")).collect()),
                host_config: Some(bollard::models::HostConfig { privileged: Some(p), ..Default::default() }),
                ..Default::default()
            };
            let cid = d.create_container(None::<CreateContainerOptions>, c_body).await?.id;
            active.insert(tid.clone(), cid.clone());
            d.start_container(&cid, None).await?;
            let mut ec = 1;
            for _ in 0..60 {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                if let Ok(i) = d.inspect_container(&cid, None).await {
                    if let Some(s) = i.state { if !s.running.unwrap_or(false) { ec = s.exit_code.unwrap_or(1); break; } }
                }
            }
            active.remove(&tid);
            let _ = d.remove_container(&cid, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
            if ec != 0 { return Err(anyhow!("exited with {ec}")); }
            Ok(())
        })
    }

    fn stop(&self, task: &Task) -> BoxedFuture<ShutdownResult<ExitCode>> {
        let tid = task.id.clone().unwrap_or_default();
        let active = self.active_tasks.clone();
        Box::pin(async move {
            if let Some((_, cid)) = active.remove(&tid) {
                let d = Docker::connect_with_local_defaults()?;
                let _ = d.stop_container(&cid, None).await;
                let _ = d.remove_container(&cid, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
            }
            Ok(Ok(ExitCode::SUCCESS))
        })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Docker::connect_with_local_defaults()?.ping().await.map(|_| ()).map_err(|e| anyhow!(e)) })
    }
}
