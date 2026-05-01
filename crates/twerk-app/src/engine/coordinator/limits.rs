//! Rate and body size limiting middleware for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use governor::{Quota, RateLimiter};
use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;

// ── Rate Limiting ──────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub(crate) rps: u32,
}

impl RateLimitConfig {
    #[must_use]
    pub fn new(rps: u32) -> Self {
        Self { rps }
    }
}

fn retry_after_header_value(seconds: u64) -> HeaderValue {
    HeaderValue::from_str(&seconds.to_string()).unwrap_or(HeaderValue::from_static("1"))
}

/// Rate limiting middleware that enforces requests per second quota.
/// # Errors
/// Returns `StatusCode::TOO_MANY_REQUESTS` with `Retry-After` header if rate limit is exceeded.
pub async fn rate_limit_middleware(
    axum::extract::State(config): axum::extract::State<RateLimitConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let rps = NonZeroU32::new(config.rps.max(1)).unwrap_or(NonZeroU32::MIN);
    let limiter = RateLimiter::direct(Quota::per_second(rps));

    match limiter.check() {
        Ok(()) => Ok(next.run(request).await),
        Err(not_until) => {
            let retry_after = not_until
                .retry_after()
                .as_secs()
                .max(1);
            let mut response = Response::new(axum::body::Body::empty());
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            response.headers_mut().insert(
                header::RETRY_AFTER,
                retry_after_header_value(retry_after),
            );
            Ok(response)
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn rate_limit_layer(
    config: RateLimitConfig,
) -> axum::middleware::FromFnLayer<
    fn(
        axum::extract::State<RateLimitConfig>,
        axum::extract::Request,
        Next,
    ) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    RateLimitConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| {
        Box::pin(async move { rate_limit_middleware(state, req, next).await })
    })
}

// ── Body Size Limit ────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct BodyLimitConfig {
    pub(crate) limit: usize,
}

impl BodyLimitConfig {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self { limit }
    }
}

/// Body size limiting middleware that enforces maximum content length.
/// # Errors
/// Returns `StatusCode::PAYLOAD_TOO_LARGE` if content length exceeds limit.
/// Returns `StatusCode::LENGTH_REQUIRED` if Transfer-Encoding: chunked is present.
pub async fn body_limit_middleware(
    axum::extract::State(config): axum::extract::State<BodyLimitConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let is_chunked = request
        .headers()
        .get(header::TRANSFER_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("chunked"));

    if is_chunked {
        return Err(StatusCode::LENGTH_REQUIRED);
    }

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

#[allow(clippy::type_complexity)]
pub fn body_limit_layer(
    config: BodyLimitConfig,
) -> axum::middleware::FromFnLayer<
    fn(
        axum::extract::State<BodyLimitConfig>,
        axum::extract::Request,
        Next,
    ) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    BodyLimitConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| {
        Box::pin(async move { body_limit_middleware(state, req, next).await })
    })
}
