use axum::http::{header, HeaderMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestContentType {
    Json,
    Yaml,
    Unsupported,
}

#[must_use]
pub fn normalized_content_type(headers: &HeaderMap) -> String {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map_or("", |value| value)
        .split(';')
        .next()
        .map_or("", str::trim)
        .to_ascii_lowercase()
}

#[must_use]
pub fn classify_content_type(content_type: &str) -> RequestContentType {
    match content_type {
        "application/json" => RequestContentType::Json,
        "text/yaml" | "application/x-yaml" | "application/yaml" => RequestContentType::Yaml,
        _ => RequestContentType::Unsupported,
    }
}
