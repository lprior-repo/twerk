//! Twerk Engine - Lifecycle operations (start, run, terminate, await_shutdown)
//! and mode-specific implementations (run_coordinator, run_worker, run_standalone)

use super::engine_helpers::ensure_config_loaded;
use super::signals::await_signal_or_channel;
use super::state::{Mode, State};
use anyhow::Result;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, watch};
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
    shutdown_started: Arc<AtomicBool>,
    shutdown_tx: watch::Sender<bool>,
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
        run_shutdown_once(shutdown_started, shutdown_tx, cleanup).await;
    })
}

async fn run_shutdown_once<F>(
    shutdown_started: Arc<AtomicBool>,
    shutdown_tx: watch::Sender<bool>,
    cleanup: F,
) where
    F: Future<Output = ()>,
{
    if shutdown_started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        cleanup.await;
        shutdown_tx.send_replace(true);
    } else {
        await_latched_shutdown(&shutdown_tx).await;
    }
}

async fn await_latched_shutdown(shutdown_tx: &watch::Sender<bool>) {
    let mut shutdown_rx = shutdown_tx.subscribe();
    while !*shutdown_rx.borrow_and_update() {
        if shutdown_rx.changed().await.is_err() {
            return;
        }
    }
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

        self.run_owned_shutdown_cleanup().await;
        signal_shutdown_handlers(&self.terminate_tx);
        self.state = State::Terminated;
        Ok(())
    }

    /// Wait for shutdown to complete.
    /// Uses a latched watch channel so late waiters observe completed shutdown.
    pub async fn await_shutdown(&self) {
        await_latched_shutdown(&self.shutdown_tx).await;
    }

    async fn run_owned_shutdown_cleanup(&self) {
        let worker_ref = self.worker.clone();
        let coordinator = self.coordinator.clone();
        let cleanup = async move {
            stop_worker(worker_ref).await;
            stop_coordinator(coordinator).await;
        };
        run_shutdown_once(
            self.shutdown_started.clone(),
            self.shutdown_tx.clone(),
            cleanup,
        )
        .await;
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
        let shutdown_started = self.shutdown_started.clone();
        let shutdown_tx = self.shutdown_tx.clone();

        spawn_signal_handler(broadcaster, shutdown_started, shutdown_tx, async move {
            stop_coordinator(coordinator).await;
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
        let shutdown_started = self.shutdown_started.clone();
        let shutdown_tx = self.shutdown_tx.clone();

        spawn_signal_handler(broadcaster, shutdown_started, shutdown_tx, async move {
            stop_worker(worker_ref).await;
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
        let shutdown_started = self.shutdown_started.clone();
        let shutdown_tx = self.shutdown_tx.clone();

        spawn_signal_handler(broadcaster, shutdown_started, shutdown_tx, async move {
            stop_worker(worker_ref).await;
            stop_coordinator(coordinator).await;
        });

        Ok(())
    }
}

fn signal_shutdown_handlers(terminate_tx: &broadcast::Sender<()>) {
    if let Err(e) = terminate_tx.send(()) {
        debug!("Termination broadcast failed (no listeners): {}", e);
    }
}

async fn stop_worker(
    worker_ref: Arc<tokio::sync::RwLock<Option<Box<dyn super::worker::Worker + Send + Sync>>>>,
) {
    let worker = worker_ref.read().await;
    if let Some(w) = worker.as_ref() {
        if let Err(e) = w.stop().await {
            error!("error stopping worker: {}", e);
        }
    }
}

async fn stop_coordinator(
    coordinator: Arc<
        tokio::sync::RwLock<Option<Box<dyn super::coordinator::Coordinator + Send + Sync>>>,
    >,
) {
    let coord = coordinator.read().await;
    if let Some(c) = coord.as_ref() {
        if let Err(e) = c.stop().await {
            error!("error stopping coordinator: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[tokio::test]
    async fn shutdown_latch_returns_for_late_waiters_after_completion() {
        let (shutdown_tx, _shutdown_rx) = watch::channel(false);
        shutdown_tx.send_replace(true);

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            await_latched_shutdown(&shutdown_tx),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn shutdown_cleanup_runs_once_when_requested_twice() {
        let (shutdown_tx, _shutdown_rx) = watch::channel(false);
        let shutdown_started = Arc::new(AtomicBool::new(false));
        let cleanup_count = Arc::new(AtomicUsize::new(0));

        run_shutdown_once(shutdown_started.clone(), shutdown_tx.clone(), {
            let cleanup_count = cleanup_count.clone();
            async move {
                cleanup_count.fetch_add(1, Ordering::AcqRel);
            }
        })
        .await;
        run_shutdown_once(shutdown_started, shutdown_tx, {
            let cleanup_count = cleanup_count.clone();
            async move {
                cleanup_count.fetch_add(1, Ordering::AcqRel);
            }
        })
        .await;

        assert_eq!(cleanup_count.load(Ordering::Acquire), 1);
    }
}
