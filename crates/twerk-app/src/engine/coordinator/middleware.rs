//! HTTP middleware for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, Instrument};
use twerk_infrastructure::config;

use crate::engine::coordinator::auth::{BasicAuthConfig, KeyAuthConfig};
use crate::engine::coordinator::limits::{BodyLimitConfig, RateLimitConfig};
use crate::engine::coordinator::utils::{parse_body_limit, wildcard_match};

// ── CORS Middleware ────────────────────────────────────────────

pub fn cors_layer() -> CorsLayer {
    let allow_credentials = config::bool_default("middleware.web.cors.credentials", false);
    debug!("CORS middleware enabled");

    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any)
        .allow_credentials(allow_credentials)
}

// ── HTTP Logging Middleware ────────────────────────────────────

#[derive(Clone, Debug)]
pub struct HttpLogConfig {
    pub(crate) level: String,
    pub(crate) skip_paths: Vec<String>,
}

impl Default for HttpLogConfig {
    fn default() -> Self {
        Self {
            level: "DEBUG".to_string(),
            skip_paths: vec!["GET /health".to_string()],
        }
    }
}

pub async fn http_log_middleware(
    axum::extract::State(config): axum::extract::State<HttpLogConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();
    let pattern = format!("{} {}", method.as_str(), path);

    if config.skip_paths.iter().any(|p| wildcard_match(p, &pattern)) {
        return next.run(request).await;
    }

    let start = Instant::now();
    let client_ip = request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let span = tracing::info_span!(
        "http_request",
        method = %method,
        uri = %uri,
        remote_ip = %client_ip
    );

    async move {
        let response = next.run(request).await;
        let elapsed = start.elapsed();
        let status = response.status();

        let log_level = if status.is_server_error() {
            "ERROR"
        } else if status.is_client_error() {
            "WARN"
        } else {
            &config.level
        };

        match log_level {
            "ERROR" => error!(method=%method, uri=%uri, status=%status.as_u16(), remote_ip=%client_ip, elapsed_ms=elapsed.as_millis() as u64, "HTTP Request"),
            "WARN" => tracing::warn!(method=%method, uri=%uri, status=%status.as_u16(), remote_ip=%client_ip, elapsed_ms=elapsed.as_millis() as u64, "HTTP Request"),
            "INFO" => info!(method=%method, uri=%uri, status=%status.as_u16(), remote_ip=%client_ip, elapsed_ms=elapsed.as_millis() as u64, "HTTP Request"),
            _ => debug!(method=%method, uri=%uri, status=%status.as_u16(), remote_ip=%client_ip, elapsed_ms=elapsed.as_millis() as u64, "HTTP Request"),
        }
        response
    }
    .instrument(span)
    .await
}

// ── Middleware Factory ─────────────────────────────────────────

#[allow(clippy::type_complexity)]
pub fn create_web_middlewares(
    datastore: Arc<dyn twerk_infrastructure::datastore::Datastore>,
) -> (
    Option<CorsLayer>,
    Option<BasicAuthConfig>,
    Option<KeyAuthConfig>,
    Option<RateLimitConfig>,
    Option<BodyLimitConfig>,
    Option<HttpLogConfig>,
) {
    let cors = config::bool("middleware.web.cors.enabled").then(cors_layer);
    let basic_auth = config::bool("middleware.web.basicauth.enabled").then(|| BasicAuthConfig::new(datastore));
    let key_auth = config::bool("middleware.web.keyauth.enabled").then(|| KeyAuthConfig::new(config::string_default("middleware.web.keyauth.key", "")));
    let rate_limit = config::bool("middleware.web.ratelimit.enabled").then(|| RateLimitConfig::new(config::int_default("middleware.web.ratelimit.rps", 20) as u32));
    let body_limit = parse_body_limit(&config::string_default("middleware.web.bodylimit", "500K")).map(BodyLimitConfig::new);
    let http_log = config::bool_default("middleware.web.logger.enabled", true).then(|| {
        HttpLogConfig {
            level: config::string_default("middleware.web.logger.level", "DEBUG"),
            skip_paths: config::strings_default("middleware.web.logger.skip", &["GET /health"]),
        }
    });

    (cors, basic_auth, key_auth, rate_limit, body_limit, http_log)
}
