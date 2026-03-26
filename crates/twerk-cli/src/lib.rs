//! Twerk CLI - Command-line interface for the Twerk distributed workflow engine
//!
//! # Architecture
//!
//! - **Data**: Configuration types, command arguments - pure data structs
//! - **Calc**: Pure functions for banner formatting, health check parsing
//! - **Actions**: I/O operations (HTTP requests, logging) at the boundary

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![allow(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

mod banner;
pub mod cli;
pub mod commands;
pub mod error;
pub mod health;
pub mod migrate;
pub mod run;

pub use cli::{run, setup_logging, DEFAULT_DATASTORE_TYPE, DEFAULT_ENDPOINT, VERSION};
pub use commands::Commands;
pub use error::CliError;
