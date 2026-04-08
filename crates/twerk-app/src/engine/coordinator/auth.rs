//! Authentication middleware for the coordinator

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use crate::engine::coordinator::utils::{base64_decode, check_password_hash, wildcard_match};
use anyhow::Result;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tracing::error;
pub use twerk_core::user::UsernameValue;

// ── Basic Authentication ───────────────────────────────────────

#[derive(Clone)]
pub struct BasicAuthConfig {
    pub(crate) datastore: Arc<dyn twerk_infrastructure::datastore::Datastore>,
}

impl BasicAuthConfig {
    pub fn new(datastore: Arc<dyn twerk_infrastructure::datastore::Datastore>) -> Self {
        Self { datastore }
    }
}

/// Basic authentication middleware for coordinator endpoints.
/// # Errors
/// Returns `StatusCode::UNAUTHORIZED` if authentication fails or user not found.
/// Returns `StatusCode::INTERNAL_SERVER_ERROR` if datastore error occurs.
pub async fn basic_auth_middleware(
    axum::extract::State(config): axum::extract::State<BasicAuthConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let credentials = auth_header
        .and_then(|header_value| header_value.strip_prefix("Basic "))
        .and_then(base64_decode)
        .and_then(|decoded| {
            let parts: Vec<&str> = decoded.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        });

    let Some((username, password)) = credentials else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let user = match config.datastore.get_user(&username).await {
        Ok(u) => u,
        Err(twerk_infrastructure::datastore::Error::UserNotFound) => {
            return Err(StatusCode::UNAUTHORIZED)
        }
        Err(e) => {
            error!("error getting user: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let password_valid = user
        .password_hash
        .as_ref()
        .is_some_and(|h| check_password_hash(&password, h));

    if user.username.as_ref() == Some(&username) && password_valid {
        let mut request = request;
        request.extensions_mut().insert(UsernameValue(username));
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// ── API Key Authentication ─────────────────────────────────────

#[derive(Clone, Debug)]
pub struct KeyAuthConfig {
    pub(crate) key: String,
    pub(crate) skip_paths: Vec<String>,
}

impl KeyAuthConfig {
    #[must_use]
    pub fn new(key: String) -> Self {
        Self {
            key,
            skip_paths: vec!["GET /health".to_string()],
        }
    }

    #[must_use]
    pub fn with_skip_paths(mut self, paths: Vec<String>) -> Self {
        self.skip_paths = paths;
        self
    }
}

/// API key authentication middleware for coordinator endpoints.
/// # Errors
/// Returns `StatusCode::UNAUTHORIZED` if API key is missing or invalid.
pub async fn key_auth_middleware(
    axum::extract::State(config): axum::extract::State<KeyAuthConfig>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = request.method().as_str();
    let path = request.uri().path();
    let pattern = format!("{method} {path}");

    if config
        .skip_paths
        .iter()
        .any(|p| wildcard_match(p, &pattern))
    {
        return Ok(next.run(request).await);
    }

    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .or_else(|| {
            request.uri().query().and_then(|q| {
                q.split('&')
                    .find_map(|pair| pair.strip_prefix("api_key=").map(String::from))
            })
        });

    let key_valid = api_key
        .as_ref()
        .is_some_and(|key| key.as_bytes().ct_eq(config.key.as_bytes()).into());

    if key_valid {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[allow(clippy::type_complexity)]
pub fn basic_auth_layer(
    config: BasicAuthConfig,
) -> axum::middleware::FromFnLayer<
    fn(
        axum::extract::State<BasicAuthConfig>,
        axum::extract::Request,
        Next,
    ) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    BasicAuthConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| {
        Box::pin(async move { basic_auth_middleware(state, req, next).await })
    })
}

#[allow(clippy::type_complexity)]
pub fn key_auth_layer(
    config: KeyAuthConfig,
) -> axum::middleware::FromFnLayer<
    fn(
        axum::extract::State<KeyAuthConfig>,
        axum::extract::Request,
        Next,
    ) -> Pin<Box<dyn Future<Output = Result<Response, StatusCode>> + Send>>,
    KeyAuthConfig,
    Pin<Box<dyn Future<Output = Response> + Send>>,
> {
    axum::middleware::from_fn_with_state(config, move |state, req, next| {
        Box::pin(async move { key_auth_middleware(state, req, next).await })
    })
}
