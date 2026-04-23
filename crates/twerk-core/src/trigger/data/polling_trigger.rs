//! Polling-based trigger configuration.

use std::collections::HashMap;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize};

use crate::domain::GoDuration;
use crate::id::TriggerId;

use super::error::TriggerDataError;
use super::http_method::HttpMethod;
use super::validation::validate_polling;
use super::webhook_auth::WebhookAuth;

// =============================================================================
// PollingTrigger
// =============================================================================

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

/// Intermediate struct for `PollingTrigger` deserialization with validation.
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
    /// Validates the raw fields of a `PollingTrigger`.
    #[allow(clippy::too_many_arguments)]
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
    /// - `interval: impl Into<String>` - Polling interval in `GoDuration` format (e.g., "30s", "1m", "1h")
    /// - `timeout: Option<GoDuration>` - Optional request timeout
    /// - `stop_condition: Option<String>` - Optional `JMESPath` expression to stop polling
    /// - `payload: Option<serde_json::Value>` - Optional JSON payload
    /// - `disabled: bool` - Whether trigger is disabled (default: false)
    ///
    /// # Returns
    /// - `Ok(PollingTrigger { ... })` when all validations pass
    /// - `Err(TriggerDataError::InvalidTriggerId)` when `id` is invalid
    /// - `Err(TriggerDataError::InvalidUrl)` when `url` is not valid HTTP(S) URL
    /// - `Err(TriggerDataError::InvalidInterval)` when `interval` is not valid `GoDuration` or is zero
    /// - `Err(TriggerDataError::InvalidInterval)` when `timeout` is present but not valid `GoDuration`
    /// - `Err(TriggerDataError::InvalidJmespath)` when `stop_condition` is present but invalid `JMESPath`
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
