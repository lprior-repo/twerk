//! Implementation blocks for task types.

use super::state::TASK_STATE_ACTIVE;
use super::types::*;

fn clone_tasks(tasks: &[Task]) -> Vec<Task> {
    tasks.iter().map(Task::clone).collect()
}

impl Task {
    /// Returns true if the task is in an active state.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state
            .map(|state| TASK_STATE_ACTIVE.contains(&state))
            .unwrap_or(false)
    }

    /// Creates a deep clone of the task.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            job_id: self.job_id.clone(),
            parent_id: self.parent_id.clone(),
            position: self.position,
            name: self.name.clone(),
            description: self.description.clone(),
            state: self.state,
            created_at: self.created_at,
            scheduled_at: self.scheduled_at,
            started_at: self.started_at,
            completed_at: self.completed_at,
            failed_at: self.failed_at,
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run.clone(),
            image: self.image.clone(),
            registry: self.registry.clone(),
            env: self.env.clone(),
            files: self.files.clone(),
            queue: self.queue.clone(),
            redelivered: self.redelivered,
            error: self.error.clone(),
            pre: self.pre.as_ref().map(clone_tasks),
            post: self.post.as_ref().map(clone_tasks),
            sidecars: self.sidecars.as_ref().map(clone_tasks),
            mounts: self.mounts.clone(),
            networks: self.networks.clone(),
            node_id: self.node_id.clone(),
            retry: self.retry.clone(),
            limits: self.limits.clone(),
            timeout: self.timeout.clone(),
            result: self.result.clone(),
            var: self.var.clone(),
            r#if: self.r#if.clone(),
            parallel: self.parallel.clone(),
            each: self.each.clone(),
            subjob: self.subjob.clone(),
            gpus: self.gpus.clone(),
            tags: self.tags.clone(),
            workdir: self.workdir.clone(),
            priority: self.priority,
            progress: self.progress,
            probe: self.probe.clone(),
        }
    }
}

impl TaskSummary {
    /// Creates a new TaskSummary from a Task.
    #[must_use]
    pub fn from_task(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            job_id: task.job_id.clone(),
            name: task.name.clone(),
            state: task.state,
            created_at: task.created_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
            error: task.error.clone(),
            result: task.result.clone(),
            progress: task.progress,
        }
    }
}

impl SubJobTask {
    /// Creates a deep clone of the subjob task.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            tasks: self.tasks.as_ref().map(clone_tasks),
            inputs: self.inputs.clone(),
            secrets: self.secrets.clone(),
            auto_delete: self.auto_delete.clone(),
            output: self.output.clone(),
            detached: self.detached,
        }
    }
}

impl ParallelTask {
    /// Creates a deep clone of the parallel task.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.as_ref().map(clone_tasks),
            completions: self.completions,
        }
    }
}

impl EachTask {
    /// Creates a deep clone of the each task.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            var: self.var.clone(),
            list: self.list.clone(),
            task: self.task.clone(),
            size: self.size,
            completions: self.completions,
            concurrency: self.concurrency,
            index: self.index,
        }
    }
}

impl TaskRetry {
    /// Creates a deep clone of the task retry.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            limit: self.limit,
            attempts: self.attempts,
        }
    }
}

impl TaskLimits {
    /// Creates a deep clone of the task limits.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            cpus: self.cpus.clone(),
            memory: self.memory.clone(),
        }
    }
}

impl Registry {
    /// Creates a deep clone of the registry.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

impl Probe {
    /// Creates a deep clone of the probe.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            port: self.port,
            timeout: self.timeout.clone(),
        }
    }
}

impl Mount {
    /// Creates a deep clone of the mount.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            mount_type: self.mount_type.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            opts: self.opts.clone(),
            read_only: self.read_only,
        }
    }
}

impl AutoDelete {
    /// Creates a deep clone of the auto delete.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            after: self.after.clone(),
        }
    }
}

impl Webhook {
    /// Creates a deep clone of the webhook.
    #[must_use]
    pub fn clone(&self) -> Self {
        Self {
            url: self.url.clone(),
            headers: self.headers.clone(),
            event: self.event.clone(),
            r#if: self.r#if.clone(),
        }
    }
}

// Implement Default manually since we use Option fields throughout
impl Default for Task {
    fn default() -> Self {
        Self {
            id: None,
            job_id: None,
            parent_id: None,
            position: None,
            name: None,
            description: None,
            state: None,
            created_at: None,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            failed_at: None,
            cmd: None,
            entrypoint: None,
            run: None,
            image: None,
            registry: None,
            env: None,
            files: None,
            queue: None,
            redelivered: None,
            error: None,
            pre: None,
            post: None,
            sidecars: None,
            mounts: None,
            networks: None,
            node_id: None,
            retry: None,
            limits: None,
            timeout: None,
            result: None,
            var: None,
            r#if: None,
            parallel: None,
            each: None,
            subjob: None,
            gpus: None,
            tags: None,
            workdir: None,
            priority: None,
            progress: None,
            probe: None,
        }
    }
}
