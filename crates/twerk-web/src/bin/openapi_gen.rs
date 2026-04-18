//! OpenAPI spec generator for Twerk API.
//!
//! This binary generates a complete OpenAPI 3.0 specification based on
//! the API handlers and domain types in twerk-web.

use std::fs;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let spec = twerk_web::create_openapi_spec(env!("CARGO_PKG_VERSION"));

    // Create docs directory if it doesn't exist
    let docs_dir = Path::new("docs");
    if !docs_dir.exists() {
        fs::create_dir_all(docs_dir)?;
    }

    // Write the spec to docs/openapi.json
    let output_path = docs_dir.join("openapi.json");
    fs::write(&output_path, &spec)?;

    println!("OpenAPI spec generated: {}", output_path.display());
    Ok(())
}
