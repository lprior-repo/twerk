//! Default engine module
//!
//! This module provides package-level convenience functions that
//! operate on a default engine singleton, matching Go's `engine/default.go`.
//!
//! All functions delegate to the global `DEFAULT_ENGINE` behind a
//! `tokio::sync::RwLock`, ensuring safe concurrent access.

#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

use crate::engine::{Engine, JobListener, Mode};
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

use tork::broker::Broker;
use tork::datastore::Datastore;
use tork::runtime::Runtime;

/// The default engine instance, matching Go's `var defaultEngine *Engine = New(Config{})`.
static DEFAULT_ENGINE: Lazy<RwLock<Engine>> = Lazy::new(|| RwLock::new(Engine::default()));

/// Register web middleware on the default engine.
///
/// Go parity: `func RegisterWebMiddleware(mw web.MiddlewareFunc)`
pub async fn register_web_middleware(mw: crate::engine::WebMiddlewareFunc) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_web_middleware(mw);
}

/// Register task middleware on the default engine.
///
/// Go parity: `func RegisterTaskMiddleware(mw task.MiddlewareFunc)`
pub async fn register_task_middleware(mw: crate::engine::TaskMiddlewareFunc) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_task_middleware(mw);
}

/// Register job middleware on the default engine.
///
/// Go parity: `func RegisterJobMiddleware(mw job.MiddlewareFunc)`
pub async fn register_job_middleware(mw: crate::engine::JobMiddlewareFunc) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_job_middleware(mw);
}

/// Register node middleware on the default engine.
///
/// Go parity: `func RegisterNodeMiddleware(mw node.MiddlewareFunc)`
pub async fn register_node_middleware(mw: crate::engine::NodeMiddlewareFunc) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_node_middleware(mw);
}

/// Register log middleware on the default engine.
///
/// Go parity: `func RegisterLogMiddleware(mw logmw.MiddlewareFunc)`
pub async fn register_log_middleware(mw: crate::engine::LogMiddlewareFunc) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_log_middleware(mw);
}

/// Register a mounter for a specific runtime type.
///
/// Go parity: `func RegisterMounter(runtime, name string, mounter runtime.Mounter)`
#[allow(dead_code)]
pub async fn register_mounter(rt: &str, name: &str, mounter: Box<dyn tork::runtime::mount::Mounter>) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_mounter(rt, name, mounter);
}

/// Register a runtime on the default engine.
///
/// Go parity: `func RegisterRuntime(rt runtime.Runtime)`
pub async fn register_runtime(rt: Box<dyn Runtime + Send + Sync>) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_runtime(rt);
}

/// Register a datastore provider by name.
///
/// Go parity: `func RegisterDatastoreProvider(name string, provider datastore.Provider)`
pub async fn register_datastore_provider(
    name: &str,
    provider: Box<dyn Datastore + Send + Sync>,
) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_datastore_provider(name, provider);
}

/// Register a broker provider by name.
///
/// Go parity: `func RegisterBrokerProvider(name string, provider broker.Provider)`
pub async fn register_broker_provider(name: &str, provider: Box<dyn Broker + Send + Sync>) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_broker_provider(name, provider);
}

/// Register an API endpoint on the default engine.
///
/// Go parity: `func RegisterEndpoint(method, path string, handler web.HandlerFunc)`
pub async fn register_endpoint(method: &str, path: &str, handler: crate::engine::EndpointHandler) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.register_endpoint(method, path, handler);
}

/// Submit a job to the default engine.
///
/// Go parity: `func SubmitJob(ctx context.Context, ij *input.Job, listeners ...JobListener) (*tork.Job, error)`
pub async fn submit_job(
    job: tork::job::Job,
    listeners: Vec<JobListener>,
) -> anyhow::Result<tork::job::Job> {
    let engine = DEFAULT_ENGINE.read().await;
    engine.submit_job(job, listeners).await
}

/// Returns the broker from the default engine.
///
/// Go parity: `func Broker() broker.Broker`
pub async fn broker() -> crate::broker::BrokerProxy {
    let engine = DEFAULT_ENGINE.read().await;
    engine.broker_proxy()
}

/// Returns the datastore from the default engine.
///
/// Go parity: `func Datastore() datastore.Datastore`
pub async fn datastore() -> crate::datastore::DatastoreProxy {
    let engine = DEFAULT_ENGINE.read().await;
    engine.datastore_proxy()
}

/// Sets the engine mode on the default engine.
///
/// Go parity: `func SetMode(mode Mode)`
pub async fn set_mode(mode: Mode) {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.set_mode(mode);
}

/// Returns the current engine state.
///
/// Go parity: equivalent to checking engine state
pub async fn state() -> crate::engine::State {
    DEFAULT_ENGINE.read().await.state()
}

/// Starts the default engine.
///
/// Go parity: `func Start() error`
pub async fn start() -> anyhow::Result<()> {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.start().await
}

/// Terminates the default engine.
///
/// Go parity: `func Terminate() error`
pub async fn terminate() -> anyhow::Result<()> {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.terminate().await
}

/// Runs the default engine and waits for termination.
///
/// Go parity: `func Run() error`
pub async fn run() -> anyhow::Result<()> {
    let mut engine = DEFAULT_ENGINE.write().await;
    engine.run().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::State;

    /// Go parity: `TestDefaultRunStandalone`
    ///
    /// Sets the mode to Standalone on the global default engine,
    /// starts it, checks state transitions, then terminates.
    ///
    /// **Note**: This is the only test that exercises the full lifecycle on
    /// the global singleton because `DEFAULT_ENGINE` is `Lazy` and shared
    /// across all tests. After this test, the engine is in `Terminated`
    /// state and cannot be restarted. All other default-module behaviour
    /// (middleware registration, provider registration, mode changes,
    /// double-start guards) is covered by the `engine::tests` module in
    /// `lib.rs` which creates fresh `Engine` instances per test.
    #[tokio::test]
    async fn test_default_run_standalone() -> Result<(), Box<dyn std::error::Error>> {
        std::env::set_var("TORK_DATASTORE_TYPE", "inmemory");

        set_mode(Mode::Standalone).await;
        assert_eq!(state().await, State::Idle);

        start().await?;
        assert_eq!(state().await, State::Running);

        terminate().await?;
        std::env::set_var("TORK_DATASTORE_TYPE", "inmemory");
        Ok(())
    }
}
