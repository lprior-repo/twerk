//! API module for the coordinator HTTP server.
//!
//! Go parity: internal/coordinator/api/api.go
//! Middleware ordering follows Go's engine/coordinator.go:
//! 1. Body limit (always applied)
//! 2. CORS (config-gated)
//! 3. Basic auth (config-gated)
//! 4. Key auth (config-gated)
//! 5. Rate limit (config-gated)
//! 6. Logger (default enabled)

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

pub mod combinatorial;
pub mod content_type;
pub mod domain;
pub mod error;
pub mod handlers;
pub mod openapi;
pub mod openapi_types;
pub mod redact;
mod router;
mod state;
pub mod trigger_api;
pub mod types;
pub mod yaml;

pub use router::create_router;
pub use state::{AppState, Config};

#[cfg(test)]
mod trigger_update_unit_red_tests;
