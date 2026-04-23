//! Shared validation functions for trigger data construction.

use std::collections::HashMap;

use crate::domain::{GoDuration, GoDurationError};

use super::error::TriggerDataError;
use super::webhook_auth::WebhookAuth;

// =============================================================================
// Cron validation helpers
// =============================================================================

/// Normalizes timezone shorthand "Z" to "UTC".
pub(super) fn normalize_timezone(tz: &str) -> String {
    if tz == "Z" {
        "UTC".to_string()
    } else {
        tz.to_string()
    }
}

/// Validates a timezone string using chrono-tz.
pub(super) fn validate_timezone(tz: &str) -> Result<(), TriggerDataError> {
    use chrono_tz::Tz;
    tz.parse::<Tz>()
        .map(|_| ())
        .map_err(|_| TriggerDataError::InvalidTimezone(tz.to_string()))
}

// =============================================================================
// Webhook / shared validation helpers
// =============================================================================

/// Validates that a URL has a valid HTTP/HTTPS scheme.
pub(super) fn validate_url(url: &str) -> Result<(), TriggerDataError> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(TriggerDataError::InvalidUrl(url.to_string()));
    }
    Ok(())
}

/// Validates header limits:
/// - Maximum 64 header entries
/// - Maximum 512 bytes per header name
/// - Maximum 8192 bytes per header value
pub(super) fn validate_headers(headers: &HashMap<String, String>) -> Result<(), TriggerDataError> {
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
pub(super) fn validate_webhook_auth(auth: &WebhookAuth) -> Result<(), TriggerDataError> {
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
pub(super) fn validate_webhook(
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

// =============================================================================
// Polling validation helpers
// =============================================================================

/// Validates a JMESPath expression by attempting to compile it.
pub(super) fn validate_jmespath(expr: &str) -> Result<(), TriggerDataError> {
    jmespath::compile(expr)
        .map(|_| ())
        .map_err(|e| TriggerDataError::InvalidJmespath(e.to_string()))
}

/// Validates a polling interval is non-zero.
pub(super) fn validate_interval_nonzero(interval: &GoDuration) -> Result<(), TriggerDataError> {
    if interval.to_duration() == std::time::Duration::ZERO {
        return Err(TriggerDataError::InvalidInterval(
            GoDurationError::ZeroDuration,
        ));
    }
    Ok(())
}

/// Validates a polling timeout is non-zero if present.
pub(super) fn validate_timeout_nonzero(timeout: &Option<GoDuration>) -> Result<(), TriggerDataError> {
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
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_polling(
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
