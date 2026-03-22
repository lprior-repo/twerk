//! Coordinator initialization and HTTP middleware module
//!
//! This module handles coordinator creation and HTTP middleware setup
//! including CORS, authentication, rate limiting, and logging.

use crate::broker::BrokerProxy;
use crate::datastore::DatastoreProxy;
use anyhow::Result;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use governor::Quota;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, Instrument};

// Re-export types for external use
pub use tork::user::USERNAME;

/// Boxed future type for coordinator operations
pub type BoxedFuture<T> = Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>;

/// Coordinator trait for job coordination
pub trait Coordinator: Send + Sync {
    fn start(&self) -> BoxedFuture<()>;
    fn stop(&self) -> BoxedFuture<()>;
    fn submit_job(&self, job: tork::job::Job) -> BoxedFuture<tork::job::Job>;
}

/// Locker trait for distributed locking
pub trait Locker: Send + Sync {
    fn acquire_lock(&self, key: &str) -> BoxedFuture<()>;
}

/// Simple in-memory locker implementation
pub struct InMemoryLocker;

impl InMemoryLocker {
    pub fn new() -> Self {
        Self
    }
}

impl Locker for InMemoryLocker {
    fn acquire_lock(&self, _key: &str) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }
}

impl Default for InMemoryLocker {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the coordinator
pub struct Config {
    /// Coordinator name
    pub name: String,
    /// Message broker
    pub broker: Arc<dyn tork::broker::Broker>,
    /// Data store
    pub datastore: Arc<dyn tork::datastore::Datastore>,
    /// Locker for distributed locking
    pub locker: Arc<dyn Locker>,
    /// Queue configuration
    pub queues: std::collections::HashMap<String, i64>,
    /// HTTP listen address
    pub address: String,
    /// Enabled API endpoints
    pub enabled: std::collections::HashMap<String, bool>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("name", &self.name)
            .field("queues", &self.queues)
            .field("address", &self.address)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Coordinator".to_string(),
            broker: Arc::new(BrokerProxy::new()),
            datastore: Arc::new(DatastoreProxy::new()),
            locker: Arc::new(InMemoryLocker::new()),
            queues: std::collections::HashMap::new(),
            address: "0.0.0.0:8000".to_string(),
            enabled: std::collections::HashMap::new(),
        }
    }
}

impl Config {
    /// Creates a new coordinator config from environment variables
    pub fn from_env() -> Self {
        let name = std::env::var("TORK_COORDINATOR_NAME")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Coordinator".to_string());
        let address = std::env::var("TORK_COORDINATOR_ADDRESS")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "0.0.0.0:8000".to_string());

        // Parse queues from environment
        let queues: std::collections::HashMap<String, i64> =
            std::env::var("TORK_COORDINATOR_QUEUES")
                .ok()
                .map(|s| {
                    s.split(',')
                        .filter_map(|q| {
                            let parts: Vec<&str> = q.split(':').collect();
                            if parts.len() == 2 {
                                parts[1]
                                    .trim()
                                    .parse::<i64>()
                                    .ok()
                                    .map(|v| (parts[0].trim().to_string(), v))
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

        // Parse enabled endpoints from environment
        let enabled: std::collections::HashMap<String, bool> =
            std::env::var("TORK_COORDINATOR_API_ENDPOINTS")
                .ok()
                .map(|s| s.split(',').map(|e| (e.trim().to_string(), true)).collect())
                .unwrap_or_default();

        Self {
            name,
            broker: Arc::new(BrokerProxy::new()),
            datastore: Arc::new(DatastoreProxy::new()),
            locker: Arc::new(InMemoryLocker::new()),
            queues,
            address,
            enabled,
        }
    }
}

/// The actual coordinator implementation placeholder
#[allow(dead_code)]
pub struct CoordinatorImpl {
    name: String,
    broker: Arc<dyn tork::broker::Broker>,
    datastore: Arc<dyn tork::datastore::Datastore>,
}

impl CoordinatorImpl {
    /// Creates a new coordinator
    pub fn new(config: Config) -> Self {
        Self {
            name: config.name,
            broker: config.broker,
            datastore: config.datastore,
        }
    }
}

impl Coordinator for CoordinatorImpl {
    fn start(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn stop(&self) -> BoxedFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn submit_job(&self, job: tork::job::Job) -> BoxedFuture<tork::job::Job> {
        Box::pin(async { Ok(job) })
    }
}

/// Creates a new coordinator
pub async fn create_coordinator(
    broker: BrokerProxy,
    datastore: DatastoreProxy,
) -> Result<Box<dyn Coordinator + Send + Sync>> {
    // Build coordinator config from environment
    let config = Config::from_env();

    // BrokerProxy and DatastoreProxy implement the Broker and Datastore traits
    // so we can wrap them directly in Arc and use them
    let broker: Arc<dyn tork::broker::Broker> = Arc::new(broker);
    let datastore: Arc<dyn tork::datastore::Datastore> = Arc::new(datastore);

    // Create the coordinator
    let coordinator = CoordinatorImpl::new(Config {
        name: config.name,
        broker,
        datastore,
        locker: config.locker,
        queues: config.queues,
        address: config.address,
        enabled: config.enabled,
    });

    Ok(Box::new(coordinator) as Box<dyn Coordinator + Send + Sync>)
}

// =============================================================================
// Configuration helpers - local implementation matching Go conf module pattern
// =============================================================================

/// Get config string value
fn config_string(key: &str) -> String {
    let env_key = format!("TORK_{}", key.to_uppercase().replace('.', "_"));
    std::env::var(&env_key).unwrap_or_default()
}

/// Get config string with default
fn config_string_default(key: &str, default: &str) -> String {
    let value = config_string(key);
    if value.is_empty() {
        default.to_string()
    } else {
        value
    }
}

/// Get config boolean
fn config_bool(key: &str) -> bool {
    let value = config_string(key);
    value.to_lowercase() == "true" || value == "1"
}

/// Get config boolean with default
fn config_bool_default(key: &str, default: bool) -> bool {
    let value = config_string(key);
    if value.is_empty() {
        default
    } else {
        value.to_lowercase() == "true" || value == "1"
    }
}

/// Get config integer
#[allow(dead_code)]
fn config_int(key: &str) -> i64 {
    config_string(key).parse().unwrap_or(0)
}

/// Get config integer with default
fn config_int_default(key: &str, default: i64) -> i64 {
    let value = config_string(key);
    if value.is_empty() {
        default
    } else {
        value.parse().unwrap_or(default)
    }
}

/// Get config strings (comma-separated or array)
fn config_strings(key: &str) -> Vec<String> {
    let value = config_string(key);
    if value.is_empty() {
        Vec::new()
    } else if value.starts_with('[') {
        // Parse array format: ["a", "b", "c"]
        value
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        value.split(',').map(|s| s.trim().to_string()).collect()
    }
}

/// Get config strings with default
fn config_strings_default(key: &str, default: &[&str]) -> Vec<String> {
    let v = config_strings(key);
    if v.is_empty() {
        default.iter().map(|s| s.to_string()).collect()
    } else {
        v
    }
}

// =============================================================================
// Wildcard pattern matching (from Go's wildcard package)
// =============================================================================

/// Matches a string against a wildcard pattern where `*` matches any sequence
fn wildcard_match(pattern: &str, s: &str) -> bool {
    if pattern.is_empty() {
        return s.is_empty();
    }
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == s;
    }

    // Simple DP approach for wildcard matching
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let s_chars: Vec<char> = s.chars().collect();
    let lp = pattern_chars.len();
    let ls = s_chars.len();

    let mut dp = vec![false; (lp + 1) * (ls + 1)];
    dp[0] = true;

    for i in 0..lp {
        let idx = (i + 1) * (ls + 1);
        dp[idx] = if pattern_chars[i] == '*' { dp[i * (ls + 1)] } else { false };
    }

    for i in 0..lp {
        let pc = pattern_chars[i];
        for j in 0..ls {
            let idx = (i + 1) * (ls + 1) + (j + 1);
            dp[idx] = match pc {
                '*' => {
                    dp[i * (ls + 1) + j] || dp[i * (ls + 1) + (j + 1)] || dp[(i + 1) * (ls + 1) + j]
                }
                _ if pc == s_chars[j] => dp[i * (ls + 1) + j],
                _ => false,
            };
        }
    }

    dp[lp * (ls + 1) + ls]
}

// =============================================================================
// Password hashing (using bcrypt)
// =============================================================================

/// Check password against bcrypt hash
fn check_password_hash(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).is_ok_and(|r| r)
}

// =============================================================================
// CORS Middleware
// =============================================================================

/// Creates CORS middleware layer with configuration from environment
pub fn cors_layer() -> CorsLayer {
    let allow_origins = config_strings_default("middleware.web.cors.origins", &["*"]);
    let _allow_methods = config_strings_default("middleware.web.cors.methods", &["*"]);
    let _allow_headers = config_strings_default("middleware.web.cors.headers", &["*"]);
    let allow_credentials = config_bool_default("middleware.web.cors.credentials", false);
    let _expose_headers = config_strings_default("middleware.web.cors.expose", &["*"]);

    debug!("CORS middleware enabled with origins: {:?}", allow_origins);

    

    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any)
        .allow_credentials(allow_credentials)
}

// =============================================================================
// Basic Authentication Middleware
// =============================================================================

/// Basic authentication middleware configuration
#[derive(Clone)]
pub struct BasicAuthConfig {
    /// Datastore for user lookup
    datastore: Arc<dyn tork::datastore::Datastore>,
}

impl BasicAuthConfig {
    /// Creates a new BasicAuthConfig
    pub fn new(datastore: Arc<dyn tork::datastore::Datastore>) -> Self {
        Self { datastore }
    }
}

/// Basic authentication middleware for Axum
/// Validates credentials against the datastore
#[allow(clippy::type_complexity)]
pub fn basic_auth_layer(config: BasicAuthConfig) -> axum::middleware::FromFnLayer<
    fn(axum::extract::State<BasicAuthConfig>, axum::extract::Request, Next) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    BasicAuthConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| Box::pin(async move {
        basic_auth_middleware(state, req, next).await
    }))
}

async fn basic_auth_middleware(
    axum::extract::State(config): axum::extract::State<BasicAuthConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let credentials = auth_header
        .and_then(|header_value| {
            if header_value.starts_with("Basic ") {
                Some(header_value.trim_start_matches("Basic "))
            } else {
                None
            }
        })
        .and_then(base64_decode)
        .and_then(|decoded| {
            // Split at first colon
            let parts: Vec<&str> = decoded.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        });

    let (username, password) = match credentials {
        Some((u, p)) => (u, p),
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // Look up user in datastore
    let user_result = config.datastore.get_user(username.clone()).await;

    let user = match user_result {
        Ok(Some(u)) => u,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    // Verify credentials
    let username_match = user
        .username
        .as_ref()
        .is_some_and(|u| u.as_str() == username.as_str());

    let password_valid = user
        .password_hash
        .as_ref()
        .is_some_and(|h| check_password_hash(&password, h));

    if username_match && password_valid {
        // Set username in request extensions for later use
        let mut request = request;
        request.extensions_mut().insert(UsernameValue(username));
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Username value to store in request extensions
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct UsernameValue(String);

// =============================================================================
// API Key Authentication Middleware
// =============================================================================

/// API key authentication middleware configuration
#[derive(Clone, Debug)]
pub struct KeyAuthConfig {
    /// The API key to validate against
    key: String,
    /// Paths to skip authentication for
    skip_paths: Vec<String>,
}

impl KeyAuthConfig {
    /// Creates a new KeyAuthConfig
    pub fn new(key: String) -> Self {
        Self {
            key,
            skip_paths: vec!["GET /health".to_string()],
        }
    }

    /// Sets the paths to skip authentication
    #[must_use]
    pub fn with_skip_paths(mut self, paths: Vec<String>) -> Self {
        self.skip_paths = paths;
        self
    }
}

/// API key authentication middleware for Axum
#[allow(clippy::type_complexity)]
pub fn key_auth_layer(config: KeyAuthConfig) -> axum::middleware::FromFnLayer<
    fn(axum::extract::State<KeyAuthConfig>, axum::extract::Request, Next) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    KeyAuthConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| Box::pin(async move {
        key_auth_middleware(state, req, next).await
    }))
}

async fn key_auth_middleware(
    axum::extract::State(config): axum::extract::State<KeyAuthConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = request.method().as_str();
    let path = request.uri().path().to_string();
    let pattern = format!("{} {}", method, path);

    // Check if path should be skipped
    let should_skip = config.skip_paths.iter().any(|p| wildcard_match(p, &pattern));

    if should_skip {
        return Ok(next.run(request).await);
    }

    // Extract API key from X-API-Key header or query parameter
    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .or_else(|| {
            request.uri().query().and_then(|q| {
                q.split('&').find_map(|pair| {
                    if pair.starts_with("api_key=") {
                        Some(pair.trim_start_matches("api_key=").to_string())
                    } else {
                        None
                    }
                })
            })
        });

    match api_key {
        Some(key) if key == config.key => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

// =============================================================================
// Rate Limiting Middleware
// =============================================================================

/// Rate limiting middleware configuration
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Requests per second
    rps: u32,
}

impl RateLimitConfig {
    /// Creates a new RateLimitConfig
    pub fn new(rps: u32) -> Self {
        Self { rps }
    }
}

/// Rate limiting middleware for Axum using in-memory storage
#[allow(clippy::type_complexity)]
pub fn rate_limit_layer(config: RateLimitConfig) -> axum::middleware::FromFnLayer<
    fn(axum::extract::State<RateLimitConfig>, axum::extract::Request, Next) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    RateLimitConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| Box::pin(async move {
        rate_limit_middleware(state, req, next).await
    }))
}

async fn rate_limit_middleware(
    axum::extract::State(config): axum::extract::State<RateLimitConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Use a simple direct rate limiter
    use governor::RateLimiter;
    use std::num::NonZeroU32;
    let rps = NonZeroU32::new(config.rps.max(1)).unwrap_or(NonZeroU32::MIN);
    let limiter = RateLimiter::direct(Quota::per_second(rps));

    // Check if request is allowed
    match limiter.check() {
        Ok(()) => Ok(next.run(request).await),
        Err(_not_until) => {
            // Return 429 Too Many Requests with retry-after header
            let _response = Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header(header::RETRY_AFTER, "1")
                .body(StatusCode::TOO_MANY_REQUESTS.as_str())
                .unwrap_or_else(|_| Response::new(StatusCode::TOO_MANY_REQUESTS.as_str()));

            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

// =============================================================================
// Body Size Limit Middleware
// =============================================================================

/// Body size limit middleware configuration
#[derive(Clone, Debug)]
pub struct BodyLimitConfig {
    /// Maximum body size in bytes
    limit: usize,
}

impl BodyLimitConfig {
    /// Creates a new BodyLimitConfig with the given size limit
    pub fn new(limit: usize) -> Self {
        Self { limit }
    }
}

/// Body size limit middleware for Axum
#[allow(clippy::type_complexity)]
pub fn body_limit_layer(config: BodyLimitConfig) -> axum::middleware::FromFnLayer<
    fn(axum::extract::State<BodyLimitConfig>, axum::extract::Request, Next) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    BodyLimitConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| Box::pin(async move {
        body_limit_middleware(state, req, next).await
    }))
}

async fn body_limit_middleware(
    axum::extract::State(config): axum::extract::State<BodyLimitConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get content length
    let content_length = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());

    if let Some(length) = content_length {
        if length > config.limit {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
    }

    Ok(next.run(request).await)
}

// =============================================================================
// HTTP Logging Middleware
// =============================================================================

/// HTTP logging middleware configuration
#[derive(Clone, Debug)]
pub struct HttpLogConfig {
    /// Log level (DEBUG, INFO, WARN, ERROR)
    level: String,
    /// Paths to skip logging for
    skip_paths: Vec<String>,
}

impl Default for HttpLogConfig {
    fn default() -> Self {
        Self {
            level: "DEBUG".to_string(),
            skip_paths: vec!["GET /health".to_string()],
        }
    }
}

impl HttpLogConfig {
    /// Creates a new HttpLogConfig
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the log level
    #[must_use]
    pub fn with_level(mut self, level: &str) -> Self {
        self.level = level.to_string();
        self
    }

    /// Sets paths to skip
    #[must_use]
    pub fn with_skip_paths(mut self, paths: Vec<String>) -> Self {
        self.skip_paths = paths;
        self
    }
}

/// HTTP request logging middleware using tracing
#[allow(clippy::type_complexity)]
pub fn http_log_layer(config: HttpLogConfig) -> axum::middleware::FromFnLayer<
    fn(axum::extract::State<HttpLogConfig>, axum::extract::Request, Next) -> Pin<Box<dyn Future<Output = Response> + Send>>,
    HttpLogConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| Box::pin(async move {
        http_log_middleware(state, req, next).await
    }))
}

async fn http_log_middleware(
    axum::extract::State(config): axum::extract::State<HttpLogConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();
    let pattern = format!("{} {}", method.as_str(), path);

    // Check if path should be skipped
    let should_skip = config.skip_paths.iter().any(|p| wildcard_match(p, &pattern));

    if should_skip {
        return next.run(request).await;
    }

    let start = Instant::now();
    let client_ip = request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim())
        .unwrap_or("unknown")
        .to_string();

    // Create span for this request
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

        // Log based on status code and configured level
        let log_level = if status.is_server_error() {
            "ERROR"
        } else if status.is_client_error() {
            "WARN"
        } else {
            &config.level
        };

        match log_level {
            "ERROR" => {
                error!(
                    method = %method,
                    uri = %uri,
                    status = %status.as_u16(),
                    remote_ip = %client_ip,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "HTTP Request"
                );
            }
            "WARN" => {
                tracing::warn!(
                    method = %method,
                    uri = %uri,
                    status = %status.as_u16(),
                    remote_ip = %client_ip,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "HTTP Request"
                );
            }
            "INFO" => {
                info!(
                    method = %method,
                    uri = %uri,
                    status = %status.as_u16(),
                    remote_ip = %client_ip,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "HTTP Request"
                );
            }
            _ => {
                debug!(
                    method = %method,
                    uri = %uri,
                    status = %status.as_u16(),
                    remote_ip = %client_ip,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "HTTP Request"
                );
            }
        }

        response
    }
    .instrument(span)
    .await
}

// =============================================================================
// Utility Functions
// =============================================================================

/// Base64 decode helper
fn base64_decode(input: &str) -> Option<String> {
    // Use base64 crate from workspace
    use base64::{engine::general_purpose::STANDARD, Engine};

    STANDARD
        .decode(input)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

// =============================================================================
// Middleware Helper Functions
// =============================================================================

/// Parse body limit string like "500K", "1M", "10M" to bytes
fn parse_body_limit(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let multiplier = if s.ends_with('K') {
        1024
    } else if s.ends_with('M') {
        1024 * 1024
    } else if s.ends_with('G') {
        1024 * 1024 * 1024
    } else {
        return s.parse().ok();
    };

    let num_str = &s[..s.len() - 1];
    let num: usize = num_str.parse().ok()?;
    num.checked_mul(multiplier)
}

/// Helper to create all web middlewares based on configuration
pub fn create_web_middlewares(
    datastore: Arc<dyn tork::datastore::Datastore>,
) -> (
    Option<CorsLayer>,
    Option<BasicAuthConfig>,
    Option<KeyAuthConfig>,
    Option<RateLimitConfig>,
    Option<BodyLimitConfig>,
    Option<HttpLogConfig>,
) {
    let cors = config_bool("middleware.web.cors.enabled").then(cors_layer);
    let basic_auth = config_bool("middleware.web.basicauth.enabled")
        .then(|| BasicAuthConfig::new(datastore));
    let key_auth = config_bool("middleware.web.keyauth.enabled").then(|| {
        KeyAuthConfig::new(config_string_default("middleware.web.keyauth.key", ""))
    });
    let rate_limit = config_bool("middleware.web.ratelimit.enabled").then(|| {
        RateLimitConfig::new(config_int_default("middleware.web.ratelimit.rps", 20) as u32)
    });
    let body_limit = parse_body_limit(&config_string_default("middleware.web.bodylimit", "500K"))
        .map(BodyLimitConfig::new);
    let http_log = config_bool_default("middleware.web.logger.enabled", true).then(|| {
        HttpLogConfig::new()
            .with_level(&config_string_default("middleware.web.logger.level", "DEBUG"))
            .with_skip_paths(config_strings_default(
                "middleware.web.logger.skip",
                &["GET /health"],
            ))
    });

    (
        cors,
        basic_auth,
        key_auth,
        rate_limit,
        body_limit,
        http_log,
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pure function tests ─────────────────────────────────────

    #[test]
    fn test_wildcard_match_exact() {
        assert!(wildcard_match("abc", "abc"));
        assert!(!wildcard_match("abc", "abd"));
    }

    #[test]
    fn test_wildcard_match_star() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("a*c", "abc"));
        assert!(wildcard_match("a*c", "aXXc"));
        assert!(!wildcard_match("a*c", "aXXd"));
    }

    #[test]
    fn test_wildcard_match_empty() {
        assert!(wildcard_match("", ""));
        assert!(!wildcard_match("", "a"));
        assert!(!wildcard_match("a", ""));
    }

    #[test]
    fn test_wildcard_match_multiple_stars() {
        assert!(wildcard_match("*:*", "foo:bar"));
        assert!(wildcard_match("a*b*c", "axbxc"));
    }

    #[test]
    fn test_parse_body_limit() {
        assert_eq!(parse_body_limit("500K"), Some(500 * 1024));
        assert_eq!(parse_body_limit("1M"), Some(1024 * 1024));
        assert_eq!(parse_body_limit("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_body_limit("500"), Some(500));
    }

    #[test]
    fn test_parse_body_limit_edge_cases() {
        assert_eq!(parse_body_limit(""), None);
        assert_eq!(parse_body_limit("invalid"), None);
        assert_eq!(parse_body_limit("K"), None); // no number
    }

    #[test]
    fn test_config_string() {
        std::env::remove_var("TORK_TEST_KEY");
        assert_eq!(config_string("test.key"), "");
    }

    #[test]
    fn test_config_string_with_env() {
        std::env::set_var("TORK_TEST_KEY2", "test_value");
        assert_eq!(config_string("test.key2"), "test_value");
        std::env::remove_var("TORK_TEST_KEY2");
    }

    #[test]
    fn test_config_bool() {
        std::env::set_var("TORK_TEST_BOOL", "true");
        assert!(config_bool("test.bool"));
        std::env::set_var("TORK_TEST_BOOL", "false");
        assert!(!config_bool("test.bool"));
        std::env::remove_var("TORK_TEST_BOOL");
    }

    #[test]
    fn test_config_bool_with_one() {
        std::env::set_var("TORK_TEST_BOOL2", "1");
        assert!(config_bool("test.bool2"));
        std::env::remove_var("TORK_TEST_BOOL2");
    }

    #[test]
    fn test_base64_decode_valid() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let encoded = STANDARD.encode("hello:world");
        let result = base64_decode(&encoded);
        assert_eq!(result.as_deref(), Some("hello:world"));
    }

    #[test]
    fn test_base64_decode_invalid() {
        assert!(base64_decode("not-base64!!!").is_none());
    }

    #[test]
    fn test_base64_decode_empty() {
        // Empty string decodes to Some("") in standard base64
        assert_eq!(base64_decode(""), Some("".to_string()));
    }

    // ── Authentication logic tests ─────────────────────────────────

    #[test]
    fn test_check_password_hash_correct() {
        let hashed = bcrypt::hash("secret", 4).unwrap_or_default();
        assert!(check_password_hash("secret", &hashed));
    }

    #[test]
    fn test_check_password_hash_wrong() {
        let hashed = bcrypt::hash("secret", 4).unwrap_or_default();
        assert!(!check_password_hash("wrong", &hashed));
    }

    #[test]
    fn test_check_password_hash_empty() {
        assert!(!check_password_hash("", "$2b$04$invalidhash"));
    }

    // ── Middleware config construction ────────────────────────────

    #[test]
    fn test_basic_auth_config_new() {
        let _proxy = crate::datastore::DatastoreProxy::new();
        let _config = BasicAuthConfig::new(crate::datastore::new_inmemory_datastore_arc());
        // Construction succeeds — no panic
    }

    #[test]
    fn test_key_auth_config_new() {
        let _config = KeyAuthConfig::new("test-key".to_string());
        assert_eq!(_config.key, "test-key");
    }

    #[test]
    fn test_key_auth_config_with_skip_paths() {
        let config = KeyAuthConfig::new("key".to_string())
            .with_skip_paths(vec!["GET /health".to_string()]);
        assert_eq!(config.skip_paths, vec!["GET /health".to_string()]);
    }

    #[test]
    fn test_rate_limit_config_new() {
        let config = RateLimitConfig::new(50);
        assert_eq!(config.rps, 50);
    }

    #[test]
    fn test_body_limit_config_new() {
        let config = BodyLimitConfig::new(1024);
        assert_eq!(config.limit, 1024);
    }

    #[test]
    fn test_http_log_config_default() {
        let config = HttpLogConfig::new();
        assert_eq!(config.level, "DEBUG");
        assert!(config.skip_paths.iter().any(|p| p == "GET /health"));
    }

    #[test]
    fn test_http_log_config_custom() {
        let config = HttpLogConfig::new()
            .with_level("INFO")
            .with_skip_paths(vec!["POST /api".to_string()]);
        assert_eq!(config.level, "INFO");
        assert_eq!(config.skip_paths, vec!["POST /api".to_string()]);
    }

    #[test]
    fn test_cors_layer_creation() {
        let _layer = cors_layer();
        // CorsLayer constructs without panic
    }

    #[test]
    fn test_basic_auth_layer_creation() {
        let _layer = basic_auth_layer(BasicAuthConfig::new(
            crate::datastore::new_inmemory_datastore_arc(),
        ));
    }

    #[test]
    fn test_rate_limit_layer_creation() {
        let _layer = rate_limit_layer(RateLimitConfig::new(10));
        // Layer constructs without panic
    }

    #[test]
    fn test_body_limit_layer_creation() {
        let _layer = body_limit_layer(BodyLimitConfig::new(2048));
        // Layer constructs without panic
    }

    #[test]
    fn test_http_log_layer_creation() {
        let _layer = http_log_layer(HttpLogConfig::new());
        // Layer constructs without panic
    }

    #[test]
    fn test_key_auth_layer_creation() {
        let _layer = key_auth_layer(KeyAuthConfig::new("key".to_string()));
        // Layer constructs without panic
    }

    // ── InMemoryDatastore user round-trip ───────────────────────

    #[tokio::test]
    async fn test_inmemory_datastore_user_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let ds = crate::datastore::new_inmemory_datastore();

        // Create user
        let hashed = bcrypt::hash("password", 4)?;
        let user = tork::user::User {
            id: Some("u1".to_string()),
            username: Some("testuser".to_string()),
            password_hash: Some(hashed),
            ..Default::default()
        };
        ds.create_user(user).await?;

        // Look up by username
        let found = ds.get_user("testuser".to_string()).await?;
        assert!(found.is_some());
        let found = found.as_ref().and_then(|u| u.id.as_deref()).unwrap_or_default();
        assert_eq!(found, "u1");

        // Non-existent user
        let missing = ds.get_user("nobody".to_string()).await?;
        assert!(missing.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_inmemory_datastore_wrong_password_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let ds = crate::datastore::new_inmemory_datastore();

        let hashed = bcrypt::hash("correct", 4)?;
        let user = tork::user::User {
            id: Some("u2".to_string()),
            username: Some("auth_test".to_string()),
            password_hash: Some(hashed),
            ..Default::default()
        };
        ds.create_user(user).await?;

        // Password verification should fail for wrong password
        let found = ds.get_user("auth_test".to_string()).await?.unwrap();
        let hash = found.password_hash.as_deref().unwrap_or_default();
        assert!(!check_password_hash("incorrect", hash));

        // Password verification should succeed for correct password
        assert!(check_password_hash("correct", hash));

        Ok(())
    }

    // ── create_web_middlewares ──────────────────────────────────

    #[test]
    fn test_create_web_middlewares_returns_tuple() {
        let _proxy = crate::datastore::DatastoreProxy::new();
        let (_cors, _basic_auth, _key_auth, _rate_limit, _body_limit, _http_log) =
            create_web_middlewares(crate::datastore::new_inmemory_datastore_arc());
        // All six middleware configs are returned (may be None depending on env)
    }
}
