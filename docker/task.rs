//! Task type for Docker runtime.

use std::collections::HashMap;

use crate::docker::tork::{Mount, Probe, Registry, TaskLimits};

/// Task to execute in a container.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub name: Option<String>,
    pub image: String,
    pub cmd: Vec<String>,
    pub entrypoint: Vec<String>,
    pub run: Option<String>,
    pub env: HashMap<String, String>,
    pub files: HashMap<String, String>,
    pub workdir: Option<String>,
    pub limits: Option<TaskLimits>,
    pub mounts: Vec<Mount>,
    pub networks: Vec<String>,
    pub sidecars: Vec<Task>,
    pub pre: Vec<Task>,
    pub post: Vec<Task>,
    pub registry: Option<Registry>,
    pub probe: Option<Probe>,
    pub gpus: Option<String>,
    pub result: Option<String>,
    pub progress: f64,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: None,
            image: String::new(),
            cmd: Vec::new(),
            entrypoint: Vec::new(),
            run: None,
            env: HashMap::new(),
            files: HashMap::new(),
            workdir: None,
            limits: None,
            mounts: Vec::new(),
            networks: Vec::new(),
            sidecars: Vec::new(),
            pre: Vec::new(),
            post: Vec::new(),
            registry: None,
            probe: None,
            gpus: None,
            result: None,
            progress: 0.0,
        }
    }
}
