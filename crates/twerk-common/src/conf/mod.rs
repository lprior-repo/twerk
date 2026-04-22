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

#[cfg(test)]
mod tests;

use std::sync::LazyLock;

pub(crate) static CONFIG: LazyLock<std::sync::RwLock<Option<types::ConfigState>>> =
    LazyLock::new(|| std::sync::RwLock::new(None));

// Re-export public API
pub use env::extract_env_vars;
pub use lookup::{
    bool, bool_default, bool_map, broker_rabbitmq_consumer_timeout, broker_rabbitmq_durable_queues,
    broker_rabbitmq_queue_type, duration_default, int, int_default, int_map,
    middleware_web_logger_enabled, middleware_web_logger_level, middleware_web_logger_skip_paths,
    mounts_bind_allowed, mounts_bind_sources, mounts_temp_dir, runtime_docker_image_ttl,
    runtime_docker_privileged, runtime_podman_host_network, runtime_podman_privileged, string,
    string_default, string_map, strings, strings_default, unmarshal, worker_limits,
};
pub use parsing::{load_config, parse_toml_file};
pub use types::{ConfigError, ConfigState, TomlValue, WorkerLimits};
