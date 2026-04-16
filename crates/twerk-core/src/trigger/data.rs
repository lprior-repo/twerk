//! Trigger DATA types for defining triggers.
//!
//! These types are distinct from the runtime `Trigger`/`TriggerState` types in `types.rs`.
//! This module contains DATA types for constructing and serializing trigger configurations.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

pub use crate::domain_types::{CronError, CronExpression, GoDuration, GoDurationError};
pub use crate::id::{IdError, TriggerId};

// =============================================================================
// TriggerDataError - Error types for trigger data construction
// =============================================================================

/// Errors that can occur during trigger data construction or validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TriggerDataError {
    #[error("invalid trigger ID: {0}")]
    InvalidTriggerId(#[from] IdError),

    #[error("invalid cron expression: {0}")]
    InvalidCronExpression(#[from] CronError),

    #[error("invalid interval: {0}")]
    InvalidInterval(#[from] GoDurationError),

    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),

    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("invalid HTTP method: {0}")]
    InvalidHttpMethod(String),

    #[error("empty required field: {0}")]
    EmptyRequiredField(String),

    #[error("invalid JMESPath expression: {0}")]
    InvalidJmespath(String),

    #[error("header limit exceeded: {0}")]
    HeaderLimitExceeded(String),
}

// =============================================================================
// HttpMethod - HTTP method enum
// =============================================================================

/// HTTP methods supported by webhook and polling triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    /// Parses an HTTP method string (case-insensitive).
    ///
    /// # Parameters
    /// - `s: impl Into<String>` - The string to parse (e.g., "GET", "post", "PuT")
    ///
    /// # Returns
    /// - `Ok(HttpMethod)` on valid HTTP method
    /// - `Err(TriggerDataError::InvalidHttpMethod)` on invalid method
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: impl Into<String>) -> Result<HttpMethod, TriggerDataError> {
        let s = s.into();
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            "PATCH" => Ok(HttpMethod::Patch),
            _ => Err(TriggerDataError::InvalidHttpMethod(s)),
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpMethod::Get => write!(f, "GET"),
            HttpMethod::Post => write!(f, "POST"),
            HttpMethod::Put => write!(f, "PUT"),
            HttpMethod::Delete => write!(f, "DELETE"),
            HttpMethod::Patch => write!(f, "PATCH"),
        }
    }
}

impl FromStr for HttpMethod {
    type Err = TriggerDataError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

// =============================================================================
// WebhookAuth - Authentication configuration for webhooks
// =============================================================================

/// Authentication configuration for webhook triggers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum WebhookAuth {
    #[default]
    None,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        key: String,
        value: String,
        header_name: String,
    },
}

impl WebhookAuth {
    /// Creates a `WebhookAuth::None` variant.
    #[must_use]
    pub fn new_none() -> WebhookAuth {
        WebhookAuth::None
    }

    /// Creates a `WebhookAuth::None` variant (alias for `new_none`).
    #[must_use]
    pub fn none() -> WebhookAuth {
        WebhookAuth::None
    }

    /// Creates a `WebhookAuth::Basic` variant with username and password.
    ///
    /// # Parameters
    /// - `username: impl Into<String>` - Username (required, non-empty)
    /// - `password: impl Into<String>` - Password (required, non-empty)
    ///
    /// # Returns
    /// - `Ok(WebhookAuth::Basic { username, password })` when both fields are non-empty
    /// - `Err(TriggerDataError::EmptyRequiredField)` when either field is empty
    pub fn new_basic(
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<WebhookAuth, TriggerDataError> {
        let username = username.into();
        let password = password.into();

        if username.is_empty() {
            return Err(TriggerDataError::EmptyRequiredField(
                "basic_auth_username".to_string(),
            ));
        }
        if password.is_empty() {
            return Err(TriggerDataError::EmptyRequiredField(
                "basic_auth_password".to_string(),
            ));
        }

        Ok(WebhookAuth::Basic { username, password })
    }

    /// Creates a `WebhookAuth::Bearer` variant with a token.
    ///
    /// # Parameters
    /// - `token: impl Into<String>` - Bearer token (required, non-empty)
    ///
    /// # Returns
    /// - `Ok(WebhookAuth::Bearer { token })` when token is non-empty
    /// - `Err(TriggerDataError::EmptyRequiredField("bearer_token"))` when token is empty
    pub fn new_bearer(token: impl Into<String>) -> Result<WebhookAuth, TriggerDataError> {
        let token = token.into();

        if token.is_empty() {
            return Err(TriggerDataError::EmptyRequiredField(
                "bearer_token".to_string(),
            ));
        }

        Ok(WebhookAuth::Bearer { token })
    }

    /// Creates a `WebhookAuth::ApiKey` variant with key, value, and header_name.
    ///
    /// # Parameters
    /// - `key: impl Into<String>` - Key identifier (required, non-empty)
    /// - `value: impl Into<String>` - API key value (required, non-empty)
    /// - `header_name: impl Into<String>` - HTTP header name (required, non-empty)
    ///
    /// # Returns
    /// - `Ok(WebhookAuth::ApiKey { key, value, header_name })` when all fields are valid
    /// - `Err(TriggerDataError::EmptyRequiredField)` when any field is empty
    pub fn new_api_key(
        key: impl Into<String>,
        value: impl Into<String>,
        header_name: impl Into<String>,
    ) -> Result<WebhookAuth, TriggerDataError> {
        let key = key.into();
        let value = value.into();
        let header_name = header_name.into();

        if key.is_empty() {
            return Err(TriggerDataError::EmptyRequiredField(
                "apikey_key".to_string(),
            ));
        }
        if value.is_empty() {
            return Err(TriggerDataError::EmptyRequiredField(
                "apikey_value".to_string(),
            ));
        }
        if header_name.is_empty() {
            return Err(TriggerDataError::EmptyRequiredField(
                "apikey_header_name".to_string(),
            ));
        }

        Ok(WebhookAuth::ApiKey {
            key,
            value,
            header_name,
        })
    }
}

// =============================================================================
// CronTrigger - Cron-based trigger configuration
// =============================================================================

/// Normalizes timezone shorthand "Z" to "UTC".
fn normalize_timezone(tz: &str) -> String {
    if tz == "Z" {
        "UTC".to_string()
    } else {
        tz.to_string()
    }
}

/// Validates a timezone string using chrono-tz.
fn validate_timezone(tz: &str) -> Result<(), TriggerDataError> {
    use chrono_tz::Tz;
    tz.parse::<Tz>()
        .map(|_| ())
        .map_err(|_| TriggerDataError::InvalidTimezone(tz.to_string()))
}

/// Cron-based trigger configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CronTrigger {
    pub id: TriggerId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub cron: CronExpression,
    pub timezone: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Intermediate struct for CronTrigger deserialization with validation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CronTriggerRaw {
    id: TriggerId,
    name: Option<String>,
    description: Option<String>,
    cron: CronExpression,
    timezone: String,
    #[serde(default)]
    disabled: bool,
    payload: Option<serde_json::Value>,
}

impl From<CronTriggerRaw> for CronTrigger {
    fn from(raw: CronTriggerRaw) -> Self {
        CronTrigger {
            id: raw.id,
            name: raw.name,
            description: raw.description,
            cron: raw.cron,
            timezone: normalize_timezone(&raw.timezone),
            disabled: raw.disabled,
            payload: raw.payload,
        }
    }
}

impl<'de> Deserialize<'de> for CronTrigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = CronTriggerRaw::deserialize(deserializer)?;
        validate_timezone(&raw.timezone).map_err(de::Error::custom)?;
        Ok(CronTrigger::from(raw))
    }
}

impl CronTrigger {
    /// Constructs a new `CronTrigger`.
    ///
    /// # Parameters
    /// - `id: impl Into<String>` - Trigger ID (3-64 chars, alphanumeric/-/_)
    /// - `name: Option<String>` - Optional human-readable name
    /// - `description: Option<String>` - Optional description
    /// - `cron: impl Into<String>` - Cron expression (e.g., "0 0 * * * *")
    /// - `timezone: impl Into<String>` - IANA timezone string (e.g., "UTC", "Z", "America/New_York")
    /// - `disabled: bool` - Whether trigger is disabled (default: false)
    /// - `payload: Option<serde_json::Value>` - Optional JSON payload
    ///
    /// # Returns
    /// - `Ok(CronTrigger { ... })` when all validations pass
    /// - `Err(TriggerDataError::InvalidTriggerId)` when `id` is invalid
    /// - `Err(TriggerDataError::InvalidCronExpression)` when `cron` is invalid
    /// - `Err(TriggerDataError::InvalidTimezone)` when `timezone` is not a valid IANA timezone
    pub fn new(
        id: impl Into<String>,
        name: Option<String>,
        description: Option<String>,
        cron: impl Into<String>,
        timezone: impl Into<String>,
        disabled: bool,
        payload: Option<serde_json::Value>,
    ) -> Result<CronTrigger, TriggerDataError> {
        let id = TriggerId::new(id)?;
        let cron = CronExpression::new(cron)?;
        let timezone = normalize_timezone(&timezone.into());

        validate_timezone(&timezone)?;

        Ok(CronTrigger {
            id,
            name,
            description,
            cron,
            timezone,
            disabled,
            payload,
        })
    }
}

// =============================================================================
// WebhookTrigger - Webhook-based trigger configuration
// =============================================================================

/// Validates that a URL has a valid HTTP/HTTPS scheme.
fn validate_url(url: &str) -> Result<(), TriggerDataError> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(TriggerDataError::InvalidUrl(url.to_string()));
    }
    Ok(())
}

/// Validates header limits:
/// - Maximum 64 header entries
/// - Maximum 512 bytes per header name
/// - Maximum 8192 bytes per header value
fn validate_headers(headers: &HashMap<String, String>) -> Result<(), TriggerDataError> {
    if headers.len() > 64 {
        return Err(TriggerDataError::HeaderLimitExceeded(
            "header count exceeds 64".to_string(),
        ));
    }

    for (name, value) in headers {
        if name.len() > 512 {
            return Err(TriggerDataError::HeaderLimitExceeded(
                "header name exceeds 512 bytes".to_string(),
            ));
        }
        if value.len() > 8192 {
            return Err(TriggerDataError::HeaderLimitExceeded(
                "header value exceeds 8192 bytes".to_string(),
            ));
        }
    }

    Ok(())
}

/// Validates that WebhookAuth variant has all required non-empty fields.
fn validate_webhook_auth(auth: &WebhookAuth) -> Result<(), TriggerDataError> {
    match auth {
        WebhookAuth::None => Ok(()),
        WebhookAuth::Basic { username, password } => {
            if username.is_empty() {
                return Err(TriggerDataError::EmptyRequiredField(
                    "basic_auth_username".to_string(),
                ));
            }
            if password.is_empty() {
                return Err(TriggerDataError::EmptyRequiredField(
                    "basic_auth_password".to_string(),
                ));
            }
            Ok(())
        }
        WebhookAuth::Bearer { token } => {
            if token.is_empty() {
                return Err(TriggerDataError::EmptyRequiredField(
                    "bearer_token".to_string(),
                ));
            }
            Ok(())
        }
        WebhookAuth::ApiKey {
            key,
            value,
            header_name,
        } => {
            if key.is_empty() {
                return Err(TriggerDataError::EmptyRequiredField(
                    "apikey_key".to_string(),
                ));
            }
            if value.is_empty() {
                return Err(TriggerDataError::EmptyRequiredField(
                    "apikey_value".to_string(),
                ));
            }
            if header_name.is_empty() {
                return Err(TriggerDataError::EmptyRequiredField(
                    "apikey_header_name".to_string(),
                ));
            }
            Ok(())
        }
    }
}

/// Validates a WebhookTrigger's domain invariants.
fn validate_webhook(
    url: &str,
    auth: &WebhookAuth,
    headers: &Option<HashMap<String, String>>,
) -> Result<(), TriggerDataError> {
    validate_url(url)?;
    validate_webhook_auth(auth)?;
    if let Some(h) = headers {
        validate_headers(h)?;
    }
    Ok(())
}

/// Webhook-based trigger configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookTrigger {
    pub id: TriggerId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub url: String,
    pub method: HttpMethod,
    #[serde(default)]
    pub auth: WebhookAuth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(default)]
    pub disabled: bool,
}

/// Intermediate struct for WebhookTrigger deserialization with validation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebhookTriggerRaw {
    id: TriggerId,
    name: Option<String>,
    description: Option<String>,
    url: String,
    method: HttpMethod,
    #[serde(default)]
    auth: WebhookAuth,
    headers: Option<HashMap<String, String>>,
    body_template: Option<String>,
    payload: Option<serde_json::Value>,
    #[serde(default)]
    disabled: bool,
}

impl From<WebhookTriggerRaw> for WebhookTrigger {
    fn from(raw: WebhookTriggerRaw) -> Self {
        WebhookTrigger {
            id: raw.id,
            name: raw.name,
            description: raw.description,
            url: raw.url,
            method: raw.method,
            auth: raw.auth,
            headers: raw.headers,
            body_template: raw.body_template,
            payload: raw.payload,
            disabled: raw.disabled,
        }
    }
}

impl<'de> Deserialize<'de> for WebhookTrigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = WebhookTriggerRaw::deserialize(deserializer)?;
        validate_webhook(&raw.url, &raw.auth, &raw.headers).map_err(de::Error::custom)?;
        Ok(WebhookTrigger::from(raw))
    }
}

impl WebhookTrigger {
    /// Validates the raw fields of a WebhookTrigger.
    fn validate(
        id: &str,
        url: &str,
        auth: &WebhookAuth,
        headers: &Option<HashMap<String, String>>,
    ) -> Result<TriggerId, TriggerDataError> {
        let id = TriggerId::new(id)?;
        validate_webhook(url, auth, headers)?;
        Ok(id)
    }

    /// Constructs a new `WebhookTrigger`.
    ///
    /// # Parameters
    /// - `id: impl Into<String>` - Trigger ID (3-64 chars, alphanumeric/-/_)
    /// - `name: Option<String>` - Optional human-readable name
    /// - `description: Option<String>` - Optional description
    /// - `url: impl Into<String>` - Webhook endpoint URL (must start with `http://` or `https://`)
    /// - `method: HttpMethod` - HTTP method
    /// - `auth: WebhookAuth` - Authentication config (default: `WebhookAuth::None`)
    /// - `headers: Option<HashMap<String, String>>` - Custom HTTP headers
    /// - `body_template: Option<String>` - Optional body template string
    /// - `payload: Option<serde_json::Value>` - Optional JSON payload
    /// - `disabled: bool` - Whether trigger is disabled (default: false)
    ///
    /// # Returns
    /// - `Ok(WebhookTrigger { ... })` when all validations pass
    /// - `Err(TriggerDataError::InvalidTriggerId)` when `id` is invalid
    /// - `Err(TriggerDataError::InvalidUrl)` when `url` is not valid HTTP(S) URL
    /// - `Err(TriggerDataError::EmptyRequiredField)` when auth variant requires non-empty field
    /// - `Err(TriggerDataError::HeaderLimitExceeded)` when header limits are exceeded
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        name: Option<String>,
        description: Option<String>,
        url: impl Into<String>,
        method: HttpMethod,
        auth: WebhookAuth,
        headers: Option<HashMap<String, String>>,
        body_template: Option<String>,
        payload: Option<serde_json::Value>,
        disabled: bool,
    ) -> Result<WebhookTrigger, TriggerDataError> {
        let id = id.into();
        let url = url.into();

        Self::validate(&id, &url, &auth, &headers)?;

        Ok(WebhookTrigger {
            id: TriggerId::new(id)?,
            name,
            description,
            url,
            method,
            auth,
            headers,
            body_template,
            payload,
            disabled,
        })
    }
}

// =============================================================================
// PollingTrigger - Polling-based trigger configuration
// =============================================================================

/// Validates a JMESPath expression by attempting to compile it.
fn validate_jmespath(expr: &str) -> Result<(), TriggerDataError> {
    jmespath::compile(expr)
        .map(|_| ())
        .map_err(|e| TriggerDataError::InvalidJmespath(e.to_string()))
}

/// Validates a polling interval is non-zero.
fn validate_interval_nonzero(interval: &GoDuration) -> Result<(), TriggerDataError> {
    if interval.to_duration() == std::time::Duration::ZERO {
        return Err(TriggerDataError::InvalidInterval(
            GoDurationError::ZeroDuration,
        ));
    }
    Ok(())
}

/// Validates a polling timeout is non-zero if present.
fn validate_timeout_nonzero(timeout: &Option<GoDuration>) -> Result<(), TriggerDataError> {
    if let Some(t) = timeout {
        if t.to_duration() == std::time::Duration::ZERO {
            return Err(TriggerDataError::InvalidInterval(
                GoDurationError::ZeroDuration,
            ));
        }
    }
    Ok(())
}

/// Validates a PollingTrigger's domain invariants.
fn validate_polling(
    url: &str,
    auth: &WebhookAuth,
    headers: &Option<HashMap<String, String>>,
    interval: &GoDuration,
    timeout: &Option<GoDuration>,
    stop_condition: &Option<String>,
) -> Result<(), TriggerDataError> {
    validate_url(url)?;
    validate_webhook_auth(auth)?;
    validate_interval_nonzero(interval)?;
    validate_timeout_nonzero(timeout)?;
    if let Some(sc) = stop_condition {
        validate_jmespath(sc)?;
    }
    if let Some(h) = headers {
        validate_headers(h)?;
    }
    Ok(())
}

/// Polling-based trigger configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollingTrigger {
    pub id: TriggerId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub url: String,
    pub method: HttpMethod,
    #[serde(default)]
    pub auth: WebhookAuth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    pub interval: GoDuration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<GoDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(default)]
    pub disabled: bool,
}

/// Intermediate struct for PollingTrigger deserialization with validation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PollingTriggerRaw {
    id: TriggerId,
    name: Option<String>,
    description: Option<String>,
    url: String,
    method: HttpMethod,
    #[serde(default)]
    auth: WebhookAuth,
    headers: Option<HashMap<String, String>>,
    interval: GoDuration,
    timeout: Option<GoDuration>,
    stop_condition: Option<String>,
    payload: Option<serde_json::Value>,
    #[serde(default)]
    disabled: bool,
}

impl From<PollingTriggerRaw> for PollingTrigger {
    fn from(raw: PollingTriggerRaw) -> Self {
        PollingTrigger {
            id: raw.id,
            name: raw.name,
            description: raw.description,
            url: raw.url,
            method: raw.method,
            auth: raw.auth,
            headers: raw.headers,
            interval: raw.interval,
            timeout: raw.timeout,
            stop_condition: raw.stop_condition,
            payload: raw.payload,
            disabled: raw.disabled,
        }
    }
}

impl<'de> Deserialize<'de> for PollingTrigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = PollingTriggerRaw::deserialize(deserializer)?;
        validate_polling(
            &raw.url,
            &raw.auth,
            &raw.headers,
            &raw.interval,
            &raw.timeout,
            &raw.stop_condition,
        )
        .map_err(de::Error::custom)?;
        Ok(PollingTrigger::from(raw))
    }
}

impl PollingTrigger {
    /// Validates the raw fields of a PollingTrigger.
    fn validate(
        id: &str,
        url: &str,
        auth: &WebhookAuth,
        headers: &Option<HashMap<String, String>>,
        interval: &GoDuration,
        timeout: &Option<GoDuration>,
        stop_condition: &Option<String>,
    ) -> Result<TriggerId, TriggerDataError> {
        let id = TriggerId::new(id)?;
        validate_polling(url, auth, headers, interval, timeout, stop_condition)?;
        Ok(id)
    }

    /// Constructs a new `PollingTrigger`.
    ///
    /// # Parameters
    /// - `id: impl Into<String>` - Trigger ID (3-64 chars, alphanumeric/-/_)
    /// - `name: Option<String>` - Optional human-readable name
    /// - `description: Option<String>` - Optional description
    /// - `url: impl Into<String>` - Polling endpoint URL (must start with `http://` or `https://`)
    /// - `method: HttpMethod` - HTTP method
    /// - `auth: WebhookAuth` - Authentication config (default: `WebhookAuth::None`)
    /// - `headers: Option<HashMap<String, String>>` - Custom HTTP headers
    /// - `interval: impl Into<String>` - Polling interval in GoDuration format (e.g., "30s", "1m", "1h")
    /// - `timeout: Option<GoDuration>` - Optional request timeout
    /// - `stop_condition: Option<String>` - Optional JMESPath expression to stop polling
    /// - `payload: Option<serde_json::Value>` - Optional JSON payload
    /// - `disabled: bool` - Whether trigger is disabled (default: false)
    ///
    /// # Returns
    /// - `Ok(PollingTrigger { ... })` when all validations pass
    /// - `Err(TriggerDataError::InvalidTriggerId)` when `id` is invalid
    /// - `Err(TriggerDataError::InvalidUrl)` when `url` is not valid HTTP(S) URL
    /// - `Err(TriggerDataError::InvalidInterval)` when `interval` is not valid GoDuration or is zero
    /// - `Err(TriggerDataError::InvalidInterval)` when `timeout` is present but not valid GoDuration
    /// - `Err(TriggerDataError::InvalidJmespath)` when `stop_condition` is present but invalid JMESPath
    /// - `Err(TriggerDataError::EmptyRequiredField)` when auth variant requires non-empty field
    /// - `Err(TriggerDataError::HeaderLimitExceeded)` when header limits are exceeded
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        name: Option<String>,
        description: Option<String>,
        url: impl Into<String>,
        method: HttpMethod,
        auth: WebhookAuth,
        headers: Option<HashMap<String, String>>,
        interval: impl Into<String>,
        timeout: Option<GoDuration>,
        stop_condition: Option<String>,
        payload: Option<serde_json::Value>,
        disabled: bool,
    ) -> Result<PollingTrigger, TriggerDataError> {
        let id = id.into();
        let url = url.into();
        let interval_str: String = interval.into();
        let interval = GoDuration::new(&interval_str)?;

        Self::validate(
            &id,
            &url,
            &auth,
            &headers,
            &interval,
            &timeout,
            &stop_condition,
        )?;

        Ok(PollingTrigger {
            id: TriggerId::new(id)?,
            name,
            description,
            url,
            method,
            auth,
            headers,
            interval,
            timeout,
            stop_condition,
            payload,
            disabled,
        })
    }
}

// =============================================================================
// Trigger - Root enum for all trigger types
// =============================================================================

/// Root enum for trigger types with polymorphic deserialization support.
///
/// The `#[serde(tag = "type")]` attribute ensures JSON has `"type": "Cron"|"Webhook"|"Polling"`
/// discriminant field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Trigger {
    Cron(CronTrigger),
    Webhook(WebhookTrigger),
    Polling(PollingTrigger),
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- HttpMethod tests -----------------------------------------------------

    #[test]
    fn httpmethod_from_str_returns_get_when_input_is_get_uppercase() {
        assert_eq!(HttpMethod::from_str("GET"), Ok(HttpMethod::Get));
    }

    #[test]
    fn httpmethod_from_str_returns_get_when_input_is_get_lowercase() {
        assert_eq!(HttpMethod::from_str("get"), Ok(HttpMethod::Get));
    }

    #[test]
    fn httpmethod_from_str_returns_post_when_input_is_post_uppercase() {
        assert_eq!(HttpMethod::from_str("POST"), Ok(HttpMethod::Post));
    }

    #[test]
    fn httpmethod_from_str_returns_put_when_input_is_put_uppercase() {
        assert_eq!(HttpMethod::from_str("PUT"), Ok(HttpMethod::Put));
    }

    #[test]
    fn httpmethod_from_str_returns_delete_when_input_is_delete_uppercase() {
        assert_eq!(HttpMethod::from_str("DELETE"), Ok(HttpMethod::Delete));
    }

    #[test]
    fn httpmethod_from_str_returns_patch_when_input_is_patch_uppercase() {
        assert_eq!(HttpMethod::from_str("PATCH"), Ok(HttpMethod::Patch));
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_options() {
        assert_eq!(
            HttpMethod::from_str("OPTIONS"),
            Err(TriggerDataError::InvalidHttpMethod("OPTIONS".to_string()))
        );
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_empty() {
        assert_eq!(
            HttpMethod::from_str(""),
            Err(TriggerDataError::InvalidHttpMethod("".to_string()))
        );
    }

    // -- WebhookAuth tests -----------------------------------------------------

    #[test]
    fn webhookauth_new_none_returns_none_variant() {
        assert_eq!(WebhookAuth::new_none(), WebhookAuth::None);
    }

    #[test]
    fn webhookauth_none_method_equivalents_new_none() {
        assert_eq!(WebhookAuth::none(), WebhookAuth::new_none());
    }

    #[test]
    fn webhookauth_new_basic_returns_basic_when_username_and_password_are_non_empty() {
        let result = WebhookAuth::new_basic("admin", "secret");
        assert_eq!(
            result,
            Ok(WebhookAuth::Basic {
                username: "admin".to_string(),
                password: "secret".to_string()
            })
        );
    }

    #[test]
    fn webhookauth_new_basic_returns_empty_required_field_when_username_is_empty() {
        let result = WebhookAuth::new_basic("", "secret");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "basic_auth_username".to_string()
            ))
        );
    }

    #[test]
    fn webhookauth_new_basic_returns_empty_required_field_when_password_is_empty() {
        let result = WebhookAuth::new_basic("admin", "");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "basic_auth_password".to_string()
            ))
        );
    }

    #[test]
    fn webhookauth_new_bearer_returns_bearer_when_token_is_non_empty() {
        let result = WebhookAuth::new_bearer("eyJhbGciOiJIUzI1NiJ9");
        assert_eq!(
            result,
            Ok(WebhookAuth::Bearer {
                token: "eyJhbGciOiJIUzI1NiJ9".to_string()
            })
        );
    }

    #[test]
    fn webhookauth_new_bearer_returns_empty_required_field_when_token_is_empty() {
        assert_eq!(
            WebhookAuth::new_bearer(""),
            Err(TriggerDataError::EmptyRequiredField(
                "bearer_token".to_string()
            ))
        );
    }

    #[test]
    fn webhookauth_new_api_key_returns_api_key_when_all_fields_non_empty() {
        let result = WebhookAuth::new_api_key("api-key", "abc123", "X-API-Key");
        assert_eq!(
            result,
            Ok(WebhookAuth::ApiKey {
                key: "api-key".to_string(),
                value: "abc123".to_string(),
                header_name: "X-API-Key".to_string()
            })
        );
    }

    #[test]
    fn webhookauth_new_api_key_returns_empty_required_field_when_key_is_empty() {
        assert_eq!(
            WebhookAuth::new_api_key("", "abc123", "X-API-Key"),
            Err(TriggerDataError::EmptyRequiredField(
                "apikey_key".to_string()
            ))
        );
    }

    #[test]
    fn webhookauth_new_api_key_returns_empty_required_field_when_value_is_empty() {
        assert_eq!(
            WebhookAuth::new_api_key("api-key", "", "X-API-Key"),
            Err(TriggerDataError::EmptyRequiredField(
                "apikey_value".to_string()
            ))
        );
    }

    #[test]
    fn webhookauth_new_api_key_returns_empty_required_field_when_header_name_is_empty() {
        assert_eq!(
            WebhookAuth::new_api_key("api-key", "abc123", ""),
            Err(TriggerDataError::EmptyRequiredField(
                "apikey_header_name".to_string()
            ))
        );
    }

    // -- TriggerId validation tests --------------------------------------------

    #[test]
    fn crons_trigger_new_returns_invalid_trigger_id_when_id_is_too_short() {
        let result = CronTrigger::new("ab", None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    #[test]
    fn crons_trigger_new_returns_invalid_trigger_id_when_id_is_too_long() {
        let long_id = "a".repeat(65);
        let result = CronTrigger::new(long_id, None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    #[test]
    fn crons_trigger_new_returns_invalid_cron_expression_when_cron_is_invalid() {
        let result = CronTrigger::new("trigger-001", None, None, "not-a-cron", "UTC", false, None);
        assert!(matches!(
            result,
            Err(TriggerDataError::InvalidCronExpression(_))
        ));
    }

    // -- WebhookTrigger tests --------------------------------------------------

    #[test]
    fn webhook_trigger_new_returns_ok_when_all_fields_valid() {
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn webhook_trigger_new_returns_invalid_url_when_url_lacks_http_scheme() {
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "ftp://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidUrl(_))));
    }

    #[test]
    fn webhook_trigger_new_returns_header_limit_exceeded_when_header_count_exceeds_64() {
        let mut headers = HashMap::new();
        for i in 0..65 {
            headers.insert(format!("Header-{}", i), "value".to_string());
        }
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            Some(headers),
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::HeaderLimitExceeded(_))
        ));
    }

    #[test]
    fn webhook_trigger_new_returns_header_limit_exceeded_when_header_name_exceeds_512_bytes() {
        let mut headers = HashMap::new();
        headers.insert("a".repeat(513), "value".to_string());
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            Some(headers),
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::HeaderLimitExceeded(_))
        ));
    }

    #[test]
    fn webhook_trigger_new_returns_header_limit_exceeded_when_header_value_exceeds_8192_bytes() {
        let mut headers = HashMap::new();
        headers.insert("Header".to_string(), "a".repeat(8193));
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            Some(headers),
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::HeaderLimitExceeded(_))
        ));
    }

    // -- PollingTrigger tests --------------------------------------------------

    #[test]
    fn polling_trigger_new_returns_ok_when_all_fields_valid() {
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn polling_trigger_new_returns_invalid_interval_when_interval_is_zero() {
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "0s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidInterval(_))));
    }

    // -- Trigger root enum tests -----------------------------------------------

    #[test]
    fn trigger_cron_roundtrip_serialization() {
        let cron = CronTrigger::new(
            "trigger-001",
            Some("Daily Job".to_string()),
            None,
            "0 0 * * * *",
            "UTC",
            false,
            None,
        )
        .unwrap();
        let trigger = Trigger::Cron(cron);

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"cron\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        assert!(matches!(recovered, Trigger::Cron(_)));
    }

    #[test]
    fn trigger_webhook_roundtrip_serialization() {
        let webhook = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let trigger = Trigger::Webhook(webhook);

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"webhook\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        assert!(matches!(recovered, Trigger::Webhook(_)));
    }

    #[test]
    fn trigger_polling_roundtrip_serialization() {
        let polling = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let trigger = Trigger::Polling(polling);

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"polling\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        assert!(matches!(recovered, Trigger::Polling(_)));
    }
}
