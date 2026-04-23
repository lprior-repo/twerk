//! Authentication configuration for webhook triggers.

use serde::{Deserialize, Serialize};

use super::error::TriggerDataError;

// =============================================================================
// WebhookAuth
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

    /// Creates a `WebhookAuth::ApiKey` variant with key, value, and `header_name`.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
