//! Host environment middleware.
//!
//! This middleware injects host environment variables into tasks
//! when they transition to the RUNNING state.
//!
//! Full parity with Go `middleware/task/hostenv.go`.

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
///
/// Go parity: `type HostEnv struct { vars map[string]string }`
#[derive(Debug, Clone)]
pub struct HostEnv {
    /// Mapping of host variable names to task environment variable names.
    vars: HashMap<String, String>,
}

impl HostEnv {
    /// Creates a new HostEnv from variable specifications.
    ///
    /// Each spec can be either:
    /// - `"VAR_NAME"` — reads VAR_NAME and stores as VAR_NAME
    /// - `"HOST_VAR:TASK_VAR"` — reads HOST_VAR and stores as TASK_VAR
    ///
    /// Go parity: `func NewHostEnv(vars ...string) (*HostEnv, error)`
    pub fn new(vars: &[&str]) -> Result<Self, HostEnvError> {
        let vars_map = vars
            .iter()
            .map(|var_spec| {
                let parts: Vec<&str> = var_spec.split(':').collect();
                match parts.len() {
                    1 => {
                        let var_name = parts[0];
                        if var_name.is_empty() {
                            return Err(HostEnvError::InvalidSpec(var_spec.to_string()));
                        }
                        Ok((var_name.to_string(), var_name.to_string()))
                    }
                    2 => {
                        let host_var = parts[0];
                        let task_var = parts[1];
                        if host_var.is_empty() || task_var.is_empty() {
                            return Err(HostEnvError::InvalidSpec(var_spec.to_string()));
                        }
                        Ok((host_var.to_string(), task_var.to_string()))
                    }
                    _ => Err(HostEnvError::InvalidSpec(var_spec.to_string())),
                }
            })
            .collect::<Result<HashMap<String, String>, HostEnvError>>()?;

        Ok(Self { vars: vars_map })
    }

    /// Returns a middleware function that wraps the given handler.
    ///
    /// Go parity: `func (m *HostEnv) Execute(next HandlerFunc) HandlerFunc`
    #[must_use]
    pub fn middleware(&self) -> MiddlewareFunc {
        let vars = self.vars.clone();
        Arc::new(move |next: HandlerFunc| -> HandlerFunc {
            let vars = vars.clone();
            Arc::new(move |ctx: Context, et: EventType, task: &mut Task| {
                // Set host vars when task transitions to RUNNING state
                if et == EventType::StateChange && task.state == TASK_STATE_RUNNING {
                    set_host_vars(task, &vars);
                }

                next(ctx, et, task)
            })
        })
    }
}

/// Sets host environment variables in a task and all its subtasks.
///
/// Go parity: `func (m *HostEnv) setHostVars(t *tork.Task)`
///
/// Recursively processes pre and post tasks via mutable iteration,
/// ensuring modifications persist to the parent task.
fn set_host_vars(task: &mut Task, vars: &HashMap<String, String>) {
    // Initialize env map if needed
    if task.env.is_none() {
        task.env = Some(HashMap::new());
    }

    // Set each host variable
    if let Some(ref mut env) = task.env {
        for (host_name, task_name) in vars {
            if let Ok(value) = env::var(host_name) {
                env.insert(task_name.clone(), value);
            }
        }
    }

    // Recursively set host vars in pre tasks (FIX: iter_mut, not clone+discard)
    if let Some(ref mut pre) = task.pre {
        for pre_task in pre.iter_mut() {
            set_host_vars(pre_task, vars);
        }
    }

    // Recursively set host vars in post tasks (FIX: iter_mut, not clone+discard)
    if let Some(ref mut post) = task.post {
        for post_task in post.iter_mut() {
            set_host_vars(post_task, vars);
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

    #[test]
    fn test_new_host_env_empty_parts() {
        assert!(HostEnv::new(&[""]).is_err());
        assert!(HostEnv::new(&[":ALIAS"]).is_err());
        assert!(HostEnv::new(&["HOST:"]).is_err());
    }

    #[test]
    fn test_set_host_vars_simple() {
        env::set_var("TEST_HOSTENV_VAR1", "value1");
        let mut task = Task {
            state: TASK_STATE_RUNNING,
            ..Default::default()
        };
        let vars = HashMap::from([(
            "TEST_HOSTENV_VAR1".to_string(),
            "TEST_HOSTENV_VAR1".to_string(),
        )]);
        set_host_vars(&mut task, &vars);
        assert_eq!(
            task.env.as_ref().and_then(|e| e.get("TEST_HOSTENV_VAR1")),
            Some(&"value1".to_string())
        );
        env::remove_var("TEST_HOSTENV_VAR1");
    }

    #[test]
    fn test_set_host_vars_preserves_existing_env() {
        env::set_var("TEST_HOSTENV_VAR2", "value2");
        let mut task = Task {
            state: TASK_STATE_RUNNING,
            env: Some(HashMap::from([(
                "OTHER_VAR".to_string(),
                "othervalue".to_string(),
            )])),
            ..Default::default()
        };
        let vars = HashMap::from([("TEST_HOSTENV_VAR2".to_string(), "VAR2".to_string())]);
        set_host_vars(&mut task, &vars);
        let env = task.env.as_ref().expect("env should exist");
        assert_eq!(env.get("VAR2"), Some(&"value2".to_string()));
        assert_eq!(env.get("OTHER_VAR"), Some(&"othervalue".to_string()));
        env::remove_var("TEST_HOSTENV_VAR2");
    }

    #[test]
    fn test_set_host_vars_with_alias() {
        env::set_var("TEST_HOST_ALIAS_SRC", "aliased_value");
        let mut task = Task {
            state: TASK_STATE_RUNNING,
            ..Default::default()
        };
        let vars = HashMap::from([("TEST_HOST_ALIAS_SRC".to_string(), "DEST_VAR".to_string())]);
        set_host_vars(&mut task, &vars);
        assert_eq!(
            task.env.as_ref().and_then(|e| e.get("DEST_VAR")),
            Some(&"aliased_value".to_string())
        );
        env::remove_var("TEST_HOST_ALIAS_SRC");
    }

    #[test]
    fn test_set_host_vars_recurses_into_pre_tasks() {
        env::set_var("TEST_HOSTENV_PRE", "pre_value");
        let mut task = Task {
            state: TASK_STATE_RUNNING,
            pre: Some(vec![Task {
                name: Some("some pre task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        let vars = HashMap::from([("TEST_HOSTENV_PRE".to_string(), "PRE_VAR".to_string())]);
        set_host_vars(&mut task, &vars);

        // Parent task should have the var
        assert_eq!(
            task.env.as_ref().and_then(|e| e.get("PRE_VAR")),
            Some(&"pre_value".to_string())
        );

        // Pre task should also have the var (the bug fix)
        let pre_env = task.pre.as_ref().expect("pre")[0]
            .env
            .as_ref()
            .expect("pre task env");
        assert_eq!(pre_env.get("PRE_VAR"), Some(&"pre_value".to_string()));
        env::remove_var("TEST_HOSTENV_PRE");
    }

    #[test]
    fn test_set_host_vars_recurses_into_post_tasks() {
        env::set_var("TEST_HOSTENV_POST", "post_value");
        let mut task = Task {
            state: TASK_STATE_RUNNING,
            post: Some(vec![Task {
                name: Some("some post task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        let vars = HashMap::from([("TEST_HOSTENV_POST".to_string(), "POST_VAR".to_string())]);
        set_host_vars(&mut task, &vars);

        // Parent task should have the var
        assert_eq!(
            task.env.as_ref().and_then(|e| e.get("POST_VAR")),
            Some(&"post_value".to_string())
        );

        // Post task should also have the var (the bug fix)
        let post_env = task.post.as_ref().expect("post")[0]
            .env
            .as_ref()
            .expect("post task env");
        assert_eq!(post_env.get("POST_VAR"), Some(&"post_value".to_string()));
        env::remove_var("TEST_HOSTENV_POST");
    }

    #[test]
    fn test_set_host_vars_pre_and_post_together() {
        env::set_var("TEST_HOSTENV_BOTH", "both_value");
        let mut task = Task {
            state: TASK_STATE_RUNNING,
            env: Some(HashMap::from([(
                "OTHER_VAR".to_string(),
                "othervalue".to_string(),
            )])),
            pre: Some(vec![Task {
                name: Some("some pre task".to_string()),
                ..Default::default()
            }]),
            post: Some(vec![Task {
                name: Some("some post task".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        let vars = HashMap::from([("TEST_HOSTENV_BOTH".to_string(), "VAR5".to_string())]);
        set_host_vars(&mut task, &vars);

        let env = task.env.as_ref().expect("env");
        assert_eq!(env.get("VAR5"), Some(&"both_value".to_string()));
        assert_eq!(env.get("OTHER_VAR"), Some(&"othervalue".to_string()));

        // Pre task redacted (Go parity: TestHostEnv5)
        let pre_env = task.pre.as_ref().expect("pre")[0]
            .env
            .as_ref()
            .expect("pre task env");
        assert_eq!(pre_env.get("VAR5"), Some(&"both_value".to_string()));

        // Post task redacted (Go parity: TestHostEnv5)
        let post_env = task.post.as_ref().expect("post")[0]
            .env
            .as_ref()
            .expect("post task env");
        assert_eq!(post_env.get("VAR5"), Some(&"both_value".to_string()));

        env::remove_var("TEST_HOSTENV_BOTH");
    }
}
