//! Configuration module for loading TOML config with environment variable overrides.
//!
//! Loads TOML config from default paths, with environment variable overrides.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![allow(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

pub mod env;
pub mod lookup;
pub mod parsing;
pub mod types;

// Re-export public API
pub use env::extract_env_vars;
pub use lookup::{
    bool, bool_default, bool_map, duration_default, int, int_default, int_map, string,
    string_default, string_map, strings, strings_default, unmarshal, worker_limits,
    broker_rabbitmq_consumer_timeout, broker_rabbitmq_durable_queues, broker_rabbitmq_queue_type,
    mounts_bind_allowed, mounts_bind_sources, mounts_temp_dir, runtime_docker_privileged,
    runtime_docker_image_ttl, runtime_podman_host_network, runtime_podman_privileged,
    middleware_web_logger_enabled, middleware_web_logger_level, middleware_web_logger_skip_paths,
};
pub use parsing::{load_config, parse_toml_file};
pub use types::{ConfigError, ConfigState, TomlValue, WorkerLimits};
