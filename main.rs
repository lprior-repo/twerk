//! Tork CLI binary entry point
//!
//! Loads configuration and runs the CLI.

use std::process;

use tork_cli::{run, CliError};

#[tokio::main]
async fn main() {
    // Load configuration from config.toml and environment variables
    if let Err(e) = conf::load_config() {
        eprintln!("Error loading config: {}", e);
        process::exit(1);
    }

    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
