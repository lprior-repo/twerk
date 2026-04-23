//! Webhook-based trigger configuration.

use std::collections::HashMap;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize};

use crate::id::TriggerId;

use super::error::TriggerDataError;
use super::http_method::HttpMethod;
use super::validation::validate_webhook;
use super::webhook_auth::WebhookAuth;

// =============================================================================
// WebhookTrigger
// =============================================================================

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

/// Intermediate struct for `WebhookTrigger` deserialization with validation.
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
    /// Validates the raw fields of a `WebhookTrigger`.
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

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
}
