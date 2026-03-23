// Host environment middleware
// =============================================================================

/// Create a host environment middleware variable map from variable specs.
///
/// Go parity: `hostenv, err := task.NewHostEnv(conf.Strings("middleware.task.hostenv.vars")...)`
///
/// Parses specs like `"VAR"` or `"HOST_VAR:TASK_VAR"` into a HashMap.
pub fn create_hostenv_middleware(
    vars: &[String],
) -> Option<crate::engine::TaskMiddlewareFunc> {
    if vars.is_empty() {
        return None;
    }

    let var_map: HashMap<String, String> = vars
        .iter()
        .filter_map(|var_spec| {
            let parts: Vec<&str> = var_spec.split(':').collect();
            match parts.len() {
                1 if !parts[0].is_empty() => {
                    Some((parts[0].to_string(), parts[0].to_string()))
                }
                2 if !parts[0].is_empty() && !parts[1].is_empty() => {
                    Some((parts[0].to_string(), parts[1].to_string()))
                }
                _ => {
                    warn!("invalid env var spec: {}", var_spec);
                    None
                }
            }
        })
        .collect();

    if var_map.is_empty() {
        return None;
    }

    // Create the middleware function
    // Go parity: hostenv.Execute
    let middleware: crate::engine::TaskMiddlewareFunc = std::sync::Arc::new(
        move |next: crate::engine::TaskHandlerFunc| -> crate::engine::TaskHandlerFunc {
            let var_map = var_map.clone();
            std::sync::Arc::new(move |_ctx: std::sync::Arc<()>, et: crate::engine::TaskEventType, task: &mut tork::task::Task| {
                if et == crate::engine::TaskEventType::StateChange && task.state == tork::task::TASK_STATE_RUNNING {
                    if task.env.is_none() {
                        task.env = Some(HashMap::new());
                    }
                    if let Some(ref mut env_map) = task.env {
                        for (host_name, task_name) in &var_map {
                            if let Ok(value) = std::env::var(host_name) {
                                env_map.insert(task_name.clone(), value);
                            }
                        }
                    }
                }
                next(_ctx, et, task)
            })
        },
    );

    Some(middleware)
}

// =============================================================================