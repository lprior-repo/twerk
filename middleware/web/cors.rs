//! CORS middleware for Axum.
//!
//! Configurable Cross-Origin Resource Sharing using tower-http's `CorsLayer`.
//! Supports allowed origins, methods, headers, and credentials.
//!
//! # Go Parity
//!
//! Maps to Go `middleware.NewCORS()`.

use axum::http::header::{HeaderName, ACCEPT, AUTHORIZATION, CONTENT_TYPE, ORIGIN};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};

use super::config::CorsConfig;

/// Build a `CorsLayer` from the given configuration.
///
/// Returns a tower-http `CorsLayer` that can be applied to an Axum router
/// via `.layer(cors_layer(config))`.
pub fn cors_layer(config: &CorsConfig) -> CorsLayer {
    let mut layer = CorsLayer::new();

    // Origins
    layer = if config.allowed_origins.is_empty() {
        layer.allow_origin(AllowOrigin::any())
    } else {
        let origins: Vec<_> = config
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        if origins.is_empty() {
            layer.allow_origin(AllowOrigin::any())
        } else {
            layer.allow_origin(origins)
        }
    };

    // Methods
    layer = if config.allowed_methods.is_empty() {
        layer.allow_methods(AllowMethods::mirror_request())
    } else {
        let methods: Vec<_> = config
            .allowed_methods
            .iter()
            .filter_map(|m| m.parse().ok())
            .collect();
        if methods.is_empty() {
            layer.allow_methods(AllowMethods::mirror_request())
        } else {
            layer.allow_methods(methods)
        }
    };

    // Headers
    layer = if config.allowed_headers.is_empty() {
        layer.allow_headers(AllowHeaders::mirror_request())
    } else {
        let headers: Vec<HeaderName> = config
            .allowed_headers
            .iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        if headers.is_empty() {
            layer.allow_headers(AllowHeaders::mirror_request())
        } else {
            layer.allow_headers(headers)
        }
    };

    // Credentials
    if config.allow_credentials {
        layer = layer.allow_credentials(true);
    }

    // Always expose common headers
    layer.allow_headers([CONTENT_TYPE, AUTHORIZATION, ACCEPT, ORIGIN])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_layer_default_config() {
        let config = CorsConfig::default();
        let _layer = cors_layer(&config);
    }

    #[test]
    fn test_cors_layer_with_origins() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            allowed_methods: vec!["GET".to_string(), "POST".to_string()],
            allowed_headers: vec!["Content-Type".to_string()],
            allow_credentials: true,
        };
        let _layer = cors_layer(&config);
    }

    #[test]
    fn test_cors_layer_with_invalid_origins() {
        let config = CorsConfig {
            allowed_origins: vec!["not a valid origin".to_string()],
            ..Default::default()
        };
        // Should fall back to AllowOrigin::any()
        let _layer = cors_layer(&config);
    }
}
