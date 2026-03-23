//! Network connectivity utilities
//!
//! Provides functionality for checking TCP connectivity.
//! Parity with Go's `internal/netx/netx.go`: `CanConnect` uses
//! `net.DialTimeout("tcp", address, 1s)` which resolves DNS before
//! connecting. Rust mirrors this by resolving via `ToSocketAddrs` first.

#![deny(clippy::unwrap_used)]

use std::net::TcpStream;
use std::time::Duration;

/// Check if a TCP connection can be established to the given address.
///
/// Uses a 1-second timeout for the connection attempt.
/// Resolves DNS hostnames before connecting, matching Go's `net.DialTimeout`.
///
/// # Arguments
///
/// * `address` - The address to connect to (e.g., "localhost:9999", "example.com:80")
///
/// # Returns
///
/// `true` if connection succeeds, `false` otherwise
#[must_use]
pub fn can_connect(address: &str) -> bool {
    let timeout = Duration::from_secs(1);

    // Resolve address (DNS + port) — matches Go's net.DialTimeout which resolves first
    let addrs: Vec<std::net::SocketAddr> = match std::net::ToSocketAddrs::to_socket_addrs(address) {
        Ok(a) => a.collect(),
        Err(_) => return false,
    };

    // Try each resolved address (Go also iterates resolved addresses)
    addrs
        .iter()
        .any(|addr| match TcpStream::connect_timeout(addr, timeout) {
            Ok(stream) => {
                let _ = stream.shutdown(std::net::Shutdown::Both);
                true
            }
            Err(_) => false,
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Parity with Go `TestCanConnect`: verifies `CanConnect("localhost:9999")`
    /// returns true when a listener is bound on that port.
    #[test]
    fn test_can_connect_when_listening() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        let port = listener
            .local_addr()
            .expect("failed to get local addr")
            .port();
        let addr = format!("127.0.0.1:{port}");

        let (tx, rx) = mpsc::channel();

        let handle = std::thread::spawn(move || {
            let _ = tx.send(());
            let _ = listener.accept();
        });

        let _ = rx.recv_timeout(Duration::from_secs(1));
        std::thread::sleep(Duration::from_millis(10));

        assert!(
            can_connect(&addr),
            "should be able to connect to listening port"
        );

        let _ = handle.join();
    }

    /// Parity with Go `TestCanConnect`: verifies `CanConnect("localhost:8888")`
    /// returns false when nothing is listening on that port.
    #[test]
    fn test_cannot_connect_when_not_listening() {
        assert!(
            !can_connect("localhost:19888"),
            "should not be able to connect to non-listening port"
        );
    }
}
