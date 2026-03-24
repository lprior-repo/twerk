//! Body limit middleware for Axum.
//!
//! Limits the maximum size of incoming request bodies using
//! tower-http's `RequestBodyLimitLayer`.
//!
//! # Go Parity
//!
//! Maps to Go `middleware.NewBodyLimit()`.

use tower_http::limit::RequestBodyLimitLayer;

use super::config::BodyLimitConfig;

/// Build a `RequestBodyLimitLayer` from the given configuration.
///
/// Limits incoming request bodies to the configured maximum size.
/// Requests exceeding the limit will receive a 413 Payload Too Large response.
#[must_use]
pub fn body_limit_layer(config: &BodyLimitConfig) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(config.max_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_limit_layer_default() {
        let config = BodyLimitConfig::default();
        let _layer = body_limit_layer(&config);
    }

    #[test]
    fn test_body_limit_layer_custom() {
        let config = BodyLimitConfig {
            max_size: 1024 * 1024, // 1 MB
        };
        let _layer = body_limit_layer(&config);
    }

    #[test]
    fn test_body_limit_layer_small() {
        let config = BodyLimitConfig { max_size: 100 };
        let _layer = body_limit_layer(&config);
    }
}
