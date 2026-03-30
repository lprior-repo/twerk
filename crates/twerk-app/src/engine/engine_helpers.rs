//! Twerk Engine - Helper functions and types

use twerk_core::task::Task;
use twerk_infrastructure::runtime::Runtime;
use twerk_infrastructure::runtime::{BoxedFuture, ShutdownResult};

/// Resolves the locker type from environment variables.
///
/// Matches Go `initLocker()`:
/// - Reads `TWERK_LOCKER_TYPE`, falls back to `TWERK_DATASTORE_TYPE`,
///   falls back to `"inmemory"`.
pub fn resolve_locker_type() -> String {
    let from_env = match std::env::var("TWERK_LOCKER_TYPE") {
        Ok(s) if !s.is_empty() => Some(s),
        _ => None,
    };
    match from_env {
        Some(t) => t,
        None => match std::env::var("TWERK_DATASTORE_TYPE") {
            Ok(s) if !s.is_empty() => s,
            _ => "inmemory".to_string(),
        },
    }
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
