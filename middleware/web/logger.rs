//! Request logging middleware for Axum.
//!
//! Logs each request with method, path, status code, and duration.
//! Supports configurable log level and path skipping.
//!
//! # Go Parity
//!
//! Maps to Go `middleware.NewRequestLogger()`.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use tower::{Layer, Service};
use tracing::{debug, error, info, trace, warn};

use super::config::LoggerConfig;

/// Tower layer for request logging.
#[derive(Clone)]
pub struct RequestLoggerLayer {
    level: LogLevel,
    skip_paths: Vec<String>,
}

/// Log levels supported by the request logger.
#[derive(Debug, Clone, Copy)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }
}

impl RequestLoggerLayer {
    /// Create a new request logger layer from configuration.
    #[must_use]
    pub fn new(config: &LoggerConfig) -> Self {
        Self {
            level: LogLevel::from_str(&config.level),
            skip_paths: config.skip_paths.clone(),
        }
    }
}

impl<S> Layer<S> for RequestLoggerLayer {
    type Service = RequestLoggerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestLoggerService {
            inner,
            level: self.level,
            skip_paths: self.skip_paths.clone(),
        }
    }
}

/// Tower service that logs requests.
#[derive(Clone)]
pub struct RequestLoggerService<S> {
    inner: S,
    level: LogLevel,
    skip_paths: Vec<String>,
}

impl<S, ReqBody> Service<Request<ReqBody>> for RequestLoggerService<S>
where
    S: Service<Request<ReqBody>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();

        // Check if this path should be skipped
        let should_skip = self
            .skip_paths
            .iter()
            .any(|prefix| path.starts_with(prefix));

        if should_skip {
            // Skip logging — just forward the request
            let fut = self.inner.call(req);
            return Box::pin(fut);
        }

        let level = self.level;
        let start = Instant::now();

        // Clone inner service for the async block
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let response = inner.call(req).await?;
            let duration = start.elapsed();
            let status = response.status();

            log_request(level, &method, &path, status, duration);

            Ok(response)
        })
    }
}

/// Log a request at the configured level.
fn log_request(
    level: LogLevel,
    method: &str,
    path: &str,
    status: StatusCode,
    duration: std::time::Duration,
) {
    let status_code = status.as_u16();
    let duration_ms = duration.as_millis();

    match level {
        LogLevel::Trace => trace!(method, path, status = status_code, duration_ms, "request"),
        LogLevel::Debug => debug!(method, path, status = status_code, duration_ms, "request"),
        LogLevel::Info => info!(method, path, status = status_code, duration_ms, "request"),
        LogLevel::Warn => warn!(method, path, status = status_code, duration_ms, "request"),
        LogLevel::Error => error!(method, path, status = status_code, duration_ms, "request"),
    }
}

/// Create a request logger layer from config.
#[must_use]
pub fn logger_layer(config: &LoggerConfig) -> RequestLoggerLayer {
    RequestLoggerLayer::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_layer_default() {
        let config = LoggerConfig::default();
        let _layer = logger_layer(&config);
    }

    #[test]
    fn test_logger_layer_with_skip_paths() {
        let config = LoggerConfig {
            level: "debug".to_string(),
            skip_paths: vec!["/health".to_string(), "/metrics".to_string()],
        };
        let _layer = logger_layer(&config);
    }

    #[test]
    fn test_log_level_from_str() {
        assert!(matches!(LogLevel::from_str("trace"), LogLevel::Trace));
        assert!(matches!(LogLevel::from_str("debug"), LogLevel::Debug));
        assert!(matches!(LogLevel::from_str("info"), LogLevel::Info));
        assert!(matches!(LogLevel::from_str("INFO"), LogLevel::Info));
        assert!(matches!(LogLevel::from_str("warn"), LogLevel::Warn));
        assert!(matches!(LogLevel::from_str("warning"), LogLevel::Warn));
        assert!(matches!(LogLevel::from_str("error"), LogLevel::Error));
        assert!(matches!(LogLevel::from_str("unknown"), LogLevel::Info)); // default
    }
}
