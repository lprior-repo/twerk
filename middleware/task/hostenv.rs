//! Host environment middleware.
//!
//! This middleware injects host environment variables into tasks
//! when they transition to the RUNNING state.

use crate::middleware::task::task_error::TaskMiddlewareError;
use crate::middleware::task::task_handler::{Context, HandlerFunc, MiddlewareFunc};
use crate::middleware::task::task_types::EventType;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tork::task::Task;
use tork::task::TASK_STATE_RUNNING;

/// Errors that can occur during host environment operations.
#[derive(Debug, thiserror::Error)]
pub enum HostEnvError {
    /// Invalid environment variable specification.
    #[error("invalid env var spec: {0}")]
    InvalidSpec(String),
}

/// Host environment middleware.
///
/// Injects host environment variables into task environments when
/// tasks start running.
#[derive(Debug, Clone)]
pub struct HostEnv {
    /// Mapping of host variable names to task environment variable names.
    vars: HashMap<String, String>,
}

impl HostEnv {
    /// Creates a new HostEnv from variable specifications.
    ///
    /// Each spec can be either:
    /// - `"VAR_NAME"` - reads VAR_NAME and stores as VAR_NAME
    /// - `"HOST_VAR:TASK_VAR"` - reads HOST_VAR and stores as TASK_VAR
    pub fn new(vars: &[&str]) -> Result<Self, HostEnvError> {
        let mut vars_map = HashMap::new();

        for var_spec in vars {
            let parts: Vec<&str> = var_spec.split(':').collect();

            match parts.len() {
                1 => {
                    let var_name = parts[0];
                    if var_name.is_empty() {
                        return Err(HostEnvError::InvalidSpec(var_spec.to_string()));
                    }
                    vars_map.insert(var_name.to_string(), var_name.to_string());
                }
                2 => {
                    let host_var = parts[0];
                    let task_var = parts[1];
                    if host_var.is_empty() || task_var.is_empty() {
                        return Err(HostEnvError::InvalidSpec(var_spec.to_string()));
                    }
                    vars_map.insert(host_var.to_string(), task_var.to_string());
                }
                _ => {
                    return Err(HostEnvError::InvalidSpec(var_spec.to_string()));
                }
            }
        }

        Ok(Self { vars: vars_map })
    }

    /// Returns a middleware function that wraps the given handler.
    #[must_use]
    pub fn middleware(&self) -> MiddlewareFunc {
        let vars = self.vars.clone();
        Arc::new(move |next: HandlerFunc| -> HandlerFunc {
            let vars = vars.clone();
            Arc::new(move |ctx: Context, et: EventType, task: &mut Task| {
                let vars = vars.clone();

                // Set host vars when task transitions to RUNNING state
                if et == EventType::StateChange && task.state == TASK_STATE_RUNNING {
                    set_host_vars(task, &vars);
                }

                next(ctx, et, task)
            })
        })
    }
}

/// Sets host environment variables in a task.
fn set_host_vars(task: &mut Task, vars: &HashMap<String, String>) {
    // Initialize env map if needed
    if task.env.is_none() {
        task.env = Some(HashMap::new());
    }

    // Set each host variable
    if let Some(env) = &mut task.env {
        for (host_name, task_name) in vars {
            if let Ok(value) = env::var(host_name) {
                env.insert(task_name.clone(), value);
            }
        }
    }

    // Recursively set host vars in pre tasks
    if let Some(pre) = &task.pre {
        for pre_task in pre {
            let mut pre_task_mut = pre_task.clone();
            set_host_vars(&mut pre_task_mut, vars);
        }
    }

    // Recursively set host vars in post tasks
    if let Some(post) = &task.post {
        for post_task in post {
            let mut post_task_mut = post_task.clone();
            set_host_vars(&mut post_task_mut, vars);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_host_env_simple() {
        let host_env = HostEnv::new(&["TORK_HOST_VAR1"]).unwrap();
        assert_eq!(
            host_env.vars.get("TORK_HOST_VAR1"),
            Some(&"TORK_HOST_VAR1".to_string())
        );
    }

    #[test]
    fn test_new_host_env_with_alias() {
        let host_env = HostEnv::new(&["HOST_VAR:ALIAS"]).unwrap();
        assert_eq!(host_env.vars.get("HOST_VAR"), Some(&"ALIAS".to_string()));
    }

    #[test]
    fn test_new_host_env_invalid_spec() {
        let result = HostEnv::new(&["VAR1:VAR2:VAR3"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_host_env_empty() {
        let host_env = HostEnv::new(&[]).unwrap();
        assert!(host_env.vars.is_empty());
    }
}
