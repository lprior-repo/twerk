//! Rate limiting middleware for Axum.
//!
//! Uses the `governor` crate for token-bucket rate limiting.
//! Provides a tower `Layer` that wraps incoming requests with rate limiting.
//!
//! # Go Parity
//!
//! Maps to Go `middleware.NewRateLimit()`.

use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use tower::{Layer, Service};

use super::config::RateLimitConfig;

/// Direct rate limiter type used internally.
type DirectLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Tower layer for rate limiting.
///
/// Wraps services with a token-bucket rate limiter backed by the `governor` crate.
#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Arc<DirectLimiter>,
}

impl RateLimitLayer {
    /// Create a new rate limit layer with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `None` if `requests_per_second` is zero.
    pub fn new(config: &RateLimitConfig) -> Option<Self> {
        let rps = NonZeroU32::new(config.requests_per_second)?;
        let quota = Quota::per_second(rps);
        let limiter = RateLimiter::direct(quota);
        Some(Self {
            limiter: Arc::new(limiter),
        })
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// Tower service that enforces rate limits.
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: Arc<DirectLimiter>,
}

impl<S, ReqBody> Service<Request<ReqBody>> for RateLimitService<S>
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
        // Check rate limit before processing
        match self.limiter.check() {
            Ok(_) => {
                // Rate limit not exceeded — forward to inner service
                let fut = self.inner.call(req);
                Box::pin(fut)
            }
            Err(_not_until) => {
                // Rate limit exceeded — return 429 Too Many Requests
                Box::pin(async {
                    let response = Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .body(Body::from(r#"{"error":"rate limit exceeded"}"#))
                        .unwrap_or_else(|_| {
                            Response::builder()
                                .status(StatusCode::TOO_MANY_REQUESTS)
                                .body(Body::empty())
                                .expect("valid 429 response")
                        });
                    Ok(response)
                })
            }
        }
    }
}

/// Create a rate limit layer from config, or `None` if config is invalid (zero RPS).
#[must_use]
pub fn rate_limit_layer(config: &RateLimitConfig) -> Option<RateLimitLayer> {
    RateLimitLayer::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_layer_zero_rps() {
        let config = RateLimitConfig {
            requests_per_second: 0,
        };
        assert!(rate_limit_layer(&config).is_none());
    }

    #[test]
    fn test_rate_limit_layer_valid_rps() {
        let config = RateLimitConfig {
            requests_per_second: 10,
        };
        assert!(rate_limit_layer(&config).is_some());
    }

    #[test]
    fn test_rate_limit_layer_default() {
        let config = RateLimitConfig::default();
        assert!(rate_limit_layer(&config).is_some());
    }
}
