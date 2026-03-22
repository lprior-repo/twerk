//! Task-related input types for Tork
//!
//! These types represent the various task configurations in Tork.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export webhook and auto-delete from job module since they're shared
pub use crate::job::AutoDelete;
pub use crate::job::Webhook;

use tork::mount::Mount as TorkMount;
use tork::task::{
    AutoDelete as TorkAutoDelete, EachTask, ParallelTask, Probe as TorkProbe,
    Registry as TorkRegistry, SubJobTask, Task as TorkTask, TaskLimits as TorkTaskLimits,
    TaskRetry as TorkTaskRetry, Webhook as TorkWebhook,
};

/// Task input type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub struct Task {
    /// Task name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Task description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,

    /// Entrypoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,

    /// Run specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,

    /// Docker image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Registry credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,

    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Files to mount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,

    /// Queue name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue: Option<String>,

    /// Pre-requisite tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<Vec<AuxTask>>,

    /// Post-requisite tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Vec<AuxTask>>,

    /// Sidecar tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sidecars: Option<Vec<SidecarTask>>,

    /// Mounts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounts: Option<Vec<Mount>>,

    /// Networks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<String>>,

    /// Retry configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<Retry>,

    /// Resource limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Limits>,

    /// Timeout duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,

    /// Variable name for result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,

    /// Conditional expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#if: Option<String>,

    /// Parallel task configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel: Option<Parallel>,

    /// Each-task (loop) configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub each: Option<Each>,

    /// Subjob task configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjob: Option<SubJob>,

    /// GPU specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,

    /// Tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    /// Working directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,

    /// Priority (0-9)
    #[serde(default)]
    pub priority: i64,
}

/// SubJob task configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SubJob {
    /// SubJob ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// SubJob name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,

    /// Inputs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, String>>,

    /// Secrets
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, String>>,

    /// Auto-delete configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_delete: Option<AutoDelete>,

    /// Output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,

    /// Detached mode
    #[serde(default)]
    pub detached: bool,

    /// Webhooks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<Webhook>>,
}

/// Each-task loop configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Each {
    /// Loop variable name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub var: Option<String>,

    /// List expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,

    /// Task to execute for each item
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Box<Task>>,

    /// Concurrency limit
    #[serde(default)]
    pub concurrency: i64,
}

/// Parallel task configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parallel {
    /// Tasks to execute in parallel
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,
}

/// Retry configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(Default)]
pub struct Retry {
    /// Maximum retry count
    #[serde(default)]
    pub limit: i64,
}

/// Resource limits
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// CPU limit string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,

    /// Memory limit string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// Registry credentials
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    /// Username
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Mount configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Mount {
    /// Mount type (volume, bind, tmpfs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mount_type: Option<String>,

    /// Source path or volume name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Target path in container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Mount options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opts: Option<HashMap<String, String>>,
}

/// Auxiliary task (pre/post)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuxTask {
    /// Task name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Task description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,

    /// Entrypoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,

    /// Run specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,

    /// Docker image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Registry credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,

    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Files to mount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,

    /// Timeout duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}

/// Sidecar task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SidecarTask {
    /// Task name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Task description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Command to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,

    /// Entrypoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,

    /// Run specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,

    /// Docker image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Registry credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,

    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Files to mount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<HashMap<String, String>>,

    /// Timeout duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,

    /// Health probe
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<Probe>,
}

/// Health probe configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    /// Probe path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Probe port
    #[serde(default)]
    pub port: i64,

    /// Timeout duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
}


impl Task {
    /// Creates a new Task with the given name and image
    #[must_use]
    pub fn new(name: impl Into<String>, image: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            image: Some(image.into()),
            ..Default::default()
        }
    }

    /// Returns true if this is a composite task (parallel, each, or subjob)
    #[must_use]
    pub fn is_composite(&self) -> bool {
        self.parallel.is_some() || self.each.is_some() || self.subjob.is_some()
    }

    /// Returns true if this task has a valid name
    #[must_use]
    pub fn has_name(&self) -> bool {
        self.name.as_ref().is_some_and(|n| !n.is_empty())
    }

    /// Returns true if this task has an image
    #[must_use]
    pub fn has_image(&self) -> bool {
        self.image.as_ref().is_some_and(|i| !i.is_empty())
    }

    /// Returns true if this task has a command
    #[must_use]
    pub fn has_command(&self) -> bool {
        self.cmd.as_ref().is_some_and(|c| !c.is_empty())
            || self.run.as_ref().is_some_and(|r| !r.is_empty())
    }
}

impl Retry {
    /// Creates a new Retry with the given limit
    #[must_use]
    pub fn new(limit: i64) -> Self {
        Self { limit }
    }

    /// Returns true if the limit is valid (1-10)
    #[must_use]
    pub fn is_valid_limit(&self) -> bool {
        (1..=10).contains(&self.limit)
    }
}


impl Limits {
    /// Creates a new Limits with the given CPU and memory
    #[must_use]
    pub fn new(cpus: impl Into<String>, memory: impl Into<String>) -> Self {
        Self {
            cpus: Some(cpus.into()),
            memory: Some(memory.into()),
        }
    }
}

impl Mount {
    /// Returns the mount type, defaulting to "volume"
    #[must_use]
    pub fn mount_type_or_default(&self) -> &str {
        self.mount_type.as_deref().unwrap_or("volume")
    }

    /// Returns true if this is a bind mount
    #[must_use]
    pub fn is_bind(&self) -> bool {
        self.mount_type_or_default() == "bind"
    }

    /// Returns true if this is a volume mount
    #[must_use]
    pub fn is_volume(&self) -> bool {
        self.mount_type_or_default() == "volume"
    }

    /// Convert to domain Mount type
    #[must_use]
    pub fn to_tork(&self) -> TorkMount {
        TorkMount {
            id: None,
            mount_type: self
                .mount_type
                .clone()
                .unwrap_or_else(|| "volume".to_string()),
            source: self.source.clone(),
            target: self.target.clone(),
            opts: self.opts.clone(),
        }
    }
}

impl Retry {
    /// Convert to domain TaskRetry type
    #[must_use]
    pub fn to_tork(&self) -> TorkTaskRetry {
        TorkTaskRetry {
            limit: self.limit,
            attempts: 0,
        }
    }
}

impl Limits {
    /// Convert to domain TaskLimits type
    #[must_use]
    pub fn to_tork(&self) -> TorkTaskLimits {
        TorkTaskLimits {
            cpus: self.cpus.clone(),
            memory: self.memory.clone(),
        }
    }
}

impl Registry {
    /// Convert to domain Registry type
    #[must_use]
    pub fn to_tork(&self) -> TorkRegistry {
        TorkRegistry {
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }
}

impl Probe {
    /// Convert to domain Probe type
    #[must_use]
    pub fn to_tork(&self) -> TorkProbe {
        TorkProbe {
            path: self.path.clone(),
            port: self.port,
            timeout: self.timeout.clone(),
        }
    }
}

impl Webhook {
    /// Convert to domain Webhook type
    #[must_use]
    pub fn to_tork(&self) -> TorkWebhook {
        TorkWebhook {
            url: self.url.clone(),
            headers: self.headers.clone(),
            event: self.event.clone(),
            r#if: self.r#if.clone(),
        }
    }
}

impl AutoDelete {
    /// Convert to domain AutoDelete type
    #[must_use]
    pub fn to_tork(&self) -> TorkAutoDelete {
        TorkAutoDelete {
            after: self.after.clone(),
        }
    }
}

impl AuxTask {
    /// Convert to domain Task type
    #[must_use]
    pub fn to_tork(&self) -> TorkTask {
        TorkTask {
            name: self.name.clone(),
            description: self.description.clone(),
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run.clone(),
            image: self.image.clone(),
            env: self.env.clone(),
            timeout: self.timeout.clone(),
            files: self.files.clone(),
            registry: self.registry.as_ref().map(|r| r.to_tork()),
            ..Default::default()
        }
    }
}

impl SidecarTask {
    /// Convert to domain Task type
    #[must_use]
    pub fn to_tork(&self) -> TorkTask {
        TorkTask {
            name: self.name.clone(),
            description: self.description.clone(),
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run.clone(),
            image: self.image.clone(),
            env: self.env.clone(),
            timeout: self.timeout.clone(),
            files: self.files.clone(),
            registry: self.registry.as_ref().map(|r| r.to_tork()),
            probe: self.probe.as_ref().map(|p| p.to_tork()),
            ..Default::default()
        }
    }
}

impl Task {
    /// Convert to domain Task type
    #[must_use]
    pub fn to_tork(&self) -> TorkTask {
        let pre = to_aux_tasks(self.pre.as_deref());
        let post = to_aux_tasks(self.post.as_deref());
        let sidecars = to_sidecar_tasks(self.sidecars.as_deref());
        let retry = self.retry.as_ref().map(|r| r.to_tork());
        let limits = self.limits.as_ref().map(|l| l.to_tork());

        let each = self.each.as_ref().map(|e| EachTask {
            var: e.var.clone(),
            list: e.list.clone(),
            task: e.task.as_ref().map(|t| Box::new(t.to_tork())),
            size: 0,
            completions: 0,
            concurrency: e.concurrency,
            index: 0,
        });

        let subjob = self.subjob.as_ref().map(|sj| {
            let webhooks = sj
                .webhooks
                .as_ref()
                .map(|whs| whs.iter().map(|wh| wh.to_tork()).collect());
            SubJobTask {
                id: None,
                name: sj.name.clone(),
                description: sj.description.clone(),
                tasks: to_tasks(sj.tasks.as_deref()),
                inputs: sj.inputs.clone(),
                secrets: sj.secrets.clone(),
                auto_delete: sj.auto_delete.as_ref().map(|ad| ad.to_tork()),
                output: sj.output.clone(),
                detached: sj.detached,
                webhooks,
            }
        });

        let parallel = self.parallel.as_ref().map(|p| ParallelTask {
            tasks: to_tasks(p.tasks.as_deref()),
            completions: 0,
        });

        let registry = self.registry.as_ref().map(|r| r.to_tork());

        TorkTask {
            name: self.name.clone(),
            description: self.description.clone(),
            cmd: self.cmd.clone(),
            entrypoint: self.entrypoint.clone(),
            run: self.run.clone(),
            image: self.image.clone(),
            registry,
            env: self.env.clone(),
            files: self.files.clone(),
            queue: self.queue.clone(),
            pre,
            post,
            sidecars,
            mounts: to_mounts(self.mounts.as_deref()),
            networks: self.networks.clone(),
            retry,
            limits,
            timeout: self.timeout.clone(),
            var: self.var.clone(),
            r#if: self.r#if.clone(),
            parallel,
            each,
            subjob,
            gpus: self.gpus.clone(),
            tags: self.tags.clone(),
            workdir: self.workdir.clone(),
            priority: self.priority,
            ..Default::default()
        }
    }
}

/// Convert a slice of Mounts to domain Mounts
#[must_use]
pub fn to_mounts(ms: Option<&[Mount]>) -> Option<Vec<TorkMount>> {
    ms.map(|mounts| mounts.iter().map(|m| m.to_tork()).collect())
}

/// Convert a slice of AuxTasks to domain Tasks
#[must_use]
pub fn to_aux_tasks(tis: Option<&[AuxTask]>) -> Option<Vec<TorkTask>> {
    tis.map(|tasks| tasks.iter().map(|t| t.to_tork()).collect())
}

/// Convert a slice of SidecarTasks to domain Tasks
#[must_use]
pub fn to_sidecar_tasks(tis: Option<&[SidecarTask]>) -> Option<Vec<TorkTask>> {
    tis.map(|tasks| tasks.iter().map(|t| t.to_tork()).collect())
}

/// Convert a slice of Tasks to domain Tasks
#[must_use]
pub fn to_tasks(tis: Option<&[Task]>) -> Option<Vec<TorkTask>> {
    tis.map(|tasks| tasks.iter().map(|t| t.to_tork()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_new() {
        let task = Task::new("my-task", "ubuntu:20.04");
        assert_eq!(task.name, Some("my-task".to_string()));
        assert_eq!(task.image, Some("ubuntu:20.04".to_string()));
    }

    #[test]
    fn test_task_is_composite() {
        let task = Task::new("test", "image");
        assert!(!task.is_composite());

        let each_task = Task {
            each: Some(Each {
                list: Some("1+1".to_string()),
                ..Default::default()
            }),
            ..Task::new("test", "image")
        };
        assert!(each_task.is_composite());
    }

    #[test]
    fn test_retry_valid_limit() {
        let retry = Retry::new(5);
        assert!(retry.is_valid_limit());

        let retry = Retry::new(15);
        assert!(!retry.is_valid_limit());
    }

    #[test]
    fn test_mount_type() {
        let mount = Mount {
            mount_type: Some("bind".to_string()),
            ..Default::default()
        };
        assert!(mount.is_bind());
        assert!(!mount.is_volume());
    }
}
