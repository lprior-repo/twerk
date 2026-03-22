//! Network connectivity utilities
//!
//! Provides functionality for checking TCP connectivity.

use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

/// Errors that can occur during network operations.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("invalid socket address: {0}")]
    InvalidAddress(String),

    #[error("connection timeout")]
    Timeout,
}

/// Check if a TCP connection can be established to the given address.
///
/// Uses a 1-second timeout for the connection attempt.
///
/// # Arguments
///
/// * `address` - The address to connect to (e.g., "localhost:9999")
///
/// # Returns
///
/// `true` if connection succeeds, `false` otherwise
#[must_use]
pub fn can_connect(address: &str) -> bool {
    let timeout = Duration::from_secs(1);

    match SocketAddr::from_str(address) {
        Ok(addr) => match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                // Successfully connected, close the stream and return true
                let _ = stream.shutdown(std::net::Shutdown::Both);
                true
            }
            Err(_) => false,
        },
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn test_can_connect_when_listening() {
        // Create a listener on a random available port
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        let port = listener
            .local_addr()
            .expect("failed to get local addr")
            .port();
        let addr = format!("127.0.0.1:{port}");

        // Channel to signal when listener thread is ready to accept
        let (tx, rx) = mpsc::channel();

        // Spawn a thread to accept a connection (blocking)
        let handle = std::thread::spawn(move || {
            // Signal that we're about to accept
            let _ = tx.send(());
            // This will block until a connection is made
            let _ = listener.accept();
        });

        // Wait for the thread to be ready to accept
        let _ = rx.recv_timeout(Duration::from_secs(1));

        // Small delay to ensure the accept call is actually executing
        std::thread::sleep(Duration::from_millis(10));

        // Now we should be able to connect
        assert!(
            can_connect(&addr),
            "should be able to connect to listening port"
        );

        // Wait for the accept thread
        let _ = handle.join();
    }

    #[test]
    fn test_cannot_connect_when_not_listening() {
        // Use a port that's unlikely to be in use
        assert!(
            !can_connect("localhost:19888"),
            "should not be able to connect to non-listening port"
        );
    }
}
