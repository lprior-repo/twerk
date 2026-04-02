//! Twerk CLI binary entry point

use std::process;
use twerk_cli::run;

#[tokio::main]
async fn main() {
    // Run the CLI
    if let Err(e) = run().await {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
