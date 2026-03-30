//! Twerk Engine - Helper functions and types

use twerk_common::load_config;
use twerk_core::task::Task;
use twerk_infrastructure::runtime::Runtime;
use twerk_infrastructure::runtime::{BoxedFuture, ShutdownResult};

fn config_string(key: &str) -> Option<String> {
    let _ = load_config();
    match twerk_infrastructure::config::string(key) {
        value if value.is_empty() => None,
        value => Some(value),
    }
}

fn env_string(key: &str) -> Option<String> {
    std::env::var(format!("TWERK_{}", key.to_uppercase().replace('.', "_")))
        .ok()
        .filter(|value| !value.is_empty())
}

fn env_or_config_string(key: &str) -> Option<String> {
    env_string(key).or_else(|| config_string(key))
}

pub fn ensure_config_loaded() {
    let _ = load_config();
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
