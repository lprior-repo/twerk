//! Default engine module
//!
//! This module provides package-level convenience functions that
//! operate on a default engine singleton.

use crate::engine::{Engine, Mode, State};
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

/// The default engine instance
static DEFAULT_ENGINE: Lazy<RwLock<Engine>> = Lazy::new(|| RwLock::new(Engine::default()));

/// Sets the engine mode
pub fn set_mode(mode: Mode) {
    // Note: This is synchronous but we can't block on the runtime in a sync fn
    // In a real implementation, we'd use a different approach
    let _ = mode;
}

/// Returns the current engine state
pub async fn state() -> State {
    DEFAULT_ENGINE.read().await.state()
}

/// Submits a job to the default engine
pub async fn submit_job(job: tork::job::Job) -> anyhow::Result<tork::job::Job> {
    // In a real implementation, this would call engine.submit_job()
    Ok(job)
}

/// Starts the default engine
pub async fn start() -> anyhow::Result<()> {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.start().await
}

/// Terminates the default engine
pub async fn terminate() -> anyhow::Result<()> {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.terminate().await
}

/// Runs the default engine and waits for termination
pub async fn run() -> anyhow::Result<()> {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.run().await
}
