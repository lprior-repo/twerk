//! Web middleware configuration.
//!
//! Provides typed configuration for all built-in middleware components.

/// Configuration for CORS middleware.
///
/// Maps to Go `middleware.CORSConfig`.
#[derive(Debug, Clone, Default)]
pub struct CorsConfig {
    /// Allowed origins. Empty means allow all.
    pub allowed_origins: Vec<String>,
    /// Allowed HTTP methods. Defaults to common methods.
    pub allowed_methods: Vec<String>,
    /// Allowed headers. Defaults to common headers.
    pub allowed_headers: Vec<String>,
    /// Whether to allow credentials (cookies, auth headers).
    pub allow_credentials: bool,
}

/// Configuration for rate limiting middleware.
///
/// Maps to Go `middleware.RateLimitConfig`.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per second.
    pub requests_per_second: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
        }
    }
}

/// Configuration for request logging middleware.
///
/// Maps to Go `middleware.RequestLoggerConfig`.
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Log level to use (e.g. "info", "debug", "trace").
    pub level: String,
    /// Path prefixes to skip logging (e.g. "/health").
    pub skip_paths: Vec<String>,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            skip_paths: vec![],
        }
    }
}

/// Configuration for body limit middleware.
///
/// Maps to Go `middleware.BodyLimitConfig`.
#[derive(Debug, Clone)]
pub struct BodyLimitConfig {
    /// Maximum request body size in bytes.
    pub max_size: usize,
}

impl Default for BodyLimitConfig {
    fn default() -> Self {
        Self {
            max_size: 4 * 1024 * 1024, // 4 MB
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_config_default() {
        let config = CorsConfig::default();
        assert!(config.allowed_origins.is_empty());
        assert!(config.allowed_methods.is_empty());
        assert!(config.allowed_headers.is_empty());
        assert!(!config.allow_credentials);
    }

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.requests_per_second, 100);
    }

    #[test]
    fn test_logger_config_default() {
        let config = LoggerConfig::default();
        assert_eq!(config.level, "info");
        assert!(config.skip_paths.is_empty());
    }

    #[test]
    fn test_body_limit_config_default() {
        let config = BodyLimitConfig::default();
        assert_eq!(config.max_size, 4 * 1024 * 1024);
    }
}
