//! Mock runtime implementation for testing.

use std::process::ExitCode;

use twerk_core::task::Task;

use crate::runtime::{BoxedFuture as RuntimeBoxedFuture, Runtime as RuntimeTrait, ShutdownResult};

/// Mock runtime implementation for testing
#[derive(Debug, Clone, Default)]
pub struct MockRuntime;

impl RuntimeTrait for MockRuntime {
    fn run(&self, _task: &Task) -> RuntimeBoxedFuture<Option<String>> {
        Box::pin(async { Ok(None) })
    }
    fn stop(&self, _task: &Task) -> RuntimeBoxedFuture<ShutdownResult<ExitCode>> {
        Box::pin(async { Ok(Ok(ExitCode::SUCCESS)) })
    }
    fn health_check(&self) -> RuntimeBoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}
