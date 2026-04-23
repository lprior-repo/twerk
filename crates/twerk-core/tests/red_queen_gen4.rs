//! Red Queen Adversarial Test Suite — Generation 4
//!
//! Co-evolution: This generation probes domain types and type invariants.
//! These tests attempt to break:
//! - Domain types (CronExpression, Hostname, WebhookUrl)
//! - Type invariants and serialization contracts

use twerk_core::domain::{CronExpression, GoDuration, Hostname, Priority, QueueName, WebhookUrl};
use twerk_core::trigger::types::{Trigger, TriggerVariant};

// =========================================================================
// DIMENSION 1: CronExpression adversarial validation
// =========================================================================

mod cron_adversarial {
    use super::*;

    #[test]
    fn cron_empty_string_rejected() {
        let result: Result<CronExpression, _> = "".parse();
        assert_eq!(result, Err(twerk_core::domain::CronExpressionError::Empty));
    }

    #[test]
    fn cron_invalid_expression_rejected() {
        let invalid = [
            "* * * *",
            "* * * * * * *",
            "invalid",
            "60 * * * *",
            "* 25 * * *",
            "* * 32 * *",
            "* * * 13 *",
            "* * * * 8",
        ];
        for expr in invalid {
            let result: Result<CronExpression, _> = expr.parse();
            let err = result.expect_err(&format!("Should reject invalid cron: {expr}"));
            assert!(
                matches!(
                    err,
                    twerk_core::domain::CronExpressionError::InvalidFieldCount(_)
                        | twerk_core::domain::CronExpressionError::ParseError(_)
                ),
                "unexpected cron error for {expr}: {err:?}"
            );
        }
    }

    #[test]
    fn cron_valid_expressions_accepted() {
        let valid = [
            "* * * * *",
            "0 * * * *",
            "0 0 * * *",
            "*/5 * * * *",
            "0 */2 * * *",
            "0 0 1 * *",
        ];
        for expr in valid {
            let result: Result<CronExpression, _> = expr.parse();
            assert_eq!(
                result
                    .unwrap_or_else(|_| panic!("Should accept valid cron: {expr}"))
                    .as_str(),
                expr
            );
        }
    }

    #[test]
    fn cron_whitespace_trimmed() {
        let with_whitespace = "  * * * * *  ";
        let result: Result<CronExpression, _> = with_whitespace.parse();
        assert_eq!(
            result.expect("Should trim whitespace").as_str(),
            with_whitespace
        );
    }

    #[test]
    fn cron_basic_numeric_accepted() {
        let basic: Result<CronExpression, _> = "0 0 1 * *".parse();
        assert_eq!(basic.unwrap().as_str(), "0 0 1 * *");
    }

    #[test]
    fn cron_to_string_roundtrip() {
        let expr = "* * * * *".parse::<CronExpression>().unwrap();
        let roundtrip = expr.to_string();
        assert_eq!(
            roundtrip.parse::<CronExpression>().unwrap().as_str(),
            "* * * * *"
        );
    }
}

// =========================================================================
// DIMENSION 2: Hostname adversarial validation
// =========================================================================

mod hostname_adversarial {
    use super::*;

    #[test]
    fn hostname_empty_rejected() {
        let result: Result<Hostname, _> = "".parse();
        assert_eq!(result, Err(twerk_core::domain::HostnameError::Empty));
    }

    #[test]
    fn hostname_localhost_accepted() {
        let result: Result<Hostname, _> = "localhost".parse();
        assert_eq!(result.as_ref().map(Hostname::as_str), Ok("localhost"));
    }

    #[test]
    fn hostname_single_label_accepted() {
        let result: Result<Hostname, _> = "a".parse();
        assert_eq!(result.as_ref().map(Hostname::as_str), Ok("a"));
    }

    #[test]
    fn hostname_max_length_63_chars() {
        let max = "a".repeat(63);
        let result: Result<Hostname, _> = Hostname::new(&max);
        assert_eq!(result.as_ref().map(Hostname::as_str), Ok(max.as_str()));
    }

    #[test]
    fn hostname_64_chars_rejected() {
        let over = "a".repeat(64);
        let result: Result<Hostname, _> = Hostname::new(&over);
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::LabelTooLong(
                over.clone(),
                64
            ))
        );
    }

    #[test]
    fn hostname_labels_255_chars_rejected() {
        let max_label = "a".repeat(255);
        let result: Result<Hostname, _> = Hostname::new(&max_label);
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::TooLong(255)),
            "Individual labels are max 63 chars, not 255"
        );
    }

    #[test]
    fn hostname_label_63_chars_accepted() {
        let max_label = "a".repeat(63);
        let result: Result<Hostname, _> = Hostname::new(&max_label);
        assert_eq!(result.unwrap().as_str(), max_label);
    }

    #[test]
    fn hostname_label_256_chars_rejected() {
        let over_label = "a".repeat(256);
        let result: Result<Hostname, _> = Hostname::new(&over_label);
        assert_eq!(result, Err(twerk_core::domain::HostnameError::TooLong(256)));
    }

    #[test]
    fn hostname_starts_with_hyphen_rejected() {
        let result: Result<Hostname, _> = Hostname::new("-example.com");
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::InvalidLabel(
                "-example".to_string(),
                "must start with alphanumeric".to_string()
            ))
        );
    }

    #[test]
    fn hostname_ends_with_hyphen_rejected() {
        let result: Result<Hostname, _> = Hostname::new("example-.com");
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::InvalidLabel(
                "example-".to_string(),
                "must end with alphanumeric".to_string()
            ))
        );
    }

    #[test]
    fn hostname_double_hyphen_in_ip_v6_rejected() {
        let result: Result<Hostname, _> = Hostname::new("fe80::1");
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::InvalidCharacter(':'))
        );
    }

    #[test]
    fn hostname_with_numbers_accepted() {
        let result: Result<Hostname, _> = Hostname::new("example123.com");
        assert_eq!(result.unwrap().as_str(), "example123.com");
    }

    #[test]
    fn hostname_with_dash_accepted() {
        let result: Result<Hostname, _> = Hostname::new("my-example.com");
        assert_eq!(result.unwrap().as_str(), "my-example.com");
    }

    #[test]
    fn hostname_with_underscore_rejected() {
        let result: Result<Hostname, _> = Hostname::new("my_example.com");
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::InvalidCharacter('_'))
        );
    }

    #[test]
    fn hostname_null_byte_rejected() {
        let result: Result<Hostname, _> = Hostname::new("exam\0ple.com");
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::InvalidCharacter('\0'))
        );
    }

    #[test]
    fn hostname_control_char_rejected() {
        let result: Result<Hostname, _> = Hostname::new("exam\x01ple.com");
        assert_eq!(
            result,
            Err(twerk_core::domain::HostnameError::InvalidCharacter('\x01'))
        );
    }

    #[test]
    fn hostname_display_matches_input() {
        let input = "example.com";
        let hostname = Hostname::new(input).unwrap();
        assert_eq!(hostname.to_string(), input);
    }

    #[test]
    fn hostname_subdomain_accepted() {
        let result: Result<Hostname, _> = Hostname::new("sub.example.com");
        assert_eq!(result.unwrap().as_str(), "sub.example.com");
    }
}

// =========================================================================
// DIMENSION 3: WebhookUrl adversarial validation
// =========================================================================

mod webhook_url_adversarial {
    use super::*;

    #[test]
    fn webhook_url_empty_rejected() {
        let result: Result<WebhookUrl, _> = "".parse();
        assert!(matches!(
            result,
            Err(twerk_core::domain::WebhookUrlError::UrlParseError(_))
        ));
    }

    #[test]
    fn webhook_url_missing_scheme_rejected() {
        let result: Result<WebhookUrl, _> = "example.com/webhook".parse();
        assert!(matches!(
            result,
            Err(twerk_core::domain::WebhookUrlError::UrlParseError(_))
        ));
    }

    #[test]
    fn webhook_url_http_accepted() {
        let result: Result<WebhookUrl, _> = "http://example.com/webhook".parse();
        assert_eq!(result.unwrap().as_str(), "http://example.com/webhook");
    }

    #[test]
    fn webhook_url_https_accepted() {
        let result: Result<WebhookUrl, _> = "https://example.com/webhook".parse();
        assert_eq!(result.unwrap().as_str(), "https://example.com/webhook");
    }

    #[test]
    fn webhook_url_https_required_for_production() {
        let https_result: Result<WebhookUrl, _> = "https://example.com/webhook".parse();
        assert_eq!(
            https_result.unwrap().as_str(),
            "https://example.com/webhook"
        );
    }

    #[test]
    fn webhook_url_localhost_accepted() {
        let result: Result<WebhookUrl, _> = "https://localhost/webhook".parse();
        assert_eq!(result.unwrap().as_str(), "https://localhost/webhook");
    }

    #[test]
    fn webhook_url_with_port_accepted() {
        let result: Result<WebhookUrl, _> = "https://example.com:8080/webhook".parse();
        assert_eq!(result.unwrap().as_str(), "https://example.com:8080/webhook");
    }

    #[test]
    fn webhook_url_null_byte_rejected() {
        let result: Result<WebhookUrl, _> = "https://example.com/web\x00hook".parse();
        assert_eq!(
            result,
            Err(twerk_core::domain::WebhookUrlError::ControlCharacterInPath(
                0
            ))
        );
    }

    #[test]
    fn webhook_url_control_char_rejected() {
        let result: Result<WebhookUrl, _> = "https://example.com/web\x01hook".parse();
        assert_eq!(
            result,
            Err(twerk_core::domain::WebhookUrlError::ControlCharacterInPath(
                1
            ))
        );
    }

    #[test]
    fn webhook_url_display_roundtrip() {
        let input = "https://example.com/webhook";
        let url = WebhookUrl::new(input).unwrap();
        assert_eq!(url.to_string(), input);
    }
}

// =========================================================================
// DIMENSION 4: DomainTypes — Priority, QueueName, RetryLimit, GoDuration
// =========================================================================

mod domain_types_adversarial {
    use super::*;

    #[test]
    fn priority_min_value() {
        let result = Priority::new(0);
        assert_eq!(result.unwrap().to_string(), "0");
    }

    #[test]
    fn priority_max_value() {
        let result = Priority::new(9);
        assert_eq!(result.unwrap().to_string(), "9");
    }

    #[test]
    fn priority_negative_rejected() {
        let result = Priority::new(-1);
        assert_eq!(
            result,
            Err(twerk_core::domain::PriorityError::OutOfRange(-1))
        );
    }

    #[test]
    fn queue_name_empty_rejected() {
        let result: Result<QueueName, _> = "".parse();
        assert_eq!(
            result,
            Err(twerk_core::domain::QueueNameError::InvalidLength(0))
        );
    }

    #[test]
    fn queue_name_valid() {
        let result: Result<QueueName, _> = "default".parse();
        assert_eq!(result.unwrap().as_str(), "default");
    }

    #[test]
    fn queue_name_with_slash_rejected() {
        let result: Result<QueueName, _> = "my/queue".parse();
        assert_eq!(
            result,
            Err(twerk_core::domain::QueueNameError::InvalidCharacter),
            "QueueName only allows lowercase alphanumeric, hyphens, underscores, dots - no slash"
        );
    }

    #[test]
    fn queue_name_special_chars_rejected() {
        let invalid = ["my queue", "my:queue", "my@queue"];
        for q in invalid {
            let result: Result<QueueName, _> = q.parse();
            assert_eq!(
                result,
                Err(twerk_core::domain::QueueNameError::InvalidCharacter),
                "Should reject: {q}"
            );
        }
    }

    #[test]
    fn retry_limit_zero_rejected() {
        let result = twerk_core::validation::parse_retry(0);
        assert_eq!(
            result,
            Err(twerk_core::domain::DomainParseError::RetryLimit(
                twerk_core::domain::ParseRetryError::OutOfRange(0)
            )),
            "parse_retry enforces 1-10, not 0"
        );
    }

    #[test]
    fn retry_limit_large_rejected() {
        let result = twerk_core::validation::parse_retry(1000);
        assert_eq!(
            result,
            Err(twerk_core::domain::DomainParseError::RetryLimit(
                twerk_core::domain::ParseRetryError::OutOfRange(1000)
            )),
            "parse_retry enforces 1-10, not 1000"
        );
    }

    #[test]
    fn go_duration_valid() {
        let result = GoDuration::new("1s");
        assert_eq!(result.unwrap().as_str(), "1s");
    }

    #[test]
    fn go_duration_zero() {
        let result = GoDuration::new("0s");
        assert_eq!(result.unwrap().as_str(), "0s");
    }

    #[test]
    fn go_duration_display_roundtrip() {
        let input = GoDuration::new("1s").unwrap();
        let display = input.to_string();
        assert!(!display.is_empty());
    }
}

// =========================================================================
// DIMENSION 5: TriggerId stress tests (beyond gen1-3 coverage)
// =========================================================================

mod trigger_id_stress {
    use twerk_core::TriggerId;

    #[test]
    fn trigger_id_exactly_3_chars_accepted() {
        assert_eq!(TriggerId::new("abc").unwrap().as_str(), "abc");
    }

    #[test]
    fn trigger_id_exactly_64_chars_accepted() {
        let s = "a".repeat(64);
        assert_eq!(TriggerId::new(&s).unwrap().as_str(), s);
    }

    #[test]
    fn trigger_id_65_chars_rejected() {
        let s = "a".repeat(65);
        assert_eq!(
            TriggerId::new(&s),
            Err(twerk_core::id::IdError::TooLong(65))
        );
    }

    #[test]
    fn trigger_id_2_chars_rejected() {
        assert_eq!(
            TriggerId::new("ab"),
            Err(twerk_core::id::IdError::TooShort(2))
        );
    }

    #[test]
    fn trigger_id_empty_rejected() {
        assert_eq!(TriggerId::new(""), Err(twerk_core::id::IdError::Empty));
    }

    #[test]
    fn trigger_id_only_dashes() {
        assert_eq!(TriggerId::new("---").unwrap().as_str(), "---");
    }

    #[test]
    fn trigger_id_only_underscores() {
        assert_eq!(TriggerId::new("___").unwrap().as_str(), "___");
    }

    #[test]
    fn trigger_id_mixed_alphanumeric() {
        assert_eq!(
            TriggerId::new("abc-123_XYZ").unwrap().as_str(),
            "abc-123_XYZ"
        );
    }
}

// =========================================================================
// DIMENSION 6: TriggerState serde roundtrips
// =========================================================================

mod trigger_state_serde {
    use super::*;
    use twerk_core::TriggerId;
    use twerk_core::TriggerState;

    #[test]
    fn serde_all_variants_preserve_state() {
        for state in [
            TriggerState::Active,
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerState::Error,
        ] {
            let trigger = make_trigger(state);
            let json = serde_json::to_string(&trigger).unwrap();
            let recovered: Trigger = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered.state, state, "State {:?} failed roundtrip", state);
        }
    }

    #[test]
    fn serde_all_variants_preserve_variant() {
        for variant in [
            TriggerVariant::Cron,
            TriggerVariant::Webhook,
            TriggerVariant::Polling,
        ] {
            let trigger = make_trigger_with_variant(variant);
            let json = serde_json::to_string(&trigger).unwrap();
            let recovered: Trigger = serde_json::from_str(&json).unwrap();
            assert_eq!(
                recovered.variant, variant,
                "Variant {:?} failed roundtrip",
                variant
            );
        }
    }

    fn make_trigger(state: TriggerState) -> Trigger {
        Trigger {
            id: TriggerId::new("serde-test").unwrap(),
            state,
            variant: TriggerVariant::Webhook,
        }
    }

    fn make_trigger_with_variant(variant: TriggerVariant) -> Trigger {
        Trigger {
            id: TriggerId::new("serde-test").unwrap(),
            state: TriggerState::Active,
            variant,
        }
    }
}

// =========================================================================
// DIMENSION 7: GoDuration edge cases
// =========================================================================

mod goduration_adversarial {
    use super::*;

    #[test]
    fn goduration_various_units() {
        let units = ["1s", "1m", "1h", "1d", "1ms", "1us"];
        for u in units {
            let parsed = GoDuration::new(u);
            assert_eq!(parsed.unwrap().as_str(), u, "Should parse: {u}");
        }
    }

    #[test]
    fn goduration_week_not_supported() {
        let result = GoDuration::new("1w");
        assert_eq!(
            result,
            Err(twerk_core::domain::GoDurationError::UnknownUnit('w')),
            "GoDuration only supports ns, us, ms, s, m, h, d - no week"
        );
    }

    #[test]
    fn goduration_zero_all_units() {
        let units = ["0s", "0m", "0h", "0d"];
        for u in units {
            let parsed = GoDuration::new(u);
            assert_eq!(parsed.unwrap().as_str(), u, "Should parse zero: {u}");
        }
    }

    #[test]
    fn goduration_large_values() {
        let large = GoDuration::new("1000h");
        assert_eq!(large.unwrap().as_str(), "1000h");
    }
}
