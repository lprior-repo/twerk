//! HTTP extension utilities for async server lifecycle management.

use std::net::SocketAddr;
use std::time::Duration;

use thiserror::Error;
use tokio::time::sleep;

/// Errors that can occur during HTTP server operations.
#[derive(Debug, Error)]
pub enum HttpxError {
    /// The server failed to start.
    #[error("server error: {0}")]
    ServerError(String),

    /// Unable to establish connectivity within the timeout period.
    #[error("unable to start server: could not connect after {0} attempts")]
    ConnectionTimeout(u32),

    /// The server address is malformed.
    #[error("invalid server address: {0}")]
    InvalidAddress(String),
}

/// Configuration for server startup polling.
#[derive(Debug, Clone, Copy)]
pub struct PollingConfig {
    /// Maximum number of connection attempts.
    pub max_attempts: u32,
    /// Delay between each polling attempt.
    pub delay: Duration,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            max_attempts: 100,
            delay: Duration::from_millis(100),
        }
    }
}

/// Checks if a TCP connection can be established to the given address.
/// This is the core connectivity calculation - a pure function with no side effects.
#[must_use]
pub fn can_connect(address: &str) -> bool {
    address
        .parse::<SocketAddr>()
        .ok()
        .and_then(|addr| std::net::TcpStream::connect(addr).ok())
        .is_some()
}

/// Spawns the HTTP server and returns an error receiver channel.
fn spawn_server(
    listener: tokio::net::TcpListener,
    router: axum::Router,
) -> tokio::sync::mpsc::Receiver<HttpxError> {
    let (err_sender, err_receiver) = tokio::sync::mpsc::channel::<HttpxError>(1);

    tokio::spawn(async move {
        let server = axum::serve(listener, router);

        if let Err(e) = server.await {
            let _ = err_sender
                .send(HttpxError::ServerError(e.to_string()))
                .await;
        }
    });

    err_receiver
}

/// Handles retry logic when binding to an address fails.
async fn handle_bind_retry(
    addr: &str,
    bind_error: std::io::Error,
    config: PollingConfig,
) -> Result<(), HttpxError> {
    for _ in 0..config.max_attempts {
        if can_connect(addr) {
            return Err(HttpxError::ServerError(bind_error.to_string()));
        }

        sleep(config.delay).await;
    }

    Err(HttpxError::ConnectionTimeout(config.max_attempts))
}

/// Polls for server readiness until connected or timeout.
async fn poll_for_readiness(
    addr: &str,
    delay: Duration,
    max_attempts: u32,
    mut err_receiver: tokio::sync::mpsc::Receiver<HttpxError>,
) -> Result<(), HttpxError> {
    for _ in 0..max_attempts {
        if let Ok(err) = err_receiver.try_recv() {
            return Err(err);
        }

        if can_connect(addr) {
            return Ok(());
        }

        sleep(delay).await;
    }

    Err(HttpxError::ConnectionTimeout(max_attempts))
}

/// Starts an HTTP server asynchronously and waits for it to become ready.
///
/// This function spawns the server in a background task and polls for connectivity
/// up to `max_attempts` times with a 100ms delay between each attempt.
///
/// # Arguments
///
/// * `address` - The socket address to bind to (e.g., "127.0.0.1:8080")
/// * `router` - The axum Router to serve
/// * `config` - Polling configuration for connection checks
///
/// # Returns
///
/// Returns `Ok(())` if the server starts successfully and is reachable,
/// or an `HttpxError` if the server fails to start or cannot be reached.
///
/// # Errors
///
/// Returns [`HttpxError::ServerError`] if the server fails to bind or serve.
/// Returns [`HttpxError::ConnectionTimeout`] if the server doesn't become
/// reachable within the configured number of attempts.
pub async fn start_async(
    address: &str,
    router: axum::Router,
    config: PollingConfig,
) -> Result<(), HttpxError> {
    let address = address.to_string();
    let addr = address
        .parse::<SocketAddr>()
        .map_err(|_| HttpxError::InvalidAddress(address.clone()))?;

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(bind_error) => {
            return handle_bind_retry(&address, bind_error, config).await;
        }
    };

    let actual_addr = listener
        .local_addr()
        .map_err(|e| HttpxError::ServerError(e.to_string()))?;

    let err_receiver = spawn_server(listener, router);
    let polling_addr = actual_addr.to_string();

    poll_for_readiness(
        &polling_addr,
        config.delay,
        config.max_attempts,
        err_receiver,
    )
    .await
}

#[cfg(test)]
mod tests {
    #![deny(clippy::unwrap_used)]
    #![deny(clippy::expect_used)]
    #![deny(clippy::uninlined_format_args)]
    #![deny(clippy::panic)]
    use super::*;
    use axum::{routing::get, Router};

    fn get_available_addr() -> SocketAddr {
        use std::net::TcpListener as StdTcpListener;
        let listener = StdTcpListener::bind("127.0.0.1:0")
            .expect("failed to bind to random port for test");
        listener
            .local_addr()
            .expect("failed to get local address")
    }

    #[tokio::test]
    async fn test_start_async_success() {
        let addr = get_available_addr();
        let addr_str = format!("{addr}");

        // Create a simple router that responds with 200 OK
        let app = Router::new().route("/health", get(|| async { "ok" }));

        let config = PollingConfig {
            max_attempts: 50,
            delay: Duration::from_millis(50),
        };

        // Start the server - should succeed
        let result = start_async(&addr_str, app, config).await;
        assert!(result.is_ok(), "start_async failed: {:?}", result.err());
    }

    #[allow(clippy::redundant_pattern_matching)]
    #[tokio::test]
    async fn test_start_async_connection_timeout() {
        let addr = "127.0.0.1:1"; // Nothing listening here

        let config = PollingConfig {
            max_attempts: 3,
            delay: Duration::from_millis(10),
        };

        // Create a router that never completes (simulated by not starting server)
        let app = Router::new();

        // Start should fail because nothing is listening
        let result = start_async(addr, app, config).await;
        assert!(matches!(result, Err(_)));

        match result {
            Err(HttpxError::ConnectionTimeout(n)) => {
                assert_eq!(n, 3);
            }
            _ => unreachable!("expected ConnectionTimeout error"),
        }
    }

    #[test]
    fn test_can_connect_with_invalid_address() {
        // Invalid address should return false, not panic
        assert!(!can_connect("not-an-address"));
        assert!(!can_connect(""));
    }

    #[test]
    fn test_can_connect_with_unreachable_address() {
        // Address with no listener should return false, not panic
        assert!(!can_connect("127.0.0.1:1"));
    }

    #[test]
    fn test_polling_config_default() {
        let config = PollingConfig::default();
        assert_eq!(config.max_attempts, 100);
        assert_eq!(config.delay, Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_can_connect_with_valid_address() {
        let addr = get_available_addr();
        let addr_str = format!("{addr}");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("failed to bind TCP listener for test");

        assert!(can_connect(&addr_str));

        drop(listener);
    }
}
