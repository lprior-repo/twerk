//! # HTTPX Module
//!
//! HTTP server utilities for asynchronous server startup and connection management.

use std::net::SocketAddr;
use std::time::Duration;

use thiserror::Error;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tracing::info;

use crate::netx::can_connect;

/// Errors that can occur during HTTP server operations.
#[derive(Debug, Error)]
pub enum HttpError {
    #[error("failed to bind to address: {0}")]
    BindError(String),

    #[error("failed to start server: {0}")]
    StartupError(String),

    #[error("unable to start API server: connection timeout")]
    ConnectionTimeout,
}

/// Starts an HTTP server asynchronously and waits for it to become ready.
///
/// This function spawns the server in a background task and polls for
/// connection availability up to 100 times with 100ms intervals.
///
/// # Arguments
///
/// * `addr` - The socket address to bind the server to
/// * `router` - The Axum router to serve
///
/// # Returns
///
/// A `Result` containing the spawned server handle or an `HttpError`
pub async fn start_async(addr: SocketAddr, router: axum::Router) -> Result<(), HttpError> {
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| HttpError::BindError(e.to_string()))?;

    let addr_str = format!("{}", addr);

    tokio::spawn(async move {
        let server = axum::serve(listener, router);
        info!("server stopped: {:?}", server.await);
    });

    // Wait for server to be ready (poll up to 100 times with 100ms delay)
    for _ in 0..100 {
        if can_connect(&addr_str) {
            return Ok(());
        }
        timeout(
            Duration::from_millis(100),
            tokio::time::sleep(Duration::from_millis(100)),
        )
        .await
        .ok();
    }

    Err(HttpError::ConnectionTimeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;

    #[tokio::test]
    async fn test_start_async() {
        let addr: std::net::SocketAddr = match "127.0.0.1:0".parse() {
            Ok(a) => a,
            Err(_) => {
                // Socket address parsing should always succeed for this literal
                return;
            }
        };

        let router = Router::new().route("/", axum::routing::get(|| async { "OK" }));

        let result = start_async(addr, router).await;
        assert!(result.is_ok(), "server should start successfully");
    }
}
