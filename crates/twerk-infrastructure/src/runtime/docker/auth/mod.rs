//! Docker registry authentication following functional-rust conventions.
//!
//! # Architecture
//!
//! - **Data**: `Config`, `AuthConfig` structs hold authentication data
//! - **Calc**: Pure credential resolution logic
//! - **Actions**: File I/O and subprocess execution at boundary

mod auth_config;
mod auth_resolver;
mod config;
mod credential_helper;

use thiserror::Error;

pub use auth_config::{AuthConfig, Config, KubernetesConfig, ProxyConfig};
pub use auth_resolver::{decode_base64_auth, get_registry_credentials, resolve_auth_config};
pub use config::{config_path, user_home_config_path, ConfigError};
pub use credential_helper::{get_from_helper, CredentialHelperError, Credentials};

// ----------------------------------------------------------------------------
// Domain Errors
// ----------------------------------------------------------------------------

/// Errors that can occur during authentication operations.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("credentials not found in native keychain")]
    CredentialsNotFound,

    #[error("no credentials server URL")]
    CredentialsMissingServerUrl,

    #[error("invalid auth string format")]
    InvalidAuthString,

    #[error("decoded value longer than expected: expected {expected}, got {actual}")]
    DecodedLengthMismatch { expected: usize, actual: usize },

    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("credential helper error: {0}")]
    CredentialHelper(String),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}
