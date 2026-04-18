use twerk_web::api::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let spec = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&spec).expect("failed to serialize OpenAPI spec");

    let out_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("openapi.json");

    std::fs::write(&out_path, json).expect("failed to write openapi.json");

    println!("Generated OpenAPI spec at {}", out_path.display());
}
