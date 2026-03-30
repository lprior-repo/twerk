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
#![allow(clippy::module_name_repetitions)]

use crate::engine::types::{TaskEventType, TaskHandlerFunc, TaskMiddlewareFunc};
use std::collections::HashMap;
use std::sync::Arc;
use twerk_core::task::{Task, TASK_STATE_RUNNING};
use twerk_infrastructure::config;

/// Host environment configuration - maps host env var names to task env var names.
#[derive(Debug, Clone)]
pub struct HostEnv {
    /// Mapping from host variable name to task variable name.
    vars: HashMap<String, String>,
}

/// Parses a single variable specification string.
/// Format: `"HOST_VAR"` or `"HOST_VAR:TASK_VAR"`
///
/// # Errors
/// Returns an error if the spec string is invalid (contains more than one `:`).
fn parse_var_spec(spec: &str) -> Result<(String, String), String> {
    match spec.split(':').collect::<Vec<_>>()[..] {
        [host] => Ok((host.to_string(), host.to_string())),
        [host, task] => Ok((host.to_string(), task.to_string())),
        _ => Err(format!("invalid env var spec: {spec}")),
    }
}

/// Parses all variable specification strings into a mapping.
/// Returns a `HashMap` of `host_var` -> `task_var`.
///
/// # Errors
/// Returns an error if any spec string is invalid.
fn parse_var_specs(specs: &[String]) -> Result<HashMap<String, String>, String> {
    specs
        .iter()
        .map(|spec| parse_var_spec(spec))
        .collect::<Result<HashMap<_, _>, _>>()
}

/// Looks up environment variables that exist on the host system.
/// Returns only the variables that exist (filtering out missing ones).
fn get_existing_env_vars(mapping: &HashMap<String, String>) -> HashMap<String, String> {
    mapping
        .iter()
        .filter_map(|(host, task)| std::env::var(host).ok().map(|v| (task.clone(), v)))
        .collect()
}

/// Merges environment variables into a task's env, creating the map if needed.
fn merge_env_into_task(task: &mut Task, env_vars: HashMap<String, String>) {
    if task.env.is_none() {
        task.env = Some(HashMap::new());
    }
    if let Some(ref mut env) = task.env {
        env.extend(env_vars);
    }
}

/// Applies host environment variables to a subtask recursively.
fn apply_host_vars_recursive(task: &mut Task, env_vars: &HashMap<String, String>) {
    merge_env_into_task(task, env_vars.clone());

    if let Some(ref mut pre) = task.pre {
        for t in pre.iter_mut() {
            apply_host_vars_recursive(t, env_vars);
        }
    }

    if let Some(ref mut post) = task.post {
        for t in post.iter_mut() {
            apply_host_vars_recursive(t, env_vars);
        }
    }
}

impl HostEnv {
    /// Creates a new `HostEnv` from a list of variable specification strings.
    ///
    /// # Errors
    /// Returns an error if any specification string is invalid.
    pub fn new(vars: &[String]) -> Result<Self, String> {
        parse_var_specs(vars).map(|vars| Self { vars })
    }

    /// Returns the middleware function for injecting host environment variables.
    #[must_use]
    pub fn middleware(&self) -> TaskMiddlewareFunc {
        let vars = self.vars.clone();
        Arc::new(move |next: TaskHandlerFunc| {
            let next = next.clone();
            let vars = vars.clone();
            Arc::new(move |ctx: Arc<()>, et: TaskEventType, task: &mut Task| {
                if et == TaskEventType::StateChange && task.state == TASK_STATE_RUNNING {
                    let env_vars = get_existing_env_vars(&vars);
                    apply_host_vars_recursive(task, &env_vars);
                }
                next(ctx, et, task)
            })
        })
    }
}

/// Creates a hostenv middleware from configuration, if any variables are defined.
#[must_use]
pub fn create_hostenv_middleware_from_config() -> Option<TaskMiddlewareFunc> {
    let vars = config::strings_default("middleware.task.hostenv.vars", &[]);
    if vars.is_empty() {
        return None;
    }
    HostEnv::new(&vars)
        .map(|hostenv| hostenv.middleware())
        .inspect_err(|e| tracing::warn!("invalid hostenv config: {}", e))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::expect_used)]
    #[test]
    fn parse_var_spec_single_host_var() {
        let result = parse_var_spec("HOME").expect("parse should succeed");
        assert_eq!(result, ("HOME".to_string(), "HOME".to_string()));
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn parse_var_spec_with_task_mapping() {
        let result = parse_var_spec("HOST_PATH:PATH").expect("parse should succeed");
        assert_eq!(result, ("HOST_PATH".to_string(), "PATH".to_string()));
    }

    #[test]
    fn parse_var_spec_invalid_too_many_colons() {
        let result = parse_var_spec("a:b:c");
        assert!(result.is_err());
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn parse_var_specs_multiple() {
        let specs = vec!["HOME".to_string(), "HOST_PATH:PATH".to_string()];
        let result = parse_var_specs(&specs).expect("parse should succeed");
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("HOME"), Some(&"HOME".to_string()));
        assert_eq!(result.get("HOST_PATH"), Some(&"PATH".to_string()));
    }
}
