//! Runtime trait implementation for `PodmanRuntime`.

use std::sync::Arc;

use super::super::types::CoreTask;
use super::types::PodmanRuntime;
use crate::runtime::Runtime;

impl Runtime for PodmanRuntime {
    fn run(&self, task: &CoreTask) -> crate::runtime::BoxedFuture<()> {
        let mut task_clone = task.clone();
        let broker = self.broker.clone();
        let pullq = self.pullq.clone();
        let images = Arc::clone(&self.images);
        let tasks = Arc::clone(&self.tasks);
        let active_tasks = Arc::clone(&self.active_tasks);
        let mounter = Arc::clone(&self.mounter);
        let privileged = self.privileged;
        let host_network = self.host_network;
        let image_verify = self.image_verify;
        let image_ttl = self.image_ttl;

        Box::pin(async move {
            let runtime = PodmanRuntime {
                broker,
                pullq,
                images,
                tasks,
                active_tasks,
                mounter,
                privileged,
                host_network,
                image_verify,
                image_ttl,
            };
            if let Err(e) = runtime.run_inner(&mut task_clone).await {
                tracing::error!(
                    "task {} failed: {}",
                    task_clone.id.as_ref().map_or("", |id| id.as_str()),
                    e
                );
            }
            Ok(())
        })
    }

    fn stop(
        &self,
        task: &CoreTask,
    ) -> crate::runtime::BoxedFuture<crate::runtime::ShutdownResult<std::process::ExitCode>> {
        let task_id = task.id.as_ref().map_or(String::new(), ToString::to_string);
        let tasks = Arc::clone(&self.tasks);

        Box::pin(async move {
            let container_id = {
                let tasks_guard = tasks.read().await;
                tasks_guard.get(&task_id).cloned()
            };

            if let Some(cid) = container_id {
                if let Err(e) = PodmanRuntime::stop_container_static(&cid).await {
                    tracing::warn!("error stopping container {}: {}", cid, e);
                    return Err(anyhow::anyhow!(e));
                }
                let mut tasks_guard = tasks.write().await;
                tasks_guard.remove(&cid);
            }

            Ok(Ok(std::process::ExitCode::SUCCESS))
        })
    }

    fn health_check(&self) -> crate::runtime::BoxedFuture<()> {
        let pullq = self.pullq.clone();
        let images = Arc::clone(&self.images);
        let tasks = Arc::clone(&self.tasks);
        let active_tasks = Arc::clone(&self.active_tasks);
        let mounter = Arc::clone(&self.mounter);
        let privileged = self.privileged;
        let host_network = self.host_network;
        let image_verify = self.image_verify;
        let image_ttl = self.image_ttl;

        Box::pin(async move {
            let runtime = PodmanRuntime {
                broker: None,
                pullq,
                images,
                tasks,
                active_tasks,
                mounter,
                privileged,
                host_network,
                image_verify,
                image_ttl,
            };
            runtime
                .health_check_inner()
                .await
                .map_err(|e| anyhow::anyhow!(e))
        })
    }
}
