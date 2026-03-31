//! Twerk Engine - Lifecycle operations (start, run, terminate, await_shutdown)
//! and mode-specific implementations (run_coordinator, run_worker, run_standalone)

use super::engine_helpers::ensure_config_loaded;
use super::signals::await_signal_or_channel;
use super::state::{Mode, State};
use anyhow::Result;
use tokio::sync::broadcast;
use tracing::{debug, error};

impl super::Engine {
    /// Starts the engine in the configured mode
    pub async fn start(&mut self) -> Result<()> {
        if self.state != State::Idle {
            anyhow::bail!("engine is not idle");
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
    pub async fn run(&mut self) -> Result<()> {
        self.start().await?;
        self.await_shutdown().await;
        Ok(())
    }

    /// Terminates the engine
    pub async fn terminate(&mut self) -> Result<()> {
        if self.state != State::Running {
            anyhow::bail!("engine is not running");
        }

        self.state = State::Terminating;
        debug!("Terminating engine");

        // Signal termination
        if let Err(e) = self.terminate_tx.send(()) {
            debug!("Termination broadcast failed (no listeners): {}", e);
        }

        // Stop worker if present
        {
            let worker = self.worker.write().await;
            if let Some(w) = worker.as_ref() {
                if let Err(e) = w.stop().await {
                    error!("error stopping worker: {}", e);
                }
            }
        }

        // Stop coordinator if present
        {
            let coordinator = self.coordinator.write().await;
            if let Some(c) = coordinator.as_ref() {
                if let Err(e) = c.stop().await {
                    error!("error stopping coordinator: {}", e);
                }
            }
        }

        self.state = State::Terminated;
        Ok(())
    }

    /// Wait for shutdown to complete
    pub async fn await_shutdown(&self) {
        // Get the terminated receiver if available
        let terminated_rx = {
            let terminated_tx = self.terminated_tx.read().await;
            terminated_tx.as_ref().map(|tx| tx.subscribe())
        };

        if let Some(mut rx) = terminated_rx {
            let _ = rx.recv().await;
        }
    }

    async fn run_coordinator(&mut self) -> Result<()> {
        self.broker
            .init(
                &super::engine_helpers::resolve_broker_type(),
                Some(&self.engine_id),
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

        // Set up termination channels
        let (terminated_tx, _terminated_rx) = broadcast::channel::<()>(1);
        *self.terminated_tx.write().await = Some(terminated_tx);

        // Clone references for the signal handler task
        let coordinator = self.coordinator.clone();
        let terminated_tx_clone = self.terminated_tx.clone();

        // Spawn signal handler task
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
            let terminate_rx = {
                let terminated_tx = terminated_tx_clone.read().await;
                match terminated_tx.as_ref() {
                    Some(tx) => tx.subscribe(),
                    None => return,
                }
            };

            await_signal_or_channel(sigint, sigterm, terminate_rx).await;

            debug!("shutting down");

            // Stop coordinator if present
            let coord = coordinator.read().await;
            if let Some(c) = coord.as_ref() {
                if let Err(e) = c.stop().await {
                    error!("error stopping coordinator: {}", e);
                }
            }

            // Signal terminated
            let tx = terminated_tx_clone.write().await;
            if let Some(ref t) = *tx {
                if let Err(e) = t.send(()) {
                    error!("failed to broadcast termination: {e}");
                }
            }
        });

        Ok(())
    }

    async fn run_worker(&mut self) -> Result<()> {
        self.broker
            .init(
                &super::engine_helpers::resolve_broker_type(),
                Some(&self.engine_id),
            )
            .await?;

        // Take the runtime out, will be replaced after worker creation
        let runtime = self.runtime.take();
        let worker = super::worker::create_worker(self, self.broker.clone(), runtime).await?;
        worker.start().await?;
        *self.worker.write().await = Some(worker);

        // Set up termination channels
        let (terminated_tx, _terminated_rx) = broadcast::channel::<()>(1);
        *self.terminated_tx.write().await = Some(terminated_tx);

        // Clone references for the signal handler task
        let worker_ref = self.worker.clone();
        let terminated_tx_clone = self.terminated_tx.clone();

        // Spawn signal handler task
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
            let terminate_rx = {
                let terminated_tx = terminated_tx_clone.read().await;
                match terminated_tx.as_ref() {
                    Some(tx) => tx.subscribe(),
                    None => return,
                }
            };

            await_signal_or_channel(sigint, sigterm, terminate_rx).await;

            debug!("shutting down");

            // Stop worker if present
            let w = worker_ref.read().await;
            if let Some(ref worker) = *w {
                if let Err(e) = worker.stop().await {
                    error!("error stopping worker: {}", e);
                }
            }

            // Signal terminated
            let tx = terminated_tx_clone.write().await;
            if let Some(ref t) = *tx {
                if let Err(e) = t.send(()) {
                    error!("failed to broadcast termination: {e}");
                }
            }
        });

        Ok(())
    }

    async fn run_standalone(&mut self) -> Result<()> {
        self.broker
            .init(
                &super::engine_helpers::resolve_broker_type(),
                Some(&self.engine_id),
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

        // Set up termination channels
        let (terminated_tx, _terminated_rx) = broadcast::channel::<()>(1);
        *self.terminated_tx.write().await = Some(terminated_tx);

        // Clone references for the signal handler task
        let worker_ref = self.worker.clone();
        let coordinator = self.coordinator.clone();
        let terminated_tx_clone = self.terminated_tx.clone();

        // Spawn signal handler task
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
            let terminate_rx = {
                let terminated_tx = terminated_tx_clone.read().await;
                match terminated_tx.as_ref() {
                    Some(tx) => tx.subscribe(),
                    None => return,
                }
            };

            await_signal_or_channel(sigint, sigterm, terminate_rx).await;

            debug!("shutting down");

            // Stop worker if present
            let w = worker_ref.read().await;
            if let Some(ref worker) = *w {
                if let Err(e) = worker.stop().await {
                    error!("error stopping worker: {}", e);
                }
            }

            // Stop coordinator if present
            let c = coordinator.read().await;
            if let Some(ref coord) = *c {
                if let Err(e) = coord.stop().await {
                    error!("error stopping coordinator: {}", e);
                }
            }

            // Signal terminated
            let tx = terminated_tx_clone.write().await;
            if let Some(ref t) = *tx {
                if let Err(e) = t.send(()) {
                    error!("failed to broadcast termination: {e}");
                }
            }
        });

        Ok(())
    }
}
