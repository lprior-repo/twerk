//! Comprehensive tests for trigger DATA types.
//!
//! This module tests all 71 behaviors from the test-plan:
//! - HttpMethod parsing (8 behaviors)
//! - WebhookAuth constructors (12 behaviors)
//! - CronTrigger validation (9 behaviors)
//! - WebhookTrigger validation (16 behaviors)
//! - PollingTrigger validation (20 behaviors)
//! - Trigger root enum serialization (5 behaviors)

#![allow(unused_imports)]

use std::collections::HashMap;

use twerk_core::domain::{CronExpression, CronExpressionError, GoDuration, GoDurationError};
use twerk_core::id::{IdError, TriggerId};

use twerk_core::trigger::data::{
    CronTrigger, HttpMethod, PollingTrigger, Trigger, TriggerDataError, WebhookAuth, WebhookTrigger,
};

// =============================================================================
// HttpMethod Tests (Behaviors 1-8)
// =============================================================================

mod httpmethod_tests {
    use super::*;

    // Behavior 1: HttpMethod::from_str returns Ok(HttpMethod::Get) when input is "GET"
    #[test]
    fn httpmethod_from_str_returns_get_when_input_is_get_uppercase() {
        let result = HttpMethod::from_str("GET");
        assert_eq!(result, Ok(HttpMethod::Get));
    }

    // Behavior 2: HttpMethod::from_str returns Ok(HttpMethod::Get) when input is "get" (case-insensitive)
    #[test]
    fn httpmethod_from_str_returns_get_when_input_is_get_lowercase() {
        let result = HttpMethod::from_str("get");
        assert_eq!(result, Ok(HttpMethod::Get));
    }

    // Behavior 2b: Case-insensitive for mixed case
    #[test]
    fn httpmethod_from_str_returns_get_when_input_is_get_mixed_case() {
        assert_eq!(HttpMethod::from_str("Get"), Ok(HttpMethod::Get));
        assert_eq!(HttpMethod::from_str("gEt"), Ok(HttpMethod::Get));
        assert_eq!(HttpMethod::from_str("GEt"), Ok(HttpMethod::Get));
    }

    // Behavior 3: HttpMethod::from_str returns Ok(HttpMethod::Post) when input is "POST"
    #[test]
    fn httpmethod_from_str_returns_post_when_input_is_post_uppercase() {
        let result = HttpMethod::from_str("POST");
        assert_eq!(result, Ok(HttpMethod::Post));
    }

    #[test]
    fn httpmethod_from_str_returns_post_when_input_is_post_lowercase() {
        assert_eq!(HttpMethod::from_str("post"), Ok(HttpMethod::Post));
    }

    // Behavior 4: HttpMethod::from_str returns Ok(HttpMethod::Put) when input is "PUT"
    #[test]
    fn httpmethod_from_str_returns_put_when_input_is_put_uppercase() {
        let result = HttpMethod::from_str("PUT");
        assert_eq!(result, Ok(HttpMethod::Put));
    }

    #[test]
    fn httpmethod_from_str_returns_put_when_input_is_put_lowercase() {
        assert_eq!(HttpMethod::from_str("put"), Ok(HttpMethod::Put));
    }

    // Behavior 5: HttpMethod::from_str returns Ok(HttpMethod::Delete) when input is "DELETE"
    #[test]
    fn httpmethod_from_str_returns_delete_when_input_is_delete_uppercase() {
        let result = HttpMethod::from_str("DELETE");
        assert_eq!(result, Ok(HttpMethod::Delete));
    }

    #[test]
    fn httpmethod_from_str_returns_delete_when_input_is_delete_lowercase() {
        assert_eq!(HttpMethod::from_str("delete"), Ok(HttpMethod::Delete));
    }

    // Behavior 6: HttpMethod::from_str returns Ok(HttpMethod::Patch) when input is "PATCH"
    #[test]
    fn httpmethod_from_str_returns_patch_when_input_is_patch_uppercase() {
        let result = HttpMethod::from_str("PATCH");
        assert_eq!(result, Ok(HttpMethod::Patch));
    }

    #[test]
    fn httpmethod_from_str_returns_patch_when_input_is_patch_lowercase() {
        assert_eq!(HttpMethod::from_str("patch"), Ok(HttpMethod::Patch));
    }

    // Behavior 7: HttpMethod::from_str returns Err(TriggerDataError::InvalidHttpMethod) when input is invalid
    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_options() {
        let result = HttpMethod::from_str("OPTIONS");
        assert_eq!(
            result,
            Err(TriggerDataError::InvalidHttpMethod("OPTIONS".to_string()))
        );
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_trace() {
        let result = HttpMethod::from_str("TRACE");
        assert_eq!(
            result,
            Err(TriggerDataError::InvalidHttpMethod("TRACE".to_string()))
        );
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_connect() {
        let result = HttpMethod::from_str("CONNECT");
        assert_eq!(
            result,
            Err(TriggerDataError::InvalidHttpMethod("CONNECT".to_string()))
        );
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_head() {
        let result = HttpMethod::from_str("HEAD");
        assert_eq!(
            result,
            Err(TriggerDataError::InvalidHttpMethod("HEAD".to_string()))
        );
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_purge() {
        let result = HttpMethod::from_str("PURGE");
        assert_eq!(
            result,
            Err(TriggerDataError::InvalidHttpMethod("PURGE".to_string()))
        );
    }

    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_random_string() {
        let result = HttpMethod::from_str("RANDOM");
        assert_eq!(
            result,
            Err(TriggerDataError::InvalidHttpMethod("RANDOM".to_string()))
        );
    }

    // Behavior 8: HttpMethod::from_str returns Err when input is empty
    #[test]
    fn httpmethod_from_str_returns_invalid_http_method_when_input_is_empty() {
        let result = HttpMethod::from_str("");
        assert_eq!(
            result,
            Err(TriggerDataError::InvalidHttpMethod("".to_string()))
        );
    }

    // Additional: serialize as uppercase
    #[test]
    fn httpmethod_serialize_returns_uppercase() {
        assert_eq!(serde_json::to_string(&HttpMethod::Get).unwrap(), "\"GET\"");
        assert_eq!(
            serde_json::to_string(&HttpMethod::Post).unwrap(),
            "\"POST\""
        );
        assert_eq!(serde_json::to_string(&HttpMethod::Put).unwrap(), "\"PUT\"");
        assert_eq!(
            serde_json::to_string(&HttpMethod::Delete).unwrap(),
            "\"DELETE\""
        );
        assert_eq!(
            serde_json::to_string(&HttpMethod::Patch).unwrap(),
            "\"PATCH\""
        );
    }

    // Additional: Display impl returns uppercase
    #[test]
    fn httpmethod_display_returns_uppercase() {
        assert_eq!(format!("{}", HttpMethod::Get), "GET");
        assert_eq!(format!("{}", HttpMethod::Post), "POST");
        assert_eq!(format!("{}", HttpMethod::Put), "PUT");
        assert_eq!(format!("{}", HttpMethod::Delete), "DELETE");
        assert_eq!(format!("{}", HttpMethod::Patch), "PATCH");
    }

    // Additional: FromStr impl works
    #[test]
    fn httpmethod_from_str_trait_implementation() {
        use std::str::FromStr;
        assert_eq!("GET".parse::<HttpMethod>(), Ok(HttpMethod::Get));
        assert_eq!("post".parse::<HttpMethod>(), Ok(HttpMethod::Post));
    }
}

// =============================================================================
// WebhookAuth Tests (Behaviors 9-20)
// =============================================================================

mod webhookauth_tests {
    use super::*;

    // Behavior 9: WebhookAuth::new_none returns Ok(WebhookAuth::None)
    #[test]
    fn webhookauth_new_none_returns_none_variant() {
        let result = WebhookAuth::new_none();
        assert_eq!(result, WebhookAuth::None);
    }

    // Behavior 10: WebhookAuth::new_basic returns Ok when username and password are non-empty
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
    fn webhookauth_new_basic_preserves_username_and_password_values() {
        let result = WebhookAuth::new_basic("user123", "pass456").unwrap();
        match result {
            WebhookAuth::Basic { username, password } => {
                assert_eq!(username, "user123");
                assert_eq!(password, "pass456");
            }
            _ => panic!("Expected Basic variant"),
        }
    }

    // Behavior 11: WebhookAuth::new_basic returns Err when username is empty
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

    // Behavior 12: WebhookAuth::new_basic returns Err when password is empty
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

    // Both empty - username takes precedence
    #[test]
    fn webhookauth_new_basic_returns_username_error_when_both_empty() {
        let result = WebhookAuth::new_basic("", "");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "basic_auth_username".to_string()
            ))
        );
    }

    // Behavior 13: WebhookAuth::new_bearer returns Ok when token is non-empty
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
    fn webhookauth_new_bearer_preserves_token_value() {
        let result = WebhookAuth::new_bearer("my-super-secret-token").unwrap();
        match result {
            WebhookAuth::Bearer { token } => {
                assert_eq!(token, "my-super-secret-token");
            }
            _ => panic!("Expected Bearer variant"),
        }
    }

    // Behavior 14: WebhookAuth::new_bearer returns Err when token is empty
    #[test]
    fn webhookauth_new_bearer_returns_empty_required_field_when_token_is_empty() {
        let result = WebhookAuth::new_bearer("");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "bearer_token".to_string()
            ))
        );
    }

    // Behavior 15: WebhookAuth::new_api_key returns Ok when all fields are non-empty
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
    fn webhookauth_new_api_key_preserves_all_field_values() {
        let result = WebhookAuth::new_api_key("key-name", "key-value", "X-Custom-Header").unwrap();
        match result {
            WebhookAuth::ApiKey {
                key,
                value,
                header_name,
            } => {
                assert_eq!(key, "key-name");
                assert_eq!(value, "key-value");
                assert_eq!(header_name, "X-Custom-Header");
            }
            _ => panic!("Expected ApiKey variant"),
        }
    }

    // Behavior 16: WebhookAuth::new_api_key returns Err when key is empty
    #[test]
    fn webhookauth_new_api_key_returns_empty_required_field_when_key_is_empty() {
        let result = WebhookAuth::new_api_key("", "abc123", "X-API-Key");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "apikey_key".to_string()
            ))
        );
    }

    // Behavior 17: WebhookAuth::new_api_key returns Err when value is empty
    #[test]
    fn webhookauth_new_api_key_returns_empty_required_field_when_value_is_empty() {
        let result = WebhookAuth::new_api_key("api-key", "", "X-API-Key");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "apikey_value".to_string()
            ))
        );
    }

    // Behavior 18: WebhookAuth::new_api_key returns Err when header_name is empty
    #[test]
    fn webhookauth_new_api_key_returns_empty_required_field_when_header_name_is_empty() {
        let result = WebhookAuth::new_api_key("api-key", "abc123", "");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "apikey_header_name".to_string()
            ))
        );
    }

    // All three empty - key takes precedence
    #[test]
    fn webhookauth_new_api_key_returns_key_error_when_all_empty() {
        let result = WebhookAuth::new_api_key("", "", "");
        assert_eq!(
            result,
            Err(TriggerDataError::EmptyRequiredField(
                "apikey_key".to_string()
            ))
        );
    }

    // Behavior 19: WebhookAuth::none returns WebhookAuth::None equivalent to new_none
    #[test]
    fn webhookauth_none_method_equivalents_new_none() {
        assert_eq!(WebhookAuth::none(), WebhookAuth::new_none());
    }

    #[test]
    fn webhookauth_none_returns_none_variant() {
        assert_eq!(WebhookAuth::none(), WebhookAuth::None);
    }

    // Behavior 20: WebhookAuth serialization produces tag "type" with variant name
    #[test]
    fn webhookauth_serialization_produces_type_tag_none() {
        let json = serde_json::to_string(&WebhookAuth::None).unwrap();
        // Note: rename_all = "camelCase" converts "None" to "none"
        assert!(json.contains("\"type\":\"none\""));
    }

    #[test]
    fn webhookauth_serialization_produces_type_tag_basic() {
        let auth = WebhookAuth::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        // Note: rename_all = "camelCase" converts "Basic" to "basic"
        assert!(json.contains("\"type\":\"basic\""));
        assert!(json.contains("\"username\""));
        assert!(json.contains("\"password\""));
    }

    #[test]
    fn webhookauth_serialization_produces_type_tag_bearer() {
        let auth = WebhookAuth::Bearer {
            token: "token123".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        // Note: rename_all = "camelCase" converts "Bearer" to "bearer"
        assert!(json.contains("\"type\":\"bearer\""));
        assert!(json.contains("\"token\""));
    }

    #[test]
    fn webhookauth_serialization_produces_type_tag_api_key() {
        let auth = WebhookAuth::ApiKey {
            key: "key".to_string(),
            value: "value".to_string(),
            header_name: "X-Key".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        // Note: rename_all = "camelCase" converts "ApiKey" to "apiKey"
        // Fields inside variants may not be renamed - they keep their original names
        assert!(json.contains("\"type\":\"apiKey\""));
        assert!(json.contains("\"key\""));
        assert!(json.contains("\"value\""));
        assert!(json.contains("\"header_name\"")); // field name unchanged
    }

    // Deserialization roundtrip
    #[test]
    fn webhookauth_basic_roundtrip_serialization() {
        let original = WebhookAuth::Basic {
            username: "admin".to_string(),
            password: "secret".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let recovered: WebhookAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(original, recovered);
    }

    #[test]
    fn webhookauth_bearer_roundtrip_serialization() {
        let original = WebhookAuth::Bearer {
            token: "jwt.token.here".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let recovered: WebhookAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(original, recovered);
    }

    #[test]
    fn webhookauth_api_key_roundtrip_serialization() {
        let original = WebhookAuth::ApiKey {
            key: "my-key".to_string(),
            value: "my-value".to_string(),
            header_name: "X-MyKey".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let recovered: WebhookAuth = serde_json::from_str(&json).unwrap();
        assert_eq!(original, recovered);
    }

    // Default trait
    #[test]
    fn webhookauth_default_returns_none() {
        let default: WebhookAuth = WebhookAuth::default();
        assert_eq!(default, WebhookAuth::None);
    }
}

// =============================================================================
// CronTrigger Tests (Behaviors 21-29)
// =============================================================================

mod cronscheduler_tests {
    use super::*;

    // Behavior 21: CronTrigger::new returns Ok when all fields are valid
    #[test]
    fn cronscheduler_new_returns_ok_when_all_fields_valid() {
        let result = CronTrigger::new(
            "trigger-001",
            Some("Daily Job".to_string()),
            Some("Runs daily at midnight".to_string()),
            "0 0 * * * *",
            "UTC",
            false,
            None,
        );
        let trigger = result.unwrap();
        assert_eq!(trigger.id.to_string(), "trigger-001");
        assert_eq!(trigger.name, Some("Daily Job".to_string()));
        assert_eq!(
            trigger.description,
            Some("Runs daily at midnight".to_string())
        );
        assert!(!trigger.disabled);
        assert!(trigger.payload.is_none());
    }

    #[test]
    fn cronscheduler_new_returns_ok_with_minimal_fields() {
        let trigger =
            CronTrigger::new("abc", None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        assert_eq!(trigger.id.to_string(), "abc");
        assert!(trigger.name.is_none());
        assert!(trigger.description.is_none());
        assert!(!trigger.disabled);
    }

    // Behavior 22: CronTrigger::new returns Err when id is too short
    #[test]
    fn cronscheduler_new_returns_invalid_trigger_id_when_id_is_too_short() {
        let result = CronTrigger::new("ab", None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    // Behavior 23: CronTrigger::new returns Err when id is too long
    #[test]
    fn cronscheduler_new_returns_invalid_trigger_id_when_id_is_too_long() {
        let long_id = "a".repeat(65);
        let result = CronTrigger::new(long_id, None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    // Behavior 24: CronTrigger::new returns Err when id contains invalid characters
    #[test]
    fn cronscheduler_new_returns_invalid_trigger_id_when_id_has_invalid_chars() {
        let result = CronTrigger::new(
            "trigger@001!",
            None,
            None,
            "0 0 * * * *",
            "UTC",
            false,
            None,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    // Behavior 25: CronTrigger::new returns Err when cron expression is invalid
    #[test]
    fn cronscheduler_new_returns_invalid_cron_expression_when_cron_is_invalid() {
        let result = CronTrigger::new("trigger-001", None, None, "not-a-cron", "UTC", false, None);
        assert!(matches!(
            result,
            Err(TriggerDataError::InvalidCronExpression(_))
        ));
    }

    #[test]
    fn cronscheduler_new_returns_invalid_cron_expression_when_cron_is_empty() {
        let result = CronTrigger::new("trigger-001", None, None, "", "UTC", false, None);
        assert!(matches!(
            result,
            Err(TriggerDataError::InvalidCronExpression(_))
        ));
    }

    // Behavior 26: CronTrigger::new returns Err when timezone is not valid IANA timezone
    #[test]
    fn cronscheduler_new_returns_invalid_timezone_when_timezone_is_invalid() {
        let result = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "Not/A/Timezone",
            false,
            None,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidTimezone(_))));
    }

    // Behavior 27: CronTrigger::new returns Err when timezone string is garbage
    #[test]
    fn cronscheduler_new_returns_invalid_timezone_when_timezone_is_garbage() {
        let result = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "garbage",
            false,
            None,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidTimezone(_))));
    }

    // Behavior 28: CronTrigger serializes with "type": "cron" discriminant (lowercase due to rename_all = "camelCase")
    #[test]
    fn cronscheduler_serializes_with_type_cron_discriminant() {
        let cron =
            CronTrigger::new("trigger-001", None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        let trigger = Trigger::Cron(cron);
        let json = serde_json::to_string(&trigger).unwrap();
        // Note: rename_all = "camelCase" on the enum converts "Cron" to "cron"
        assert!(json.contains("\"type\":\"cron\""));
    }

    #[test]
    fn cronscheduler_serializes_camelcase_fields() {
        let cron = CronTrigger::new(
            "trigger-001",
            Some("Test".to_string()),
            None,
            "0 0 * * * *",
            "UTC",
            false,
            None,
        )
        .unwrap();
        let trigger = Trigger::Cron(cron);
        let json = serde_json::to_string(&trigger).unwrap();
        // With rename_all = "camelCase", fields should be id, name, cron, timezone, disabled
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"cron\""));
        assert!(json.contains("\"timezone\""));
    }

    // Behavior 29: CronTrigger deserialization produces correct variant from valid JSON
    #[test]
    fn cronscheduler_deserialization_produces_correct_variant() {
        // Note: field names use camelCase per struct rename_all attribute
        // Note: variant names are also converted by rename_all = "camelCase", so "Cron" -> "cron"
        let json = r#"{"type":"cron","id":"trigger-001","cron":"0 0 * * * *","timezone":"UTC"}"#;
        match serde_json::from_str::<Trigger>(json).unwrap() {
            Trigger::Cron(cron) => {
                assert_eq!(cron.id.to_string(), "trigger-001");
            }
            _ => panic!("Expected Cron variant"),
        }
    }

    #[test]
    fn cronscheduler_roundtrip_serialization() {
        let original = CronTrigger::new(
            "trigger-001",
            Some("Daily Job".to_string()),
            Some("Description".to_string()),
            "0 0 * * * *",
            "America/New_York",
            false,
            Some(serde_json::json!({"key": "value"})),
        )
        .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let recovered: CronTrigger = serde_json::from_str(&json).unwrap();

        assert_eq!(original.id, recovered.id);
        assert_eq!(original.name, recovered.name);
        assert_eq!(original.description, recovered.description);
        assert_eq!(original.disabled, recovered.disabled);
    }

    // Additional: payload is preserved
    #[test]
    fn cronscheduler_new_preserves_payload() {
        let payload = serde_json::json!({"data": "test"});
        let trigger = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "UTC",
            false,
            Some(payload.clone()),
        )
        .unwrap();
        assert_eq!(trigger.payload, Some(payload));
    }

    // Additional: disabled field defaults
    #[test]
    fn cronscheduler_disabled_defaults_to_false() {
        let trigger =
            CronTrigger::new("trigger-001", None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        assert!(!trigger.disabled);
    }

    // Valid timezone tests
    #[test]
    fn cronscheduler_new_accepts_valid_iana_timezone_america_new_york() {
        let trigger = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "America/New_York",
            false,
            None,
        )
        .unwrap();
        assert_eq!(trigger.timezone, "America/New_York");
    }

    #[test]
    fn cronscheduler_new_accepts_valid_iana_timezone_europe_london() {
        let trigger = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "Europe/London",
            false,
            None,
        )
        .unwrap();
        assert_eq!(trigger.timezone, "Europe/London");
    }

    #[test]
    fn cronscheduler_new_accepts_valid_iana_timezone_asia_tokyo() {
        let trigger = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "Asia/Tokyo",
            false,
            None,
        )
        .unwrap();
        assert_eq!(trigger.timezone, "Asia/Tokyo");
    }

    // Boundary: id exactly 3 chars (minimum valid)
    #[test]
    fn cronscheduler_new_accepts_id_with_exactly_3_chars() {
        let trigger =
            CronTrigger::new("abc", None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        assert_eq!(trigger.id.to_string(), "abc");
    }

    // Boundary: id exactly 64 chars (maximum valid)
    #[test]
    fn cronscheduler_new_accepts_id_with_exactly_64_chars() {
        let id = "a".repeat(64);
        let trigger =
            CronTrigger::new(id.clone(), None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        assert_eq!(trigger.id.to_string(), id);
    }

    // Boundary: id with 2 chars (invalid)
    #[test]
    fn cronscheduler_new_rejects_id_with_2_chars() {
        let result = CronTrigger::new("ab", None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    // Boundary: id with 65 chars (invalid)
    #[test]
    fn cronscheduler_new_rejects_id_with_65_chars() {
        let id = "a".repeat(65);
        let result = CronTrigger::new(id, None, None, "0 0 * * * *", "UTC", false, None);
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }
}

// =============================================================================
// WebhookTrigger Tests (Behaviors 30-45)
// =============================================================================

mod webhooktrigger_tests {
    use super::*;

    // Behavior 30: WebhookTrigger::new returns Ok when all fields are valid
    #[test]
    fn webhooktrigger_new_returns_ok_when_all_fields_valid() {
        let trigger = WebhookTrigger::new(
            "webhook-001",
            Some("My Webhook".to_string()),
            Some("A test webhook".to_string()),
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.id.to_string(), "webhook-001");
        assert_eq!(trigger.name, Some("My Webhook".to_string()));
        assert_eq!(trigger.description, Some("A test webhook".to_string()));
        assert_eq!(trigger.url, "https://example.com/hook");
        assert_eq!(trigger.method, HttpMethod::Post);
        assert_eq!(trigger.auth, WebhookAuth::None);
        assert!(!trigger.disabled);
    }

    #[test]
    fn webhooktrigger_new_returns_ok_with_minimal_fields() {
        let trigger = WebhookTrigger::new(
            "abc",
            None,
            None,
            "http://example.com",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.id.to_string(), "abc");
        assert!(trigger.name.is_none());
        assert_eq!(trigger.url, "http://example.com");
        assert_eq!(trigger.method, HttpMethod::Get);
    }

    // Behavior 31: WebhookTrigger::new returns Err when id is invalid
    #[test]
    fn webhooktrigger_new_returns_invalid_trigger_id_when_id_is_invalid() {
        let result = WebhookTrigger::new(
            "ab",
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
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    // Behavior 32: WebhookTrigger::new returns Err when url lacks http scheme
    #[test]
    fn webhooktrigger_new_returns_invalid_url_when_url_lacks_http_scheme() {
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

    // Behavior 33: WebhookTrigger::new returns Err when url lacks https scheme
    #[test]
    fn webhooktrigger_new_returns_invalid_url_when_url_lacks_https_scheme() {
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "file:///etc/passwd",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidUrl(_))));
    }

    // Behavior 34: WebhookTrigger::new returns Err when url is malformed
    #[test]
    fn webhooktrigger_new_returns_invalid_url_when_url_is_malformed() {
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "not-a-url",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidUrl(_))));
    }

    // Behavior 35-36: WebhookAuth::Basic with empty username/password
    #[test]
    fn webhooktrigger_new_with_webhookauth_basic_empty_username_returns_error() {
        let auth = WebhookAuth::Basic {
            username: "".to_string(),
            password: "pass".to_string(),
        };
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn webhooktrigger_new_with_webhookauth_basic_empty_password_returns_error() {
        let auth = WebhookAuth::Basic {
            username: "user".to_string(),
            password: "".to_string(),
        };
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    // Behavior 37: WebhookAuth::Bearer with empty token
    #[test]
    fn webhooktrigger_new_with_webhookauth_bearer_empty_token_returns_error() {
        let auth = WebhookAuth::Bearer {
            token: "".to_string(),
        };
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    // Behavior 38-40: WebhookAuth::ApiKey with empty fields
    #[test]
    fn webhooktrigger_new_with_webhookauth_api_key_empty_key_returns_error() {
        let auth = WebhookAuth::ApiKey {
            key: "".to_string(),
            value: "abc123".to_string(),
            header_name: "X-API-Key".to_string(),
        };
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn webhooktrigger_new_with_webhookauth_api_key_empty_value_returns_error() {
        let auth = WebhookAuth::ApiKey {
            key: "api-key".to_string(),
            value: "".to_string(),
            header_name: "X-API-Key".to_string(),
        };
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn webhooktrigger_new_with_webhookauth_api_key_empty_header_name_returns_error() {
        let auth = WebhookAuth::ApiKey {
            key: "api-key".to_string(),
            value: "abc123".to_string(),
            header_name: "".to_string(),
        };
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    // Behavior 41: Header count exceeds 64
    #[test]
    fn webhooktrigger_new_returns_header_limit_exceeded_when_header_count_exceeds_64() {
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

    // Behavior 42: Header name exceeds 512 bytes
    #[test]
    fn webhooktrigger_new_returns_header_limit_exceeded_when_header_name_exceeds_512_bytes() {
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

    // Behavior 43: Header value exceeds 8192 bytes
    #[test]
    fn webhooktrigger_new_returns_header_limit_exceeded_when_header_value_exceeds_8192_bytes() {
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

    // Boundary: exactly 64 headers is valid
    #[test]
    fn webhooktrigger_new_returns_ok_when_header_count_is_exactly_64() {
        let mut headers = HashMap::new();
        for i in 0..64 {
            headers.insert(format!("Header-{}", i), "value".to_string());
        }
        let trigger = WebhookTrigger::new(
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
        )
        .unwrap();
        assert_eq!(trigger.id.to_string(), "webhook-001");
        assert_eq!(trigger.url, "https://example.com/hook");
        assert_eq!(trigger.headers.as_ref().unwrap().len(), 64);
    }

    // Boundary: exactly 512 bytes header name is valid
    #[test]
    fn webhooktrigger_new_returns_ok_when_header_name_is_exactly_512_bytes() {
        let mut headers = HashMap::new();
        headers.insert("a".repeat(512), "value".to_string());
        let trigger = WebhookTrigger::new(
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
        )
        .unwrap();
        assert_eq!(trigger.headers.as_ref().unwrap().len(), 1);
        assert_eq!(
            trigger
                .headers
                .as_ref()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .len(),
            512
        );
    }

    // Boundary: exactly 8192 bytes header value is valid
    #[test]
    fn webhooktrigger_new_returns_ok_when_header_value_is_exactly_8192_bytes() {
        let mut headers = HashMap::new();
        headers.insert("Header".to_string(), "a".repeat(8192));
        let trigger = WebhookTrigger::new(
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
        )
        .unwrap();
        assert_eq!(
            trigger
                .headers
                .as_ref()
                .unwrap()
                .values()
                .next()
                .unwrap()
                .len(),
            8192
        );
    }

    // Behavior 44: WebhookTrigger serializes with correct discriminant
    #[test]
    fn webhooktrigger_serializes_with_type_webhook_discriminant() {
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
    }

    // Behavior 45: WebhookTrigger deserialization produces correct variant
    #[test]
    fn webhooktrigger_deserialization_produces_correct_variant() {
        // Note: variant names are converted by rename_all = "camelCase", so "Webhook" -> "webhook"
        let json = r#"{"type":"webhook","id":"webhook-001","url":"https://example.com/hook","method":"POST"}"#;
        match serde_json::from_str::<Trigger>(json).unwrap() {
            Trigger::Webhook(webhook) => {
                assert_eq!(webhook.id.to_string(), "webhook-001");
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    #[test]
    fn webhooktrigger_roundtrip_serialization() {
        let original = WebhookTrigger::new(
            "webhook-001",
            Some("My Webhook".to_string()),
            Some("Description".to_string()),
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::Bearer {
                token: "secret".to_string(),
            },
            Some(HashMap::from([(
                "Content-Type".to_string(),
                "application/json".to_string(),
            )])),
            Some("body template".to_string()),
            Some(serde_json::json!({"key": "value"})),
            true,
        )
        .unwrap();
        let trigger = Trigger::Webhook(original);

        let json = serde_json::to_string(&trigger).unwrap();
        let recovered: Trigger = serde_json::from_str(&json).unwrap();

        match recovered {
            Trigger::Webhook(webhook) => {
                assert_eq!(webhook.id.to_string(), "webhook-001");
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    // Valid URL schemes
    #[test]
    fn webhooktrigger_new_accepts_http_url() {
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "http://example.com/hook",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.url, "http://example.com/hook");
        assert_eq!(trigger.method, HttpMethod::Get);
    }

    #[test]
    fn webhooktrigger_new_accepts_https_url() {
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.url, "https://example.com/hook");
    }

    // Invalid URL schemes
    #[test]
    fn webhooktrigger_new_rejects_ftp_url() {
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "ftp://example.com/hook",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidUrl(_))));
    }

    #[test]
    fn webhooktrigger_new_rejects_ws_url() {
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "ws://example.com/hook",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidUrl(_))));
    }

    #[test]
    fn webhooktrigger_new_rejects_mailto_url() {
        let result = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "mailto://example.com",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidUrl(_))));
    }

    // With valid auth variants
    #[test]
    fn webhooktrigger_new_accepts_basic_auth() {
        let auth = WebhookAuth::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        match trigger.auth {
            WebhookAuth::Basic { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected Basic auth"),
        }
    }

    #[test]
    fn webhooktrigger_new_accepts_bearer_auth() {
        let auth = WebhookAuth::Bearer {
            token: "token123".to_string(),
        };
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        match trigger.auth {
            WebhookAuth::Bearer { token } => {
                assert_eq!(token, "token123");
            }
            _ => panic!("Expected Bearer auth"),
        }
    }

    #[test]
    fn webhooktrigger_new_accepts_api_key_auth() {
        let auth = WebhookAuth::ApiKey {
            key: "api-key".to_string(),
            value: "abc123".to_string(),
            header_name: "X-API-Key".to_string(),
        };
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            auth,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        match trigger.auth {
            WebhookAuth::ApiKey {
                key,
                value,
                header_name,
            } => {
                assert_eq!(key, "api-key");
                assert_eq!(value, "abc123");
                assert_eq!(header_name, "X-API-Key");
            }
            _ => panic!("Expected ApiKey auth"),
        }
    }
}

// =============================================================================
// PollingTrigger Tests (Behaviors 46-66)
// =============================================================================

mod pollingtrigger_tests {
    use super::*;

    // Behavior 46: PollingTrigger::new returns Ok when all fields are valid
    #[test]
    fn pollingtrigger_new_returns_ok_when_all_fields_valid() {
        let trigger = PollingTrigger::new(
            "polling-001",
            Some("Data Poller".to_string()),
            Some("Polls for data".to_string()),
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
        assert_eq!(trigger.id.to_string(), "polling-001");
        assert_eq!(trigger.name, Some("Data Poller".to_string()));
        assert_eq!(trigger.description, Some("Polls for data".to_string()));
        assert_eq!(trigger.url, "https://api.example.com/data");
        assert_eq!(trigger.method, HttpMethod::Get);
        assert!(trigger.headers.is_none());
        assert!(!trigger.disabled);
    }

    #[test]
    fn pollingtrigger_new_returns_ok_with_minimal_fields() {
        let trigger = PollingTrigger::new(
            "abc",
            None,
            None,
            "http://example.com",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "1s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.id.to_string(), "abc");
        assert!(trigger.name.is_none());
        assert_eq!(trigger.url, "http://example.com");
        assert_eq!(trigger.method, HttpMethod::Get);
    }

    // Behavior 47: PollingTrigger::new returns Err when id is invalid
    #[test]
    fn pollingtrigger_new_returns_invalid_trigger_id_when_id_is_invalid() {
        let result = PollingTrigger::new(
            "ab",
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
        assert!(matches!(result, Err(TriggerDataError::InvalidTriggerId(_))));
    }

    // Behavior 48: PollingTrigger::new returns Err when url is invalid
    #[test]
    fn pollingtrigger_new_returns_invalid_url_when_url_is_invalid() {
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "not-a-url",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidUrl(_))));
    }

    // Behavior 49: PollingTrigger::new returns Err when interval is zero duration
    // Note: This test FAILS because GoDuration accepts "0s" as valid - implementation gap
    #[test]
    fn pollingtrigger_new_returns_invalid_interval_when_interval_is_zero() {
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
        // The contract says zero interval should return InvalidInterval error
        // But GoDuration::new accepts "0s" - this is an implementation gap
        assert!(matches!(result, Err(TriggerDataError::InvalidInterval(_))));
    }

    // Behavior 50: PollingTrigger::new returns Err when interval is invalid GoDuration format
    #[test]
    fn pollingtrigger_new_returns_invalid_interval_when_interval_is_invalid_format() {
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "not-a-duration",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidInterval(_))));
    }

    // Behavior 51: PollingTrigger::new returns Err when timeout is zero duration
    #[test]
    fn pollingtrigger_new_returns_invalid_interval_when_timeout_is_zero() {
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            Some(GoDuration::new("0s").unwrap()),
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidInterval(_))));
    }

    // Behavior 52: PollingTrigger::new returns Err when timeout is invalid GoDuration format
    // Note: We cannot construct an invalid GoDuration to pass to PollingTrigger::new
    // because GoDuration::new validates at construction time. The error would occur
    // at GoDuration construction, not at PollingTrigger::new.
    #[test]
    fn pollingtrigger_new_accepts_valid_timeout() {
        let timeout = GoDuration::new("5s").unwrap();
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            Some(timeout),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.timeout.as_ref().unwrap().to_string(), "5s");
    }

    // Behavior 53: PollingTrigger::new returns Err when stop_condition is invalid JMESPath
    #[test]
    fn pollingtrigger_new_returns_invalid_jmespath_when_stop_condition_is_invalid() {
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
            Some("not..valid..jmespath".to_string()),
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidJmespath(_))));
    }

    // Behavior 54: PollingTrigger::new returns Err when headers exceed limits
    #[test]
    fn pollingtrigger_new_returns_header_limit_exceeded_when_headers_exceed_limits() {
        let mut headers = HashMap::new();
        for i in 0..65 {
            headers.insert(format!("Header-{}", i), "value".to_string());
        }
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::HeaderLimitExceeded(_))
        ));
    }

    // Behavior 55-60: Auth field validation in PollingTrigger
    #[test]
    fn pollingtrigger_new_with_basic_auth_empty_username_returns_error() {
        let auth = WebhookAuth::Basic {
            username: "".to_string(),
            password: "pass".to_string(),
        };
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn pollingtrigger_new_with_basic_auth_empty_password_returns_error() {
        let auth = WebhookAuth::Basic {
            username: "user".to_string(),
            password: "".to_string(),
        };
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn pollingtrigger_new_with_bearer_auth_empty_token_returns_error() {
        let auth = WebhookAuth::Bearer {
            token: "".to_string(),
        };
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn pollingtrigger_new_with_api_key_auth_empty_key_returns_error() {
        let auth = WebhookAuth::ApiKey {
            key: "".to_string(),
            value: "abc123".to_string(),
            header_name: "X-API-Key".to_string(),
        };
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn pollingtrigger_new_with_api_key_auth_empty_value_returns_error() {
        let auth = WebhookAuth::ApiKey {
            key: "api-key".to_string(),
            value: "".to_string(),
            header_name: "X-API-Key".to_string(),
        };
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    #[test]
    fn pollingtrigger_new_with_api_key_auth_empty_header_name_returns_error() {
        let auth = WebhookAuth::ApiKey {
            key: "api-key".to_string(),
            value: "abc123".to_string(),
            header_name: "".to_string(),
        };
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::EmptyRequiredField(_))
        ));
    }

    // Behavior 61-63: Header limits in PollingTrigger
    #[test]
    fn pollingtrigger_new_returns_header_limit_exceeded_when_header_count_exceeds_64() {
        let mut headers = HashMap::new();
        for i in 0..65 {
            headers.insert(format!("Header-{}", i), "value".to_string());
        }
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            "30s",
            None,
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
    fn pollingtrigger_new_returns_header_limit_exceeded_when_header_name_exceeds_512_bytes() {
        let mut headers = HashMap::new();
        headers.insert("a".repeat(513), "value".to_string());
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            "30s",
            None,
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
    fn pollingtrigger_new_returns_header_limit_exceeded_when_header_value_exceeds_8192_bytes() {
        let mut headers = HashMap::new();
        headers.insert("Header".to_string(), "a".repeat(8193));
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            "30s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::HeaderLimitExceeded(_))
        ));
    }

    // Behavior 64: Header count exactly 64 is valid
    #[test]
    fn pollingtrigger_new_returns_ok_when_header_count_is_exactly_64() {
        let mut headers = HashMap::new();
        for i in 0..64 {
            headers.insert(format!("Header-{}", i), "value".to_string());
        }
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.headers.as_ref().unwrap().len(), 64);
    }

    // Behavior 65: PollingTrigger serializes with correct discriminant
    #[test]
    fn pollingtrigger_serializes_with_type_polling_discriminant() {
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
    }

    // Behavior 66: PollingTrigger deserialization produces correct variant
    #[test]
    fn pollingtrigger_deserialization_produces_correct_variant() {
        // Note: variant names are converted by rename_all = "camelCase", so "Polling" -> "polling"
        let json = r#"{"type":"polling","id":"polling-001","url":"https://api.example.com/data","method":"GET","interval":"30s"}"#;
        match serde_json::from_str::<Trigger>(json).unwrap() {
            Trigger::Polling(polling) => {
                assert_eq!(polling.id.to_string(), "polling-001");
            }
            _ => panic!("Expected Polling variant"),
        }
    }

    #[test]
    fn pollingtrigger_roundtrip_serialization() {
        let original = PollingTrigger::new(
            "polling-001",
            Some("Data Poller".to_string()),
            Some("Description".to_string()),
            "https://api.example.com/data",
            HttpMethod::Post,
            WebhookAuth::Bearer {
                token: "secret".to_string(),
            },
            Some(HashMap::from([(
                "Authorization".to_string(),
                "Bearer token".to_string(),
            )])),
            "1m",
            Some(GoDuration::new("5s").unwrap()),
            Some("data.complete == true".to_string()),
            Some(serde_json::json!({"key": "value"})),
            false,
        )
        .unwrap();
        let trigger = Trigger::Polling(original);

        let json = serde_json::to_string(&trigger).unwrap();
        let recovered: Trigger = serde_json::from_str(&json).unwrap();

        match recovered {
            Trigger::Polling(polling) => {
                assert_eq!(polling.id.to_string(), "polling-001");
            }
            _ => panic!("Expected Polling variant"),
        }
    }

    // Additional: valid interval formats
    #[test]
    fn pollingtrigger_new_accepts_1s_interval() {
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "1s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.interval.to_string(), "1s");
    }

    #[test]
    fn pollingtrigger_new_accepts_1m_interval() {
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "1m",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.interval.to_string(), "1m");
    }

    #[test]
    fn pollingtrigger_new_accepts_1h_interval() {
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "1h",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.interval.to_string(), "1h");
    }

    // Additional: valid JMESPath expressions
    #[test]
    fn pollingtrigger_new_accepts_valid_jmespath_simple() {
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            None,
            Some("data.status".to_string()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.stop_condition.as_ref().unwrap(), "data.status");
    }

    #[test]
    fn pollingtrigger_new_accepts_valid_jmespath_array_index() {
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            None,
            Some("data[0].status".to_string()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.stop_condition.as_ref().unwrap(), "data[0].status");
    }

    #[test]
    fn pollingtrigger_new_accepts_valid_jmespath_filter() {
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            None,
            Some("data[?status == 'complete']".to_string()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            trigger.stop_condition.as_ref().unwrap(),
            "data[?status == 'complete']"
        );
    }

    // With valid auth variants
    #[test]
    fn pollingtrigger_new_accepts_basic_auth() {
        let auth = WebhookAuth::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        match trigger.auth {
            WebhookAuth::Basic { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected Basic auth"),
        }
    }

    #[test]
    fn pollingtrigger_new_accepts_bearer_auth() {
        let auth = WebhookAuth::Bearer {
            token: "token123".to_string(),
        };
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        match trigger.auth {
            WebhookAuth::Bearer { token } => {
                assert_eq!(token, "token123");
            }
            _ => panic!("Expected Bearer auth"),
        }
    }

    #[test]
    fn pollingtrigger_new_accepts_api_key_auth() {
        let auth = WebhookAuth::ApiKey {
            key: "api-key".to_string(),
            value: "abc123".to_string(),
            header_name: "X-API-Key".to_string(),
        };
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            auth,
            None,
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        match trigger.auth {
            WebhookAuth::ApiKey {
                key,
                value,
                header_name,
            } => {
                assert_eq!(key, "api-key");
                assert_eq!(value, "abc123");
                assert_eq!(header_name, "X-API-Key");
            }
            _ => panic!("Expected ApiKey auth"),
        }
    }

    // Negative interval
    #[test]
    fn pollingtrigger_new_rejects_negative_interval() {
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "-1s",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidInterval(_))));
    }
}

// =============================================================================
// Trigger Root Enum Tests (Behaviors 67-71)
// =============================================================================

mod trigger_root_enum_tests {
    use super::*;

    // Behavior 67: Trigger::Cron roundtrip
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
        let trigger = Trigger::Cron(cron.clone());

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"cron\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        match recovered {
            Trigger::Cron(cron_recovered) => {
                assert_eq!(cron.id, cron_recovered.id);
                assert_eq!(cron.name, cron_recovered.name);
                assert_eq!(cron.cron.to_string(), cron_recovered.cron.to_string());
            }
            _ => panic!("Expected Cron variant"),
        }
    }

    // Behavior 68: Trigger::Webhook roundtrip
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
        let trigger = Trigger::Webhook(webhook.clone());

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"webhook\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        match recovered {
            Trigger::Webhook(webhook_recovered) => {
                assert_eq!(webhook.id, webhook_recovered.id);
                assert_eq!(webhook.url, webhook_recovered.url);
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    // Behavior 69: Trigger::Polling roundtrip
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
        let trigger = Trigger::Polling(polling.clone());

        let json = serde_json::to_string(&trigger).unwrap();
        assert!(json.contains("\"type\":\"polling\""));

        let recovered: Trigger = serde_json::from_str(&json).unwrap();
        match recovered {
            Trigger::Polling(polling_recovered) => {
                assert_eq!(polling.id, polling_recovered.id);
                assert_eq!(polling.url, polling_recovered.url);
            }
            _ => panic!("Expected Polling variant"),
        }
    }

    // Behavior 70: Trigger deserialization selects correct variant based on "type" field
    #[test]
    fn trigger_deserialization_selects_cron_variant() {
        // Note: variant names are converted by rename_all = "camelCase", so "Cron" -> "cron"
        let json = r#"{"type":"cron","id":"trigger-001","cron":"0 0 * * * *","timezone":"UTC"}"#;
        let result: Trigger = serde_json::from_str(json).unwrap();
        assert!(matches!(result, Trigger::Cron(_)));
    }

    #[test]
    fn trigger_deserialization_selects_webhook_variant() {
        // Note: variant names are converted by rename_all = "camelCase", so "Webhook" -> "webhook"
        let json = r#"{"type":"webhook","id":"webhook-001","url":"https://example.com/hook","method":"GET"}"#;
        let result: Trigger = serde_json::from_str(json).unwrap();
        assert!(matches!(result, Trigger::Webhook(_)));
    }

    #[test]
    fn trigger_deserialization_selects_polling_variant() {
        // Note: variant names are converted by rename_all = "camelCase", so "Polling" -> "polling"
        let json = r#"{"type":"polling","id":"polling-001","url":"https://api.example.com/data","method":"GET","interval":"30s"}"#;
        let result: Trigger = serde_json::from_str(json).unwrap();
        assert!(matches!(result, Trigger::Polling(_)));
    }

    // Behavior 71: Trigger serialization omits None Option fields
    #[test]
    fn trigger_omits_none_option_fields_cron() {
        let cron = CronTrigger::new(
            "trigger-001",
            None, // name is None
            None, // description is None
            "0 0 * * * *",
            "UTC",
            false,
            None, // payload is None
        )
        .unwrap();
        let trigger = Trigger::Cron(cron);
        let json = serde_json::to_string(&trigger).unwrap();

        // Should NOT contain "name", "description", "payload" fields
        assert!(!json.contains("\"name\""));
        assert!(!json.contains("\"description\""));
        assert!(!json.contains("\"payload\""));
    }

    #[test]
    fn trigger_omits_none_option_fields_webhook() {
        let webhook = WebhookTrigger::new(
            "webhook-001",
            None, // name
            None, // description
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            None, // headers
            None, // body_template
            None, // payload
            false,
        )
        .unwrap();
        let trigger = Trigger::Webhook(webhook);
        let json = serde_json::to_string(&trigger).unwrap();

        assert!(!json.contains("\"name\""));
        assert!(!json.contains("\"description\""));
        assert!(!json.contains("\"headers\""));
        assert!(!json.contains("\"bodyTemplate\""));
        assert!(!json.contains("\"payload\""));
    }

    #[test]
    fn trigger_omits_none_option_fields_polling() {
        let polling = PollingTrigger::new(
            "polling-001",
            None, // name
            None, // description
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None, // headers
            "30s",
            None, // timeout
            None, // stop_condition
            None, // payload
            false,
        )
        .unwrap();
        let trigger = Trigger::Polling(polling);
        let json = serde_json::to_string(&trigger).unwrap();

        assert!(!json.contains("\"name\""));
        assert!(!json.contains("\"description\""));
        assert!(!json.contains("\"headers\""));
        assert!(!json.contains("\"timeout\""));
        assert!(!json.contains("\"stopCondition\""));
        assert!(!json.contains("\"payload\""));
    }

    // Additional: JSON discriminant field is "type"
    #[test]
    fn trigger_serialized_json_has_type_field() {
        let cron =
            CronTrigger::new("trigger-001", None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        let trigger = Trigger::Cron(cron);
        let json = serde_json::to_string(&trigger).unwrap();

        // Check that "type" field exists
        assert!(json.contains("\"type\""));
    }

    // Additional: unknown type discriminant returns error
    #[test]
    fn trigger_deserialization_fails_with_unknown_type() {
        let json = r#"{"type":"Unknown","id":"test"}"#;
        let error = serde_json::from_str::<Trigger>(json).unwrap_err();
        assert_eq!(
            error.to_string(),
            "unknown variant `Unknown`, expected one of `cron`, `webhook`, `polling` at line 1 column 17"
        );
    }

    // Additional: missing type field returns error
    #[test]
    fn trigger_deserialization_fails_without_type_field() {
        let json = r#"{"id":"test","cron":"0 0 * * * *","timezone":"UTC"}"#;
        let error = serde_json::from_str::<Trigger>(json).unwrap_err();
        assert_eq!(
            error.to_string(),
            "missing field `type` at line 1 column 51"
        );
    }

    // CamelCase field naming - note: fields are already camelCase in the struct,
    // so rename_all doesn't change them (id stays id, not triggerId)
    #[test]
    fn trigger_cron_has_camelcase_fields_in_json() {
        let cron = CronTrigger::new(
            "trigger-001",
            Some("Test".to_string()),
            None,
            "0 0 * * * *",
            "UTC",
            false,
            None,
        )
        .unwrap();
        let trigger = Trigger::Cron(cron);
        let json = serde_json::to_string(&trigger).unwrap();

        // With rename_all = "camelCase", fields that are already camelCase stay as-is
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"cron\""));
        assert!(json.contains("\"timezone\""));
        assert!(json.contains("\"type\":\"cron\"")); // type discriminant should be present
    }
}

// =============================================================================
// Error Type Coverage Tests
// =============================================================================

mod error_type_coverage_tests {
    use super::*;

    // All 9 error variants should be testable

    #[test]
    fn trigger_data_error_invalid_trigger_id() {
        let err = TriggerDataError::InvalidTriggerId(IdError::TooShort(2));
        assert!(format!("{}", err).contains("invalid trigger ID"));
    }

    #[test]
    fn trigger_data_error_invalid_cron_expression() {
        let err = TriggerDataError::InvalidCronExpression(CronExpressionError::ParseError(
            "bad cron".to_string(),
        ));
        assert!(format!("{}", err).contains("invalid cron expression"));
    }

    #[test]
    fn trigger_data_error_invalid_interval() {
        let err = TriggerDataError::InvalidInterval(GoDurationError::Empty);
        assert!(format!("{}", err).contains("invalid interval"));
    }

    #[test]
    fn trigger_data_error_invalid_timezone() {
        let err = TriggerDataError::InvalidTimezone("Invalid/TZ".to_string());
        assert!(format!("{}", err).contains("invalid timezone"));
    }

    #[test]
    fn trigger_data_error_invalid_url() {
        let err = TriggerDataError::InvalidUrl("not-a-url".to_string());
        assert!(format!("{}", err).contains("invalid URL"));
    }

    #[test]
    fn trigger_data_error_invalid_http_method() {
        let err = TriggerDataError::InvalidHttpMethod("INVALID".to_string());
        assert!(format!("{}", err).contains("invalid HTTP method"));
    }

    #[test]
    fn trigger_data_error_empty_required_field() {
        let err = TriggerDataError::EmptyRequiredField("test_field".to_string());
        assert!(format!("{}", err).contains("empty required field"));
    }

    #[test]
    fn trigger_data_error_invalid_jmespath() {
        let err = TriggerDataError::InvalidJmespath("invalid".to_string());
        assert!(format!("{}", err).contains("invalid JMESPath expression"));
    }

    #[test]
    fn trigger_data_error_header_limit_exceeded() {
        let err = TriggerDataError::HeaderLimitExceeded("test".to_string());
        assert!(format!("{}", err).contains("header limit exceeded"));
    }

    // Error equality
    #[test]
    fn trigger_data_error_invalid_http_method_equality() {
        let err1 = TriggerDataError::InvalidHttpMethod("TEST".to_string());
        let err2 = TriggerDataError::InvalidHttpMethod("TEST".to_string());
        let err3 = TriggerDataError::InvalidHttpMethod("OTHER".to_string());

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn trigger_data_error_empty_required_field_equality() {
        let err1 = TriggerDataError::EmptyRequiredField("field1".to_string());
        let err2 = TriggerDataError::EmptyRequiredField("field1".to_string());
        let err3 = TriggerDataError::EmptyRequiredField("field2".to_string());

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    // Clone and debug
    #[test]
    fn trigger_data_error_is_cloneable() {
        let err = TriggerDataError::InvalidHttpMethod("TEST".to_string());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn trigger_data_error_has_debug_representation() {
        let err = TriggerDataError::InvalidHttpMethod("TEST".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidHttpMethod"));
    }
}

// =============================================================================
// Additional Coverage Tests for trigger/data.rs
// =============================================================================

mod additional_coverage_tests {
    use super::*;

    // Test CronTrigger with disabled = true
    #[test]
    fn cronscheduler_new_accepts_disabled_true() {
        let trigger =
            CronTrigger::new("trigger-001", None, None, "0 0 * * * *", "UTC", true, None).unwrap();
        assert!(trigger.disabled);
    }

    // Test CronTrigger with payload set
    #[test]
    fn cronscheduler_new_accepts_payload() {
        let payload = serde_json::json!({"key": "value", "nested": {"a": 1}});
        let trigger = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "UTC",
            false,
            Some(payload.clone()),
        )
        .unwrap();
        assert_eq!(trigger.payload, Some(payload));
    }

    // Test WebhookTrigger with all optional fields set
    #[test]
    fn webhooktrigger_new_with_all_optional_fields() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Custom".to_string(), "value".to_string());

        let payload = serde_json::json!({"webhook": "data"});
        let trigger = WebhookTrigger::new(
            "webhook-001",
            Some("My Webhook".to_string()),
            Some("Description".to_string()),
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            Some(headers),
            Some("{{ body }}".to_string()),
            Some(payload),
            true, // disabled = true
        )
        .unwrap();

        assert_eq!(trigger.id.to_string(), "webhook-001");
        assert_eq!(trigger.name, Some("My Webhook".to_string()));
        assert_eq!(trigger.description, Some("Description".to_string()));
        assert_eq!(trigger.url, "https://example.com/hook");
        assert_eq!(trigger.method, HttpMethod::Post);
        assert_eq!(trigger.auth, WebhookAuth::None);
        assert_eq!(trigger.headers.as_ref().unwrap().len(), 2);
        assert_eq!(trigger.body_template.as_ref().unwrap(), "{{ body }}");
        assert!(trigger.disabled);
    }

    // Test PollingTrigger with all optional fields set
    #[test]
    fn pollingtrigger_new_with_all_optional_fields() {
        let headers = HashMap::from([("X-Header".to_string(), "value".to_string())]);
        let payload = serde_json::json!({"polling": "data"});

        let trigger = PollingTrigger::new(
            "polling-001",
            Some("My Poller".to_string()),
            Some("Description".to_string()),
            "https://api.example.com/data",
            HttpMethod::Post,
            WebhookAuth::None,
            Some(headers),
            "1m",
            Some(GoDuration::new("30s").unwrap()),
            Some("data.complete".to_string()),
            Some(payload),
            true, // disabled = true
        )
        .unwrap();

        assert_eq!(trigger.id.to_string(), "polling-001");
        assert_eq!(trigger.name, Some("My Poller".to_string()));
        assert_eq!(trigger.description, Some("Description".to_string()));
        assert_eq!(trigger.url, "https://api.example.com/data");
        assert_eq!(trigger.method, HttpMethod::Post);
        assert_eq!(trigger.headers.as_ref().unwrap().len(), 1);
        assert_eq!(trigger.interval.to_string(), "1m");
        assert_eq!(trigger.timeout.as_ref().unwrap().to_string(), "30s");
        assert_eq!(trigger.stop_condition.as_ref().unwrap(), "data.complete");
        assert!(trigger.disabled);
    }

    // Test that PollingTrigger rejects large timeout values (> 0)
    #[test]
    fn pollingtrigger_new_accepts_large_timeout() {
        let timeout = GoDuration::new("3600s").unwrap(); // 1 hour
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "30s",
            Some(timeout),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.timeout.as_ref().unwrap().to_string(), "3600s");
    }

    // Test webhook with very long URL (boundary)
    #[test]
    fn webhooktrigger_new_accepts_long_url() {
        let long_path = "a".repeat(2000);
        let url = format!("https://example.com/{}", long_path);
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            &url,
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert!(trigger.url.len() > 2000);
    }

    // Test polling with very long URL (boundary)
    #[test]
    fn pollingtrigger_new_accepts_long_url() {
        let long_path = "a".repeat(2000);
        let url = format!("https://api.example.com/{}", long_path);
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            &url,
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
        assert!(trigger.url.len() > 2000);
    }

    // Test webhook with single character header value (boundary)
    #[test]
    fn webhooktrigger_new_accepts_single_char_header_value() {
        let mut headers = HashMap::new();
        headers.insert("X-Key".to_string(), "x".to_string());
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            trigger.headers.as_ref().unwrap().values().next().unwrap(),
            "x"
        );
    }

    // Test polling with single character header value (boundary)
    #[test]
    fn pollingtrigger_new_accepts_single_char_header_value() {
        let mut headers = HashMap::new();
        headers.insert("X-Key".to_string(), "x".to_string());
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            trigger.headers.as_ref().unwrap().values().next().unwrap(),
            "x"
        );
    }

    // Test webhook with 1 header (boundary minimum)
    #[test]
    fn webhooktrigger_new_accepts_single_header() {
        let mut headers = HashMap::new();
        headers.insert("X-Header".to_string(), "value".to_string());
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.headers.as_ref().unwrap().len(), 1);
    }

    // Test polling with 1 header (boundary minimum)
    #[test]
    fn pollingtrigger_new_accepts_single_header() {
        let mut headers = HashMap::new();
        headers.insert("X-Header".to_string(), "value".to_string());
        let trigger = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            Some(headers),
            "30s",
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.headers.as_ref().unwrap().len(), 1);
    }

    // Test webhook body_template is preserved
    #[test]
    fn webhooktrigger_new_preserves_body_template() {
        let trigger = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            Some("{{ template }}".to_string()),
            None,
            false,
        )
        .unwrap();
        assert_eq!(trigger.body_template.as_ref().unwrap(), "{{ template }}");
    }

    // Test CronTrigger timezone preservation
    #[test]
    fn cronscheduler_new_preserves_timezone() {
        let trigger = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "Europe/Paris",
            false,
            None,
        )
        .unwrap();
        assert_eq!(trigger.timezone, "Europe/Paris");
    }

    // Test that GoDuration error formats are correct
    #[test]
    fn pollingtrigger_new_rejects_invalid_go_duration_format() {
        // "1w" is not a valid Go duration format
        let result = PollingTrigger::new(
            "polling-001",
            None,
            None,
            "https://api.example.com/data",
            HttpMethod::Get,
            WebhookAuth::None,
            None,
            "1w",
            None,
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(TriggerDataError::InvalidInterval(_))));
    }

    // Test various invalid cron expressions
    #[test]
    fn cronscheduler_new_rejects_cron_with_invalid_characters() {
        // Cron expression with invalid characters
        let result = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "abc def ghi jkl mno pqr",
            "UTC",
            false,
            None,
        );
        assert!(matches!(
            result,
            Err(TriggerDataError::InvalidCronExpression(_))
        ));
    }

    #[test]
    fn cronscheduler_new_rejects_cron_with_too_few_fields() {
        // 4 fields is invalid (5-field cron is normalized to 6-field by domain::CronExpression)
        let result = CronTrigger::new("trigger-001", None, None, "0 * * *", "UTC", false, None);
        assert!(matches!(
            result,
            Err(TriggerDataError::InvalidCronExpression(_))
        ));
    }

    // Test that cron expressions are preserved as entered
    #[test]
    fn cronscheduler_new_preserves_cron_expression() {
        let trigger =
            CronTrigger::new("trigger-001", None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        assert_eq!(trigger.cron.to_string(), "0 0 * * * *");
    }

    // Test that Payload is properly serialized and deserialized for CronTrigger
    #[test]
    fn cronscheduler_payload_roundtrip() {
        let payload = serde_json::json!({"complex": {"nested": [1, 2, 3]}});
        let cron = CronTrigger::new(
            "trigger-001",
            None,
            None,
            "0 0 * * * *",
            "UTC",
            false,
            Some(payload.clone()),
        )
        .unwrap();

        let json = serde_json::to_string(&cron).unwrap();
        let recovered: CronTrigger = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.payload, Some(payload));
    }

    // Test that Payload is properly serialized and deserialized for WebhookTrigger
    #[test]
    fn webhooktrigger_payload_roundtrip() {
        let payload = serde_json::json!({"webhook": "test"});
        let webhook = WebhookTrigger::new(
            "webhook-001",
            None,
            None,
            "https://example.com/hook",
            HttpMethod::Post,
            WebhookAuth::None,
            None,
            None,
            Some(payload.clone()),
            false,
        )
        .unwrap();

        let json = serde_json::to_string(&webhook).unwrap();
        let recovered: WebhookTrigger = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.payload, Some(payload));
    }

    // Test that Payload is properly serialized and deserialized for PollingTrigger
    #[test]
    fn pollingtrigger_payload_roundtrip() {
        let payload = serde_json::json!({"polling": "test"});
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
            Some(payload.clone()),
            false,
        )
        .unwrap();

        let json = serde_json::to_string(&polling).unwrap();
        let recovered: PollingTrigger = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.payload, Some(payload));
    }

    // Test that disabled field serializes correctly for CronTrigger
    #[test]
    fn cronscheduler_disabled_serialize_default() {
        // Note: #[serde(default)] only affects deserialization, not serialization.
        // The field will still be serialized even when false.
        let cron =
            CronTrigger::new("trigger-001", None, None, "0 0 * * * *", "UTC", false, None).unwrap();
        let json = serde_json::to_string(&cron).unwrap();
        // disabled=false is serialized as "disabled":false
        assert!(json.contains("\"disabled\":false"));
    }

    // Test that disabled = true serializes correctly
    #[test]
    fn cronscheduler_disabled_serializes_when_true() {
        let cron =
            CronTrigger::new("trigger-001", None, None, "0 0 * * * *", "UTC", true, None).unwrap();
        let json = serde_json::to_string(&cron).unwrap();
        assert!(json.contains("\"disabled\":true"));
    }

    // Test CronTrigger deserialization with all fields
    #[test]
    fn cronscheduler_deserialization_with_all_fields() {
        let json = r#"{"id":"trigger-001","name":"Test","description":"A test trigger","cron":"0 0 * * * *","timezone":"America/New_York","disabled":true,"payload":{"key":"value"}}"#;
        let trigger: CronTrigger = serde_json::from_str(json).unwrap();
        assert_eq!(trigger.id.to_string(), "trigger-001");
        assert_eq!(trigger.name.as_ref().unwrap(), "Test");
        assert_eq!(trigger.description.as_ref().unwrap(), "A test trigger");
        assert!(trigger.disabled);
        assert_eq!(
            trigger.payload.as_ref().unwrap().get("key").unwrap(),
            "value"
        );
    }

    // Test WebhookTrigger deserialization with all fields
    #[test]
    fn webhooktrigger_deserialization_with_all_fields() {
        let json = r#"{"id":"webhook-001","name":"Test","url":"https://example.com/hook","method":"POST","headers":{"X-Test":"value"},"bodyTemplate":"{{ body }}","disabled":true}"#;
        let trigger: WebhookTrigger = serde_json::from_str(json).unwrap();
        assert_eq!(trigger.id.to_string(), "webhook-001");
        assert_eq!(trigger.name.as_ref().unwrap(), "Test");
        assert!(trigger.disabled);
        assert_eq!(
            trigger.headers.as_ref().unwrap().get("X-Test").unwrap(),
            "value"
        );
        assert_eq!(trigger.body_template.as_ref().unwrap(), "{{ body }}");
    }

    // Test PollingTrigger deserialization with all fields
    #[test]
    fn pollingtrigger_deserialization_with_all_fields() {
        let json = r#"{"id":"polling-001","name":"Test","url":"https://api.example.com/data","method":"GET","headers":{"X-Test":"value"},"interval":"1m","timeout":"30s","stopCondition":"data.complete","disabled":true}"#;
        let trigger: PollingTrigger = serde_json::from_str(json).unwrap();
        assert_eq!(trigger.id.to_string(), "polling-001");
        assert_eq!(trigger.name.as_ref().unwrap(), "Test");
        assert!(trigger.disabled);
        assert_eq!(trigger.interval.to_string(), "1m");
        assert_eq!(trigger.timeout.as_ref().unwrap().to_string(), "30s");
        assert_eq!(trigger.stop_condition.as_ref().unwrap(), "data.complete");
    }

    // Test Trigger enum deserialization with different variants
    #[test]
    fn trigger_deserialization_cron_with_full_fields() {
        let json = r#"{"type":"cron","id":"trigger-001","cron":"0 0 * * * *","timezone":"UTC","disabled":true}"#;
        let result: Trigger = serde_json::from_str(json).unwrap();
        match result {
            Trigger::Cron(cron) => {
                assert_eq!(cron.id.to_string(), "trigger-001");
                assert!(cron.disabled);
            }
            _ => panic!("Expected Cron variant"),
        }
    }

    #[test]
    fn trigger_deserialization_webhook_with_full_fields() {
        let json = r#"{"type":"webhook","id":"webhook-001","url":"https://example.com/hook","method":"GET","disabled":false}"#;
        let result: Trigger = serde_json::from_str(json).unwrap();
        match result {
            Trigger::Webhook(webhook) => {
                assert_eq!(webhook.id.to_string(), "webhook-001");
                assert!(!webhook.disabled);
            }
            _ => panic!("Expected Webhook variant"),
        }
    }

    #[test]
    fn trigger_deserialization_polling_with_full_fields() {
        let json = r#"{"type":"polling","id":"polling-001","url":"https://api.example.com/data","method":"GET","interval":"30s","disabled":true}"#;
        let result: Trigger = serde_json::from_str(json).unwrap();
        match result {
            Trigger::Polling(polling) => {
                assert_eq!(polling.id.to_string(), "polling-001");
                assert!(polling.disabled);
            }
            _ => panic!("Expected Polling variant"),
        }
    }

    // Test that we can deserialize with missing optional fields
    #[test]
    fn webhooktrigger_deserialization_minimal() {
        let json = r#"{"id":"webhook-001","url":"https://example.com/hook","method":"POST"}"#;
        let trigger: WebhookTrigger = serde_json::from_str(json).unwrap();
        assert_eq!(trigger.id.to_string(), "webhook-001");
        assert!(trigger.name.is_none());
        assert!(trigger.description.is_none());
        assert_eq!(trigger.auth, WebhookAuth::None);
        assert!(trigger.headers.is_none());
        assert!(trigger.body_template.is_none());
        assert!(!trigger.disabled);
    }

    #[test]
    fn pollingtrigger_deserialization_minimal() {
        let json = r#"{"id":"polling-001","url":"https://api.example.com/data","method":"GET","interval":"30s"}"#;
        let trigger: PollingTrigger = serde_json::from_str(json).unwrap();
        assert_eq!(trigger.id.to_string(), "polling-001");
        assert!(trigger.name.is_none());
        assert!(trigger.description.is_none());
        assert_eq!(trigger.auth, WebhookAuth::None);
        assert!(trigger.headers.is_none());
        assert!(trigger.timeout.is_none());
        assert!(trigger.stop_condition.is_none());
        assert!(!trigger.disabled);
    }
}
