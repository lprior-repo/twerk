//! Domain types for podman runtime.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub run: String,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub env: HashMap<String, String>,
    pub mounts: Vec<Mount>,
    pub files: HashMap<String, String>,
    pub networks: Vec<String>,
    pub limits: Option<TaskLimits>,
    pub registry: Option<Registry>,
    pub gpus: Option<String>,
    pub probe: Option<Probe>,
    pub sidecars: Vec<Task>,
    pub pre: Vec<Task>,
    pub post: Vec<Task>,
    pub workdir: Option<String>,
    pub result: String,
    pub progress: f64,
}

#[derive(Debug, Clone)]
pub struct Mount {
    pub id: String,
    pub mount_type: MountType,
    pub source: String,
    pub target: String,
    pub opts: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MountType {
    Volume,
    Bind,
    Tmpfs,
}

impl std::fmt::Display for MountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountType::Volume => write!(f, "volume"),
            MountType::Bind => write!(f, "bind"),
            MountType::Tmpfs => write!(f, "tmpfs"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskLimits {
    pub cpus: String,
    pub memory: String,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct Probe {
    pub path: String,
    pub port: i64,
    pub timeout: String,
}

pub mod slug {
    pub fn make(s: &str) -> String {
        s.to_lowercase()
            .chars()
            .map(|c| if c == ' ' { '-' } else { c })
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>()
    }
}
