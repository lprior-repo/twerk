//! Twerk Engine - Helper functions and types

use twerk_common::conf::string as config_string;
use twerk_common::load_config;
use twerk_common::var_with_twerk_prefix;
use twerk_core::task::Task;
use twerk_infrastructure::runtime::Runtime;
use twerk_infrastructure::runtime::{BoxedFuture, ShutdownResult};

fn config_string_opt(key: &str) -> Option<String> {
    let _ = load_config(); // side-effect: ensure config loaded; error irrelevant
    match config_string(key) {
        value if value.is_empty() => None,
        value => Some(value),
    }
}

pub fn env_string(key: &str) -> String {
    var_with_twerk_prefix(key).unwrap_or_else(|| config_string(key))
}

pub fn env_string_default(key: &str, default: &str) -> String {
    let value = env_string(key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

fn env_or_config_string(key: &str) -> Option<String> {
    var_with_twerk_prefix(key).or_else(|| config_string_opt(key))
}

pub fn ensure_config_loaded() {
    let _ = load_config();
}

pub fn resolve_engine_id(config_engine_id: Option<String>) -> Option<String> {
    config_engine_id
        .and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .or_else(|| {
            env_or_config_string("engine.id").and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
        })
}

pub fn resolve_broker_type() -> String {
    env_or_config_string("broker.type").unwrap_or_else(|| "inmemory".to_string())
}

/// Resolves the locker type from environment variables.
///
/// Matches Go `initLocker()`:
/// - Reads `TWERK_LOCKER_TYPE`, falls back to `TWERK_DATASTORE_TYPE`,
///   falls back to `"inmemory"`.
pub fn resolve_locker_type() -> String {
    env_or_config_string("locker.type")
        .or_else(|| env_or_config_string("datastore.type"))
        .unwrap_or_else(|| "inmemory".to_string())
}

/// Mock runtime for testing
#[derive(Debug)]
pub struct MockRuntime;

impl Runtime for MockRuntime {
    fn run(&self, _task: &Task) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn stop(&self, _task: &Task) -> BoxedFuture<ShutdownResult<std::process::ExitCode>> {
        Box::pin(async { Ok(Ok(std::process::ExitCode::SUCCESS)) })
    }

    fn health_check(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}
