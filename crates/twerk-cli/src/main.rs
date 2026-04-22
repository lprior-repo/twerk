//! Twerk CLI binary entry point

use std::process;
use twerk_cli::run;

#[tokio::main]
async fn main() {
    process::exit(run().await);
}
