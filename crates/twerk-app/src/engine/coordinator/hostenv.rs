//! Host environment variable injection middleware.
//!
//! Go parity: middleware/task/hostenv.go
//! Injects host machine environment variables into tasks environments
//! when the task transitions to RUNNING state.
//!
//! Config: `middleware.task.hostenv.vars` - list of `"VAR_NAME"` or `HOST_VAR:TASK_VAR"` spec strings.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetition)]

use std::collections::HashMap;
use std::sync::Arc;
use twerk_core::task::{Task, TASK_STATE_RUNNING};
use twerk_infrastructure::config;
use crate::engine::types::{TaskEventType, TaskHandlerFunc, TaskHandlerError, TaskMiddlewareFunc};

pub struct HostEnv {
    vars: HashMap<String, String>,
}

impl HostEnv {
    pub fn new(vars: &[String]) -> Result<Self, String> {
        let mut vars_map = HashMap::new();
        for var_spec in vars {
            let parts: Vec<&str> = var_spec.split(':').collect();
            match parts.len() {
                1 => {
                    vars_map.insert(parts[0].to_string(), parts[0].to_string());
                }
                2 => {
                    vars_map.insert(parts[0].to_string(), parts[1].to_string());
                }
                _ => {
                    return Err(format!("invalid env var spec: {var_spec}"));
                }
            }
        }
        Ok(Self { vars: vars_map })
    }

    pub fn middleware(&self) -> TaskMiddlewareFunc {
        let vars = self.vars.clone();
        Arc::new(move |next: TaskHandlerFunc| {
            let next = next.clone();
            let vars = vars.clone();
            Arc::new(move |ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
                if et == TaskEventType::StateChange && task.state == TASK_STATE_RUNNING {
                    if task.env.is_none() {
                        task.env = Some(HashMap::new());
                    }
                    if let Some(ref mut env) = task.env {
                        for (name, alias) in &vars {
                            if let Ok(v) = std::env::var(name) {
                                env.insert(alias.clone(), v);
                            }
                        }
                    }
                    if let Some(ref mut pre) = task.pre {
                        for t in pre.iter_mut() {
                            set_host_vars_on_subtask(t, &vars);
                        }
                    }
                    if let Some(ref mut post) = task.post {
                        for t in post.iter_mut() {
                            set_host_vars_on_subtask(t, &vars);
                        }
                    }
                }
                next(ctx, et, task)
            })
        })
    }
}

fn set_host_vars_on_subtask(task: &mut Task, vars: &HashMap<String, String>) {
    if task.env.is_none() {
        task.env = Some(HashMap::new());
    }
    if let Some(ref mut env) = task.env {
        for (name, alias) in &vars {
            if let Ok(v) = std::env::var(name) {
                env.insert(alias.clone(), v);
            }
        }
    }
    if let Some(ref mut pre) = task.pre {
        for t in pre.iter_mut() {
            set_host_vars_on_subtask(t, vars);
        }
    }
    if let Some(ref mut post) = task.post {
        for t in post.iter_mut() {
            set_host_vars_on_subtask(t, vars);
        }
    }
}

pub fn create_hostenv_middleware_from_config() -> Option<TaskMiddlewareFunc> {
    let vars = config::strings_default("middleware.task.hostenv.vars", &[]);
    if vars.is_empty() {
        return None;
    }
    match HostEnv::new(&vars) {
        Ok(hostenv) => Some(hostenv.middleware()),
        Err(e) => {
            tracing::warn!("invalid hostenv config: {}", e);
            None
        }
    }
}
