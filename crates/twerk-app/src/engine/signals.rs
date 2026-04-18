//! Twerk Engine - Signal handling and shutdown coordination

use tokio::signal::unix::{Signal, SignalKind};
use tokio::sync::broadcast;
use tracing::debug;

enum SignalOrTermination {
    Sigint,
    Sigterm,
    Termination,
}

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
        if let (Some(sigint), Some(sigterm)) = (self.sigint.as_mut(), self.sigterm.as_mut()) {
            wait_for_signal_or_termination(sigint, sigterm, &mut self.terminate_rx).await;
        } else {
            wait_for_termination_channel(&mut self.terminate_rx).await;
        }
    }
}

/// Helper function to await signal or channel (extracted for reuse)
pub async fn await_signal_or_channel(
    sigint: Option<Signal>,
    sigterm: Option<Signal>,
    mut terminate_rx: broadcast::Receiver<()>,
) {
    match (sigint, sigterm) {
        (Some(mut sigint), Some(mut sigterm)) => {
            wait_for_signal_or_termination(&mut sigint, &mut sigterm, &mut terminate_rx).await;
        }
        _ => {
            wait_for_termination_channel(&mut terminate_rx).await;
        }
    }
}

async fn wait_for_signal_or_termination(
    sigint: &mut Signal,
    sigterm: &mut Signal,
    terminate_rx: &mut broadcast::Receiver<()>,
) {
    let received = select_signal_or_termination(sigint, sigterm, terminate_rx).await;
    log_received_signal(received);
}

async fn select_signal_or_termination(
    sigint: &mut Signal,
    sigterm: &mut Signal,
    terminate_rx: &mut broadcast::Receiver<()>,
) -> SignalOrTermination {
    tokio::select! {
        _ = sigint.recv() => SignalOrTermination::Sigint,
        _ = sigterm.recv() => SignalOrTermination::Sigterm,
        _ = terminate_rx.recv() => SignalOrTermination::Termination,
    }
}

fn log_received_signal(signal: SignalOrTermination) {
    match signal {
        SignalOrTermination::Sigint => {
            debug!("Received SIGINT signal");
        }
        SignalOrTermination::Sigterm => {
            debug!("Received SIGTERM signal");
        }
        SignalOrTermination::Termination => {
            debug!("Received termination signal");
        }
    }
}

async fn wait_for_termination_channel(receive: &mut broadcast::Receiver<()>) {
    debug!("Signal handlers unavailable, awaiting programmatic termination");
    let _ = receive.recv().await;
    debug!("Received termination signal");
}