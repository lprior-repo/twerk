use std::path::PathBuf;

use serde_json::Value;

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn tracked_spec_json() -> Value {
    serde_json::from_slice(&std::fs::read(workspace_root().join("docs/openapi.json")).unwrap())
        .unwrap()
}

pub fn tracked_spec_yaml_as_json() -> Value {
    serde_yaml::from_slice::<serde_yaml::Value>(
        &std::fs::read(workspace_root().join("docs/openapi.yaml")).unwrap(),
    )
    .map(|value| serde_json::to_value(value).unwrap())
    .unwrap()
}

pub fn mirrored_web_spec_json() -> Value {
    serde_json::from_slice(
        &std::fs::read(workspace_root().join("crates/twerk-web/openapi.json")).unwrap(),
    )
    .unwrap()
}

pub fn request_body_content<'a>(spec: &'a Value, path: &str) -> &'a serde_json::Map<String, Value> {
    spec["paths"][path]["post"]["requestBody"]["content"]
        .as_object()
        .unwrap()
}

pub fn request_body_schema_ref(spec: &Value, path: &str, media_type: &str) -> String {
    spec["paths"][path]["post"]["requestBody"]["content"][media_type]["schema"]["$ref"]
        .as_str()
        .unwrap()
        .to_string()
}
