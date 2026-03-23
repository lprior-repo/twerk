// =============================================================================

/// Creates a new worker with the given broker and optional runtime.
///
/// Go parity: `func (e *Engine) initWorker() error`
///
/// When no runtime is provided, one is created from environment configuration.
/// Host environment middleware is registered from `TORK_MIDDLEWARE_TASK_HOSTENV_VARS`.
pub async fn create_worker(
    engine: &mut crate::engine::Engine,
    _broker: BrokerProxy,
    runtime: Option<Box<dyn RuntimeTrait + Send + Sync>>,
) -> Result<Box<dyn Worker + Send + Sync>> {
    let config = read_runtime_config();

    // Initialize runtime if not provided
    // Go: rt, err := e.initRuntime()
    let _rt = match runtime {
        Some(r) => r,
        None => create_runtime_from_config(&config).await?,
    };

    debug!("Worker runtime initialized: {:?}", config.runtime_type);

    // Create and register hostenv middleware
    // Go: hostenv, err := task.NewHostEnv(conf.Strings("middleware.task.hostenv.vars")...)
    //     e.cfg.Middleware.Task = append(e.cfg.Middleware.Task, hostenv.Execute)
    if let Some(hostenv_mw) = create_hostenv_middleware(&config.hostenv_vars) {
        engine.register_task_middleware(hostenv_mw);
        debug!(
            "Registered hostenv middleware for vars: {:?}",
            config.hostenv_vars
        );
    }

    // Read worker configuration from environment
    // Go: conf.StringDefault("worker.name", "Worker")
    let _name = std::env::var("TORK_WORKER_NAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Worker".to_string());
    let _address = std::env::var("TORK_WORKER_ADDRESS").ok().filter(|s| !s.is_empty());

    // Parse queues from environment
    // Go: conf.IntMap("worker.queues")
    let _queues: HashMap<String, i32> = std::env::var("TORK_WORKER_QUEUES")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|q| {
                    let parts: Vec<&str> = q.split(':').collect();
                    if parts.len() == 2 {
                        parts[1]
                            .trim()
                            .parse::<i32>()
                            .ok()
                            .map(|v| (parts[0].trim().to_string(), v))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_else(|| {
            let mut m = HashMap::new();
            m.insert("default".to_string(), 1);
            m
        });

    // Get default limits from environment using Limits struct
    // Go parity: reads conf.String("worker.limits.cpus"), conf.String("worker.limits.memory"), conf.String("worker.limits.timeout")
    let limits = read_limits();
    debug!(
        "Worker limits: cpus={}, memory={}, timeout={}",
        limits.cpus, limits.memory, limits.timeout
    );

    // Return a placeholder worker
    // Full implementation would create a real Worker from tork_runtime
    Ok(Box::new(NoOpWorker) as Box<dyn Worker + Send + Sync>)
}

// =============================================================================
// Tests