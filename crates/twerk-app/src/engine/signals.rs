//! Twerk Engine - Signal handling and shutdown coordination

use tokio::signal::unix::{Signal, SignalKind};
use tokio::sync::broadcast;
use tracing::debug;

/// Signal handler for graceful shutdown
pub struct SignalHandler {
    sigint: Option<Signal>,
    sigterm: Option<Signal>,
    terminate_rx: broadcast::Receiver<()>,
}

impl SignalHandler {
    /// Creates a new signal handler with optional programmatic termination channel
    pub fn new(terminate_rx: Option<broadcast::Receiver<()>>) -> Result<Self, std::io::Error> {
        Ok(Self {
            sigint: Some(tokio::signal::unix::signal(SignalKind::interrupt())?),
            sigterm: Some(tokio::signal::unix::signal(SignalKind::terminate())?),
            terminate_rx: terminate_rx.unwrap_or_else(|| {
                let (tx, rx) = broadcast::channel(1);
                drop(tx);
                rx
            }),
        })
    }

    /// Waits for either an OS signal (SIGINT/SIGTERM) or a broadcast
    /// channel message. Gracefully degrades if signal handler registration
    /// fails (e.g. in a constrained container).
    pub async fn wait_for_shutdown(&mut self) {
        await_signals_or_channel(self.sigint.take(), self.sigterm.take(), self.terminate_rx.recv()).await;
    }
}

#[allow(clippy::cognitive_complexity)]
async fn await_signals_or_channel<F>(sigint: Option<Signal>, sigterm: Option<Signal>, terminate_rx: F)
where
    F: std::future::Future<Output = Result<(), broadcast::error::RecvError>>,
{
    match (sigint, sigterm) {
        (Some(mut sigint), Some(mut sigterm)) => {
            tokio::select! {
                _ = sigint.recv() => {
                    debug!("Received SIGINT signal");
                }
                _ = sigterm.recv() => {
                    debug!("Received SIGTERM signal");
                }
                _ = terminate_rx => {
                    debug!("Received termination signal");
                }
            }
        }
        _ => {
            debug!("Signal handlers unavailable, awaiting programmatic termination");
            let _ = terminate_rx.await;
            debug!("Received termination signal");
        }
    }
}

/// Helper function to await signal or channel (extracted for reuse)
pub async fn await_signal_or_channel(
    sigint: Option<Signal>,
    sigterm: Option<Signal>,
    mut terminate_rx: broadcast::Receiver<()>,
) {
    await_signals_or_channel(sigint, sigterm, terminate_rx.recv()).await
}
