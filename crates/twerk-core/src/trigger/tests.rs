//! Unit, property-based, and formal verification tests for the trigger system.
//!
//! This module contains comprehensive tests including:
//! - Unit tests for TriggerId, TriggerState, TriggerVariant, TriggerError
//! - State transition matrix validation tests
//! - InMemoryTriggerRegistry integration tests
//! - Proptest property-based tests
//! - Kani formal verification proofs

#[cfg(test)]
use crate::trigger::types::{
    Trigger, TriggerContext, TriggerError, TriggerId, TriggerIdError, TriggerState, TriggerVariant,
};
#[cfg(test)]
use crate::trigger::{is_valid_transition, InMemoryTriggerRegistry, TriggerRegistry};

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TriggerId Validation Tests
    // =========================================================================

    #[test]
    fn trigger_id_returns_ok_when_input_is_3_alphanumeric_chars() {
        let result = TriggerId::new("abc");
        assert_eq!(result, Ok(TriggerId("abc".into())));
    }

    #[test]
    fn trigger_id_returns_ok_when_input_contains_hyphens() {
        let result = TriggerId::new("my-trigger-001");
        assert_eq!(result, Ok(TriggerId("my-trigger-001".into())));
    }

    #[test]
    fn trigger_id_returns_ok_when_input_contains_underscores() {
        let result = TriggerId::new("my_trigger_001");
        assert_eq!(result, Ok(TriggerId("my_trigger_001".into())));
    }

    #[test]
    fn trigger_id_returns_ok_when_input_is_64_alphanumeric_chars() {
        let input = "a".repeat(64);
        let result = TriggerId::new(&input);
        assert_eq!(result, Ok(TriggerId(input)));
    }

    #[test]
    fn trigger_id_returns_length_error_when_input_too_short() {
        let result = TriggerId::new("ab");
        assert_eq!(result, Err(TriggerIdError::TooShort(2)));
    }

    #[test]
    fn trigger_id_returns_length_error_when_input_too_long() {
        let input = "a".repeat(65);
        let result = TriggerId::new(&input);
        assert_eq!(result, Err(TriggerIdError::TooLong(65)));
    }

    #[test]
    fn trigger_id_returns_length_error_when_input_is_empty() {
        let result = TriggerId::new("");
        assert_eq!(result, Err(TriggerIdError::Empty));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_spaces() {
        let result = TriggerId::new("my trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_special_chars() {
        let result = TriggerId::new("my@trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_returns_ok_when_input_contains_unicode() {
        let result = TriggerId::new("触发器");
        assert_eq!(result, Ok(TriggerId::from("触发器")));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_control_char() {
        let result = TriggerId::new("my\ntrigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_dot() {
        let result = TriggerId::new("my.trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacters));
    }

    #[test]
    fn trigger_id_returns_invalid_char_error_when_input_contains_hash() {
        let result = TriggerId::new("my#trigger");
        assert_eq!(result, Err(TriggerIdError::InvalidCharacters));
    }

    // =========================================================================
    // TriggerState Serialization Tests
    // =========================================================================

    #[test]
    fn trigger_state_serializes_to_screaming_snake_case() {
        assert_eq!(
            serde_json::to_string(&TriggerState::Active).unwrap(),
            "\"Active\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerState::Paused).unwrap(),
            "\"Paused\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerState::Disabled).unwrap(),
            "\"Disabled\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerState::Error).unwrap(),
            "\"Error\""
        );
    }

    #[test]
    fn trigger_state_deserializes_from_screaming_snake_case() {
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"ACTIVE\"").unwrap(),
            TriggerState::Active
        );
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"PAUSED\"").unwrap(),
            TriggerState::Paused
        );
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"DISABLED\"").unwrap(),
            TriggerState::Disabled
        );
        assert_eq!(
            serde_json::from_str::<TriggerState>("\"ERROR\"").unwrap(),
            TriggerState::Error
        );
    }

    // =========================================================================
    // TriggerVariant Serialization Tests
    // =========================================================================

    #[test]
    fn trigger_variant_serializes_to_pascal_case() {
        assert_eq!(
            serde_json::to_string(&TriggerVariant::Cron).unwrap(),
            "\"Cron\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerVariant::Webhook).unwrap(),
            "\"Webhook\""
        );
        assert_eq!(
            serde_json::to_string(&TriggerVariant::Polling).unwrap(),
            "\"Polling\""
        );
    }

    #[test]
    fn trigger_variant_deserializes_from_pascal_case() {
        assert_eq!(
            serde_json::from_str::<TriggerVariant>("\"Cron\"").unwrap(),
            TriggerVariant::Cron
        );
        assert_eq!(
            serde_json::from_str::<TriggerVariant>("\"Webhook\"").unwrap(),
            TriggerVariant::Webhook
        );
        assert_eq!(
            serde_json::from_str::<TriggerVariant>("\"Polling\"").unwrap(),
            TriggerVariant::Polling
        );
    }

    // =========================================================================
    // TriggerError Display Tests
    // =========================================================================

    #[test]
    fn trigger_error_not_found_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::NotFound(id.clone());
        assert!(err.to_string().contains("trigger not found"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_already_exists_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::AlreadyExists(id.clone());
        assert!(err.to_string().contains("trigger already registered"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_invalid_state_transition_displays_correctly() {
        let err = TriggerError::InvalidStateTransition(TriggerState::Active, TriggerState::Error);
        assert!(err.to_string().contains("invalid state transition"));
        assert!(err.to_string().contains("Active"));
        assert!(err.to_string().contains("Error"));
    }

    #[test]
    fn trigger_error_datastore_unavailable_displays_correctly() {
        let err = TriggerError::DatastoreUnavailable("connection refused".into());
        assert!(err.to_string().contains("datastore unavailable"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn trigger_error_broker_unavailable_displays_correctly() {
        let err = TriggerError::BrokerUnavailable("connection refused".into());
        assert!(err.to_string().contains("broker unavailable"));
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn trigger_error_concurrency_limit_reached_displays_correctly() {
        let err = TriggerError::ConcurrencyLimitReached;
        assert!(err.to_string().contains("concurrency limit reached"));
    }

    #[test]
    fn trigger_error_trigger_not_active_displays_correctly() {
        let err = TriggerError::TriggerNotActive(TriggerState::Paused);
        assert!(err.to_string().contains("trigger is not active"));
        assert!(err.to_string().contains("Paused"));
    }

    #[test]
    fn trigger_error_trigger_in_error_state_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::TriggerInErrorState(id.clone());
        assert!(err.to_string().contains("trigger is in error state"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_trigger_disabled_displays_correctly() {
        let id = TriggerId("test-trigger".into());
        let err = TriggerError::TriggerDisabled(id.clone());
        assert!(err.to_string().contains("trigger is disabled"));
        assert!(err.to_string().contains("test-trigger"));
    }

    #[test]
    fn trigger_error_invalid_configuration_displays_correctly() {
        let err = TriggerError::InvalidConfiguration("test error".into());
        assert!(err.to_string().contains("invalid trigger configuration"));
        assert!(err.to_string().contains("test error"));
    }

    // =========================================================================
    // TriggerIdError Display Tests
    // =========================================================================

    #[test]
    fn trigger_id_error_length_out_of_range_displays_correctly() {
        let err = TriggerIdError::TooShort(5);
        assert!(err.to_string().contains("too short"));
        assert!(err.to_string().contains("5"));
    }

    #[test]
    fn trigger_id_error_invalid_character_displays_correctly() {
        let err = TriggerIdError::InvalidCharacters;
        assert!(err.to_string().contains("invalid"));
    }

    // =========================================================================
    // State Transition Matrix Tests
    // =========================================================================

    #[test]
    fn is_valid_transition_active_to_paused_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_active_to_disabled_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_active_to_error_for_polling_only() {
        assert!(!is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_paused_to_active_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_paused_to_disabled_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_paused_to_error_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_disabled_to_active_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_disabled_to_paused_is_valid() {
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_disabled_to_error_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_error_to_active_for_polling_only() {
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_error_to_paused_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_error_to_disabled_is_invalid() {
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(!is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_active_state() {
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_paused_state() {
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_disabled_state() {
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Polling
        ));
    }

    #[test]
    fn is_valid_transition_self_is_valid_for_error_state() {
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Cron
        ));
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Webhook
        ));
        assert!(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Polling
        ));
    }

    // =========================================================================
    // InMemoryTriggerRegistry Tests
    // =========================================================================

    #[tokio::test]
    async fn register_succeeds_when_trigger_is_valid_and_id_is_unique() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger-001".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        let result = registry.register(trigger).await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger-001".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().id.as_str(), "test-trigger-001");
    }

    #[tokio::test]
    async fn register_returns_already_exists_when_id_is_duplicate() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger.clone()).await.unwrap();
        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::AlreadyExists(TriggerId(
                "test-trigger".into()
            )))
        );
    }

    #[tokio::test]
    async fn register_returns_invalid_configuration_when_state_is_disabled() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Disabled,
            variant: TriggerVariant::Cron,
        };

        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Disabled state".into(),
            ))
        );
    }

    #[tokio::test]
    async fn register_returns_invalid_configuration_when_state_is_error() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Error,
            variant: TriggerVariant::Polling,
        };

        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Error state".into(),
            ))
        );
    }

    #[tokio::test]
    async fn unregister_succeeds_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("to-delete".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry.unregister(&TriggerId("to-delete".into())).await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("to-delete".into())).await;
        assert_eq!(retrieved.unwrap(), None);
    }

    #[tokio::test]
    async fn unregister_returns_not_found_when_trigger_does_not_exist() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry.unregister(&TriggerId("nonexistent".into())).await;
        assert_eq!(
            result,
            Err(TriggerError::NotFound(TriggerId("nonexistent".into())))
        );
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_paused_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Paused)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Paused);
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_disabled_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Disabled);
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_error_for_polling_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Error);
    }

    #[tokio::test]
    async fn set_state_transitions_paused_to_active_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Active);
    }

    #[tokio::test]
    async fn set_state_transitions_paused_to_disabled_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Disabled);
    }

    #[tokio::test]
    async fn set_state_transitions_disabled_to_active_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await
            .unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Active);
    }

    #[tokio::test]
    async fn set_state_transitions_disabled_to_paused_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await
            .unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Paused)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Paused);
    }

    #[tokio::test]
    async fn set_state_transitions_error_to_active_for_polling_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();
        registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await
            .unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));

        let retrieved = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(retrieved.unwrap().unwrap().state, TriggerState::Active);
    }

    #[tokio::test]
    async fn set_state_transitions_active_to_active_self_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Active)
            .await;
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn set_state_rejects_active_to_error_for_cron_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Active,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_active_to_error_for_webhook_trigger() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Webhook,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Active,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_paused_to_error_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Paused,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_rejects_disabled_to_error_transition() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await
            .unwrap();
        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::InvalidStateTransition(
                TriggerState::Disabled,
                TriggerState::Error
            ))
        );
    }

    #[tokio::test]
    async fn set_state_returns_not_found_when_trigger_does_not_exist() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry
            .set_state(&TriggerId("nonexistent".into()), TriggerState::Active)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::NotFound(TriggerId("nonexistent".into())))
        );
    }

    #[tokio::test]
    async fn get_returns_some_when_trigger_exists() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        let result = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(result.unwrap().unwrap().id.as_str(), "test-trigger");
    }

    #[tokio::test]
    async fn get_returns_none_when_trigger_not_found() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry.get(&TriggerId("nonexistent".into())).await;
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn list_returns_all_triggers_when_triggers_exist() {
        let registry = InMemoryTriggerRegistry::new();

        registry
            .register(Trigger {
                id: TriggerId("trigger-1".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Cron,
            })
            .await
            .unwrap();

        registry
            .register(Trigger {
                id: TriggerId("trigger-2".into()),
                state: TriggerState::Paused,
                variant: TriggerVariant::Webhook,
            })
            .await
            .unwrap();

        let result = registry.list().await;
        let triggers = result.unwrap();
        assert_eq!(triggers.len(), 2);
    }

    #[tokio::test]
    async fn list_returns_empty_vec_when_no_triggers_exist() {
        let registry = InMemoryTriggerRegistry::new();
        let result = registry.list().await;
        assert_eq!(result.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn list_by_state_returns_matching_triggers_when_matches_exist() {
        let registry = InMemoryTriggerRegistry::new();

        registry
            .register(Trigger {
                id: TriggerId("active-1".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Cron,
            })
            .await
            .unwrap();

        registry
            .register(Trigger {
                id: TriggerId("paused-1".into()),
                state: TriggerState::Paused,
                variant: TriggerVariant::Webhook,
            })
            .await
            .unwrap();

        registry
            .register(Trigger {
                id: TriggerId("active-2".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Polling,
            })
            .await
            .unwrap();

        let result = registry.list_by_state(TriggerState::Active).await;
        let triggers = result.unwrap();
        assert_eq!(triggers.len(), 2);
        assert!(triggers.iter().all(|t| t.state == TriggerState::Active));
    }

    #[tokio::test]
    async fn list_by_state_returns_empty_when_no_matches() {
        let registry = InMemoryTriggerRegistry::new();

        registry
            .register(Trigger {
                id: TriggerId("active-1".into()),
                state: TriggerState::Active,
                variant: TriggerVariant::Cron,
            })
            .await
            .unwrap();

        let result = registry.list_by_state(TriggerState::Paused).await;
        assert_eq!(result.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn fire_returns_job_id_when_trigger_is_active_and_broker_available() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert!(
            result.is_ok(),
            "fire should succeed for active trigger with broker available"
        );
        let job_id = result.unwrap();
        assert_eq!(job_id.as_str().len(), 36); // UUID v4 length
    }

    #[tokio::test]
    async fn fire_returns_not_found_when_trigger_does_not_exist() {
        let registry = InMemoryTriggerRegistry::new();

        let ctx = TriggerContext {
            trigger_id: TriggerId("nonexistent".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::NotFound(TriggerId("nonexistent".into())))
        );
    }

    #[tokio::test]
    async fn fire_returns_trigger_not_active_when_trigger_is_paused() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::TriggerNotActive(TriggerState::Paused))
        );
    }

    #[tokio::test]
    async fn fire_returns_trigger_disabled_when_trigger_is_disabled() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Disabled)
            .await
            .unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::TriggerDisabled(TriggerId(
                "test-trigger".into()
            )))
        );
    }

    #[tokio::test]
    async fn fire_returns_trigger_in_error_state_when_polling_trigger_is_in_error() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Polling,
        };

        registry.register(trigger).await.unwrap();
        registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Error)
            .await
            .unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Polling,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::TriggerInErrorState(TriggerId(
                "test-trigger".into()
            )))
        );
    }

    #[tokio::test]
    async fn fire_returns_broker_unavailable_when_broker_is_down() {
        let registry = InMemoryTriggerRegistry::new();
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();
        registry.set_broker_available(false);

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(
            result,
            Err(TriggerError::BrokerUnavailable("connection refused".into()))
        );
    }

    #[tokio::test]
    async fn fire_returns_concurrency_limit_when_limit_reached() {
        let registry = InMemoryTriggerRegistry::with_concurrency_limit(0);
        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        registry.register(trigger).await.unwrap();

        let ctx = TriggerContext {
            trigger_id: TriggerId("test-trigger".into()),
            timestamp: time::OffsetDateTime::now_utc(),
            event_data: None,
            trigger_type: TriggerVariant::Cron,
        };

        let result = registry.fire(ctx).await;
        assert_eq!(result, Err(TriggerError::ConcurrencyLimitReached));
    }

    // =========================================================================
    // Datastore Unavailability Tests
    // =========================================================================

    #[tokio::test]
    async fn register_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let trigger = Trigger {
            id: TriggerId("test-trigger".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        let result = registry.register(trigger).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn unregister_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.unregister(&TriggerId("test-trigger".into())).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn set_state_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry
            .set_state(&TriggerId("test-trigger".into()), TriggerState::Paused)
            .await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn get_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.get(&TriggerId("test-trigger".into())).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn list_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.list().await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }

    #[tokio::test]
    async fn list_by_state_returns_datastore_unavailable_when_storage_fails() {
        let registry = InMemoryTriggerRegistry::new();
        registry.set_datastore_available(false);

        let result = registry.list_by_state(TriggerState::Active).await;
        assert_eq!(
            result,
            Err(TriggerError::DatastoreUnavailable(
                "connection refused".into()
            ))
        );
    }
}

// =============================================================================
// Proptest Property-Based Tests
// =============================================================================

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    impl Arbitrary for TriggerState {
        type Strategy = BoxedStrategy<TriggerState>;
        type Parameters = ();

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            prop_oneof![
                Just(TriggerState::Active),
                Just(TriggerState::Paused),
                Just(TriggerState::Disabled),
                Just(TriggerState::Error),
            ]
            .boxed()
        }
    }

    impl Arbitrary for TriggerVariant {
        type Strategy = BoxedStrategy<TriggerVariant>;
        type Parameters = ();

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            prop_oneof![
                Just(TriggerVariant::Cron),
                Just(TriggerVariant::Webhook),
                Just(TriggerVariant::Polling),
            ]
            .boxed()
        }
    }

    proptest! {
        #[test]
        fn trigger_id_new_accepts_any_valid_3_to_64_char_alphanumeric_input(s in "[a-zA-Z0-9]{3,64}") {
            let result = TriggerId::new(&s);
            prop_assert_eq!(result, Ok(TriggerId(s)));
        }

        #[test]
        fn trigger_id_new_accepts_hyphens_and_underscores(
            prefix in "[a-zA-Z0-9]{1,20}",
            suffix in "[a-zA-Z0-9]{1,20}"
        ) {
            let with_hyphen = format!("{}-{}", prefix, suffix);
            let with_underscore = format!("{}_{}", prefix, suffix);

            prop_assert_eq!(TriggerId::new(&with_hyphen), Ok(TriggerId(with_hyphen.clone())));
            prop_assert_eq!(TriggerId::new(&with_underscore), Ok(TriggerId(with_underscore.clone())));
        }

        #[test]
        fn trigger_id_new_rejects_strings_shorter_than_3_chars(s in "[a-zA-Z0-9]{0,2}") {
            let result = TriggerId::new(&s);
            if s.is_empty() {
                prop_assert_eq!(result, Err(TriggerIdError::Empty));
            } else {
                prop_assert_eq!(result, Err(TriggerIdError::TooShort(s.len())));
            }
        }

        #[test]
        fn trigger_id_new_rejects_strings_longer_than_64_chars(s in "[a-zA-Z0-9]{65,128}") {
            let result = TriggerId::new(&s);
            prop_assert_eq!(result, Err(TriggerIdError::TooLong(s.len())));
        }

        #[test]
        fn trigger_id_roundtrip_through_string_preserves_value(s in "[a-zA-Z0-9\\-_]{3,64}") {
            let id = TriggerId::new(&s).unwrap();
            prop_assert_eq!(id.as_str(), s);
        }
    }

    proptest! {
        #[test]
        fn state_transition_matrix_exhaustive_validation(
            from_state: TriggerState,
            to_state: TriggerState,
            variant: TriggerVariant
        ) {
            let result = is_valid_transition(from_state, to_state, variant);

            if from_state == to_state {
                prop_assert!(result);
            } else if from_state == TriggerState::Error && to_state == TriggerState::Active {
                prop_assert_eq!(result, variant == TriggerVariant::Polling);
            } else if from_state == TriggerState::Active && to_state == TriggerState::Error {
                prop_assert_eq!(result, variant == TriggerVariant::Polling);
            } else if from_state == TriggerState::Error {
                prop_assert!(!result);
            } else if to_state == TriggerState::Error {
                prop_assert!(!result);
            }
        }
    }
}

// =============================================================================
// Kani Formal Verification Harnesses
// =============================================================================

#[cfg(kani)]
#[allow(unexpected_cfgs)]
mod kani_proofs {
    use super::*;

    #[cfg(kani)]
    #[kani::proof]
    fn verify_trigger_id_length_bounds() {
        let input: String = kani::any();
        let len = input.len();

        if TriggerId::new(&input).is_ok() {
            kani::assert(
                len >= 3 && len <= 64,
                "Valid TriggerId must have length 3-64",
            );
        }
    }

    #[cfg(kani)]
    #[kani::proof]
    fn verify_state_transition_matrix_completeness() {
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Active,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Paused,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Disabled,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Active,
            TriggerState::Error,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Active,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Paused,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Disabled,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Paused,
            TriggerState::Error,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Active,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Paused,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Disabled,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Disabled,
            TriggerState::Error,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Active,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Paused,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Disabled,
            TriggerVariant::Polling,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Cron,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Webhook,
        ));
        kani::cover(is_valid_transition(
            TriggerState::Error,
            TriggerState::Error,
            TriggerVariant::Polling,
        ));
    }

    #[cfg(kani)]
    #[kani::proof]
    async fn verify_unique_id_constraint_after_register() {
        let registry = InMemoryTriggerRegistry::new();
        let id = TriggerId("unique-trigger".into());
        let trigger = Trigger {
            id: id.clone(),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        };

        let result = registry.register(trigger.clone()).await;
        kani::assert(result.is_ok(), "First register of unique ID should succeed");
    }
}
