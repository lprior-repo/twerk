use std::fs;
use std::path::Path;

fn main() {
    let spec = twerk_web::create_openapi_spec(env!("CARGO_PKG_VERSION"));
    let json = serde_json::to_string_pretty(&spec).expect("failed to serialize OpenAPI spec");

    let out_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("openapi.json");

    fs::write(&out_path, json).expect("failed to write openapi.json");

    println!("Generated OpenAPI spec at {}", out_path.display());
}
