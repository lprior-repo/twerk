//! Twerk Engine - Lifecycle operations (start, run, terminate, await_shutdown)
//! and mode-specific implementations (run_coordinator, run_worker, run_standalone)

use super::engine_helpers::ensure_config_loaded;
use super::signals::await_signal_or_channel;
use super::state::{Mode, State};
use anyhow::Result;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, instrument};

// ── Typed engine lifecycle errors ──────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub(crate) enum EngineError {
    #[error("engine is not idle")]
    NotIdle,
    #[error("engine is not running")]
    NotRunning,
}

/// Spawns a signal handler task that listens for SIGINT, SIGTERM, or a broadcast
/// termination signal, then runs the provided cleanup future.
fn spawn_signal_handler<F>(
    broadcaster: Arc<broadcast::Sender<()>>,
    cleanup: F,
) -> tokio::task::JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .map_or_else(
                |e| {
                    error!("Failed to register SIGINT: {e}");
                    None
                },
                Some,
            );
        let sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_or_else(
                |e| {
                    error!("Failed to register SIGTERM: {e}");
                    None
                },
                Some,
            );

        await_signal_or_channel(sigint, sigterm, broadcaster.subscribe()).await;

        debug!("shutting down");
        cleanup.await;
    })
}

impl super::Engine {
    /// Starts the engine in the configured mode
    #[instrument(skip_all)]
    pub async fn start(&mut self) -> Result<()> {
        if self.state != State::Idle {
            return Err(EngineError::NotIdle.into());
        }

        ensure_config_loaded();

        match self.mode {
            Mode::Coordinator => self.run_coordinator().await,
            Mode::Worker => self.run_worker().await,
            Mode::Standalone => self.run_standalone().await,
        }?;

        self.state = State::Running;
        Ok(())
    }

    /// Runs the engine and waits for termination
    #[instrument(skip_all)]
    pub async fn run(&mut self) -> Result<()> {
        self.start().await?;
        self.await_shutdown().await;
        Ok(())
    }

    /// Terminates the engine
    #[instrument(skip_all)]
    pub async fn terminate(&mut self) -> Result<()> {
        if self.state != State::Running {
            return Err(EngineError::NotRunning.into());
        }

        self.state = State::Terminating;
        debug!("Terminating engine");

        // Signal termination via terminate_broadcaster (which the signal handlers listen on)
        if let Err(e) = self.terminate_tx.send(()) {
            debug!("Termination broadcast failed (no listeners): {}", e);
        }

        // Stop worker if present
        {
            let worker = self.worker.read().await;
            if let Some(w) = worker.as_ref() {
                if let Err(e) = w.stop().await {
                    error!("error stopping worker: {}", e);
                }
            }
        }

        // Stop coordinator if present
        {
            let coordinator = self.coordinator.read().await;
            if let Some(c) = coordinator.as_ref() {
                if let Err(e) = c.stop().await {
                    error!("error stopping coordinator: {}", e);
                }
            }
        }

        // Signal termination completion via Notify - properly wakes all waiters
        self.terminated_notify.notify_waiters();
        self.state = State::Terminated;
        Ok(())
    }

    /// Wait for shutdown to complete.
    /// Uses Notify which properly wakes late waiters - no more missed signals.
    pub async fn await_shutdown(&self) {
        // notified() waits for notify() to be called.
        // If notify() was already called, this returns immediately.
        self.terminated_notify.notified().await;
    }

    /// Shuts down the engine and cancels all queued work.
    ///
    /// Unlike `terminate()` which waits for running tasks to complete,
    /// `shutdown()` cancels queued (pending) tasks immediately.
    #[instrument(skip_all)]
    pub async fn shutdown(&mut self) -> Result<()> {
        if self.state != State::Running && self.state != State::Terminating {
            return Err(EngineError::NotRunning.into());
        }

        self.state = State::Terminating;
        debug!("Shutting down engine - cancelling queued work");

        // Signal termination via terminate_broadcaster
        if let Err(e) = self.terminate_tx.send(()) {
            debug!("Termination broadcast failed (no listeners): {}", e);
        }

        // Clear submitted tasks - these are queued but not yet processed
        self.submitted_tasks.clear();

        // Stop worker if present
        {
            let worker = self.worker.read().await;
            if let Some(w) = worker.as_ref() {
                if let Err(e) = w.stop().await {
                    error!("error stopping worker: {}", e);
                }
            }
        }

        // Stop coordinator if present
        {
            let coordinator = self.coordinator.read().await;
            if let Some(c) = coordinator.as_ref() {
                if let Err(e) = c.stop().await {
                    error!("error stopping coordinator: {}", e);
                }
            }
        }

        // Signal termination completion
        self.terminated_notify.notify_waiters();
        self.state = State::Terminated;
        Ok(())
    }

    #[instrument(skip_all)]
    async fn run_coordinator(&mut self) -> Result<()> {
        self.broker
            .init(
                &super::engine_helpers::resolve_broker_type(),
                self.engine_id.as_deref(),
            )
            .await?;
        self.datastore.init().await?;

        // Create locker — resolve type from env (locker.type → datastore.type → inmemory)
        let locker_type = super::engine_helpers::resolve_locker_type();
        let locker = super::locker::create_locker(&locker_type).await?;
        *self.locker.write().await = Some(locker);

        let coord =
            super::coordinator::create_coordinator(self.broker.clone(), self.datastore.clone())
                .await?;
        coord.start().await?;
        *self.coordinator.write().await = Some(coord);

        let coordinator = self.coordinator.clone();
        let broadcaster = self.terminate_broadcaster.clone();

        spawn_signal_handler(broadcaster, async move {
            let coord = coordinator.read().await;
            if let Some(c) = coord.as_ref() {
                if let Err(e) = c.stop().await {
                    error!("error stopping coordinator: {}", e);
                }
            }
        });

        Ok(())
    }

    #[instrument(skip_all)]
    async fn run_worker(&mut self) -> Result<()> {
        self.broker
            .init(
                &super::engine_helpers::resolve_broker_type(),
                self.engine_id.as_deref(),
            )
            .await?;

        // Take the runtime out, will be replaced after worker creation
        let runtime = self.runtime.take();
        let worker = super::worker::create_worker(self, self.broker.clone(), runtime).await?;
        worker.start().await?;
        *self.worker.write().await = Some(worker);

        let worker_ref = self.worker.clone();
        let broadcaster = self.terminate_broadcaster.clone();

        spawn_signal_handler(broadcaster, async move {
            let w = worker_ref.read().await;
            if let Some(ref worker) = *w {
                if let Err(e) = worker.stop().await {
                    error!("error stopping worker: {}", e);
                }
            }
        });

        Ok(())
    }

    #[instrument(skip_all)]
    async fn run_standalone(&mut self) -> Result<()> {
        self.broker
            .init(
                &super::engine_helpers::resolve_broker_type(),
                self.engine_id.as_deref(),
            )
            .await?;
        self.datastore.init().await?;

        // Create locker — resolve type from env (locker.type → datastore.type → inmemory)
        let locker_type = super::engine_helpers::resolve_locker_type();
        let locker = super::locker::create_locker(&locker_type).await?;
        *self.locker.write().await = Some(locker);

        // Take the runtime out, will be replaced after worker creation
        let runtime = self.runtime.take();
        let worker = super::worker::create_worker(self, self.broker.clone(), runtime).await?;
        worker.start().await?;
        *self.worker.write().await = Some(worker);

        let coord =
            super::coordinator::create_coordinator(self.broker.clone(), self.datastore.clone())
                .await?;
        coord.start().await?;
        *self.coordinator.write().await = Some(coord);

        let worker_ref = self.worker.clone();
        let coordinator = self.coordinator.clone();
        let broadcaster = self.terminate_broadcaster.clone();

        spawn_signal_handler(broadcaster, async move {
            let w = worker_ref.read().await;
            if let Some(ref worker) = *w {
                if let Err(e) = worker.stop().await {
                    error!("error stopping worker: {}", e);
                }
            }

            let c = coordinator.read().await;
            if let Some(ref coord) = *c {
                if let Err(e) = coord.stop().await {
                    error!("error stopping coordinator: {}", e);
                }
            }
        });

        Ok(())
    }
}
