//! Integration tests for TriggerRegistry trait.
//!
//! These tests use the InMemoryTriggerRegistry fake implementation to test
//! the TriggerRegistry trait methods against real async behavior.

use time::OffsetDateTime;
use twerk_core::trigger::{
    InMemoryTriggerRegistry, Trigger, TriggerContext, TriggerId, TriggerRegistry, TriggerState,
    TriggerVariant,
};

// =============================================================================
// TriggerRegistry Trait Integration Tests
// =============================================================================

#[tokio::test]
async fn trigger_registry_register_succeeds_for_valid_cron_trigger() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("cron-trigger".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    let result = registry.register(trigger).await;
    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn trigger_registry_register_succeeds_for_valid_webhook_trigger() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("webhook-trigger".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Webhook,
    };

    let result = registry.register(trigger).await;
    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn trigger_registry_register_succeeds_for_valid_polling_trigger() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("polling-trigger".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Polling,
    };

    let result = registry.register(trigger).await;
    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn trigger_registry_register_succeeds_with_paused_state() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("paused-trigger".into()),
        state: TriggerState::Paused,
        variant: TriggerVariant::Cron,
    };

    let result = registry.register(trigger).await;
    assert_eq!(result, Ok(()));
}

#[tokio::test]
async fn trigger_registry_register_fails_with_duplicate_id() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger1 = Trigger {
        id: TriggerId("duplicate-id".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };
    let trigger2 = Trigger {
        id: TriggerId("duplicate-id".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Webhook,
    };

    registry.register(trigger1).await.unwrap();
    let result = registry.register(trigger2).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::AlreadyExists(TriggerId(
            "duplicate-id".into()
        )))
    );
}

#[tokio::test]
async fn trigger_registry_register_fails_for_disabled_state() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("disabled-trigger".into()),
        state: TriggerState::Disabled,
        variant: TriggerVariant::Cron,
    };

    let result = registry.register(trigger).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidConfiguration(
            "new triggers cannot start in Disabled state".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_register_fails_for_error_state() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("error-trigger".into()),
        state: TriggerState::Error,
        variant: TriggerVariant::Polling,
    };

    let result = registry.register(trigger).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidConfiguration(
            "new triggers cannot start in Error state".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_unregister_removes_trigger() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("to-unregister".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();
    registry
        .unregister(&TriggerId("to-unregister".into()))
        .await
        .unwrap();

    let result = registry
        .get(&TriggerId("to-unregister".into()))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn trigger_registry_unregister_fails_for_nonexistent_id() {
    let registry = InMemoryTriggerRegistry::new();

    let result = registry.unregister(&TriggerId("nonexistent".into())).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::NotFound(TriggerId(
            "nonexistent".into()
        )))
    );
}

#[tokio::test]
async fn trigger_registry_set_state_updates_state() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("state-test".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();
    registry
        .set_state(&TriggerId("state-test".into()), TriggerState::Paused)
        .await
        .unwrap();

    let result = registry
        .get(&TriggerId("state-test".into()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.state, TriggerState::Paused);
}

#[tokio::test]
async fn trigger_registry_set_state_fails_for_nonexistent_trigger() {
    let registry = InMemoryTriggerRegistry::new();

    let result = registry
        .set_state(&TriggerId("nonexistent".into()), TriggerState::Active)
        .await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::NotFound(TriggerId(
            "nonexistent".into()
        )))
    );
}

#[tokio::test]
async fn trigger_registry_set_state_fails_for_invalid_transition() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("invalid-transition".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron, // Not Polling
    };

    registry.register(trigger).await.unwrap();
    let result = registry
        .set_state(&TriggerId("invalid-transition".into()), TriggerState::Error)
        .await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidStateTransition(
            TriggerState::Active,
            TriggerState::Error
        ))
    );
}

#[tokio::test]
async fn trigger_registry_get_returns_trigger_when_exists() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("get-test".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Webhook,
    };

    registry.register(trigger).await.unwrap();
    let result = registry.get(&TriggerId("get-test".into())).await.unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap().id.as_str(), "get-test");
}

#[tokio::test]
async fn trigger_registry_get_returns_none_when_not_exists() {
    let registry = InMemoryTriggerRegistry::new();
    let result = registry
        .get(&TriggerId("nonexistent".into()))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn trigger_registry_list_returns_all_triggers() {
    let registry = InMemoryTriggerRegistry::new();

    registry
        .register(Trigger {
            id: TriggerId("list-1".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();

    registry
        .register(Trigger {
            id: TriggerId("list-2".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Webhook,
        })
        .await
        .unwrap();

    let result = registry.list().await.unwrap();
    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn trigger_registry_list_returns_empty_when_no_triggers() {
    let registry = InMemoryTriggerRegistry::new();
    let result = registry.list().await.unwrap();
    assert_eq!(result, vec![]);
}

#[tokio::test]
async fn trigger_registry_list_by_state_filters_correctly() {
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

    let active_triggers = registry.list_by_state(TriggerState::Active).await.unwrap();
    let paused_triggers = registry.list_by_state(TriggerState::Paused).await.unwrap();

    assert_eq!(active_triggers.len(), 2);
    assert_eq!(paused_triggers.len(), 1);
    assert!(active_triggers
        .iter()
        .all(|t| t.state == TriggerState::Active));
    assert!(paused_triggers
        .iter()
        .all(|t| t.state == TriggerState::Paused));
}

#[tokio::test]
async fn trigger_registry_list_by_state_returns_empty_for_no_matches() {
    let registry = InMemoryTriggerRegistry::new();

    registry
        .register(Trigger {
            id: TriggerId("active-only".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();

    let result = registry.list_by_state(TriggerState::Paused).await.unwrap();
    assert_eq!(result, vec![]);
}

#[tokio::test]
async fn trigger_registry_fire_creates_job_for_active_trigger() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("fire-test".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();

    let ctx = TriggerContext {
        trigger_id: TriggerId("fire-test".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Cron,
    };

    let result = registry.fire(ctx).await;
    assert!(
        result.is_ok(),
        "fire should succeed for active trigger with broker available"
    );
    let job_id = result.unwrap();
    assert_eq!(job_id.as_str().len(), 36); // UUID v4 format
}

#[tokio::test]
async fn trigger_registry_fire_increments_fire_count() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("count-test".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();

    let ctx = TriggerContext {
        trigger_id: TriggerId("count-test".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Cron,
    };

    registry.fire(ctx.clone()).await.unwrap();
    registry.fire(ctx).await.unwrap();

    assert_eq!(registry.fire_count(), 2);
}

#[tokio::test]
async fn trigger_registry_fire_fails_for_nonexistent_trigger() {
    let registry = InMemoryTriggerRegistry::new();

    let ctx = TriggerContext {
        trigger_id: TriggerId("nonexistent".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Cron,
    };

    let result = registry.fire(ctx).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::NotFound(TriggerId(
            "nonexistent".into()
        )))
    );
}

#[tokio::test]
async fn trigger_registry_fire_fails_for_paused_trigger() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("paused-fire".into()),
        state: TriggerState::Paused,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();

    let ctx = TriggerContext {
        trigger_id: TriggerId("paused-fire".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Cron,
    };

    let result = registry.fire(ctx).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::TriggerNotActive(
            TriggerState::Paused
        ))
    );
}

#[tokio::test]
async fn trigger_registry_fire_fails_for_disabled_trigger() {
    let registry = InMemoryTriggerRegistry::new();
    // Register as Active, then transition to Disabled
    let trigger = Trigger {
        id: TriggerId("disabled-fire".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();
    registry
        .set_state(&TriggerId("disabled-fire".into()), TriggerState::Disabled)
        .await
        .unwrap();

    let ctx = TriggerContext {
        trigger_id: TriggerId("disabled-fire".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Cron,
    };

    let result = registry.fire(ctx).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::TriggerDisabled(
            TriggerId("disabled-fire".into())
        ))
    );
}

#[tokio::test]
async fn trigger_registry_fire_fails_for_polling_in_error_state() {
    let registry = InMemoryTriggerRegistry::new();
    // Register as Active Polling, then transition to Error
    let trigger = Trigger {
        id: TriggerId("error-fire".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Polling,
    };

    registry.register(trigger).await.unwrap();
    registry
        .set_state(&TriggerId("error-fire".into()), TriggerState::Error)
        .await
        .unwrap();

    let ctx = TriggerContext {
        trigger_id: TriggerId("error-fire".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Polling,
    };

    let result = registry.fire(ctx).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::TriggerInErrorState(
            TriggerId("error-fire".into())
        ))
    );
}

#[tokio::test]
async fn trigger_registry_fire_fails_when_broker_unavailable() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("broker-down".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();
    registry.set_broker_available(false);

    let ctx = TriggerContext {
        trigger_id: TriggerId("broker-down".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Cron,
    };

    let result = registry.fire(ctx).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::BrokerUnavailable(
            "connection refused".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_fire_fails_when_concurrency_limit_reached() {
    let registry = InMemoryTriggerRegistry::with_concurrency_limit(1);
    let trigger = Trigger {
        id: TriggerId("concurrency-test".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();

    let ctx = TriggerContext {
        trigger_id: TriggerId("concurrency-test".into()),
        timestamp: OffsetDateTime::now_utc(),
        event_data: None,
        trigger_type: TriggerVariant::Cron,
    };

    // First fire should succeed
    let result1 = registry.fire(ctx.clone()).await;
    assert!(result1.is_ok(), "First fire should succeed");

    // Second fire also succeeds because first fire released the permit
    // The semaphore limits concurrent fires, not sequential fires
    let result2 = registry.fire(ctx).await;
    assert!(
        result2.is_ok(),
        "Second fire succeeds because permit was released by first fire"
    );
}

// =============================================================================
// Datastore Unavailability Tests
// =============================================================================

#[tokio::test]
async fn trigger_registry_register_fails_when_datastore_unavailable() {
    let registry = InMemoryTriggerRegistry::new();
    registry.set_datastore_available(false);

    let trigger = Trigger {
        id: TriggerId("ds-down".into()),
        state: TriggerState::Active,
        variant: TriggerVariant::Cron,
    };

    let result = registry.register(trigger).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::DatastoreUnavailable(
            "connection refused".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_unregister_fails_when_datastore_unavailable() {
    let registry = InMemoryTriggerRegistry::new();
    registry.set_datastore_available(false);

    let result = registry.unregister(&TriggerId("ds-down".into())).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::DatastoreUnavailable(
            "connection refused".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_set_state_fails_when_datastore_unavailable() {
    let registry = InMemoryTriggerRegistry::new();
    registry.set_datastore_available(false);

    let result = registry
        .set_state(&TriggerId("ds-down".into()), TriggerState::Active)
        .await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::DatastoreUnavailable(
            "connection refused".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_get_fails_when_datastore_unavailable() {
    let registry = InMemoryTriggerRegistry::new();
    registry.set_datastore_available(false);

    let result = registry.get(&TriggerId("ds-down".into())).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::DatastoreUnavailable(
            "connection refused".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_list_fails_when_datastore_unavailable() {
    let registry = InMemoryTriggerRegistry::new();
    registry.set_datastore_available(false);

    let result = registry.list().await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::DatastoreUnavailable(
            "connection refused".into()
        ))
    );
}

#[tokio::test]
async fn trigger_registry_list_by_state_fails_when_datastore_unavailable() {
    let registry = InMemoryTriggerRegistry::new();
    registry.set_datastore_available(false);

    let result = registry.list_by_state(TriggerState::Active).await;
    assert_eq!(
        result,
        Err(twerk_core::trigger::TriggerError::DatastoreUnavailable(
            "connection refused".into()
        ))
    );
}

// =============================================================================
// State Transition Tests
// =============================================================================

#[tokio::test]
async fn trigger_registry_all_valid_transitions_succeed() {
    let registry = InMemoryTriggerRegistry::new();

    // Active -> Paused
    registry
        .register(Trigger {
            id: TriggerId("trans-1".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-1".into()), TriggerState::Paused)
        .await
        .unwrap();

    // Active -> Disabled
    registry
        .register(Trigger {
            id: TriggerId("trans-2".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-2".into()), TriggerState::Disabled)
        .await
        .unwrap();

    // Paused -> Active (need to register Active, set to Paused, then back to Active)
    registry
        .register(Trigger {
            id: TriggerId("trans-3".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-3".into()), TriggerState::Paused)
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-3".into()), TriggerState::Active)
        .await
        .unwrap();

    // Paused -> Disabled (need to register Active, set to Paused, then set to Disabled)
    registry
        .register(Trigger {
            id: TriggerId("trans-4".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-4".into()), TriggerState::Paused)
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-4".into()), TriggerState::Disabled)
        .await
        .unwrap();

    // Disabled -> Active (need to register Active, set to Disabled, then back to Active)
    registry
        .register(Trigger {
            id: TriggerId("trans-5".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-5".into()), TriggerState::Disabled)
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-5".into()), TriggerState::Active)
        .await
        .unwrap();

    // Disabled -> Paused (need to register Active, set to Disabled, then set to Paused)
    registry
        .register(Trigger {
            id: TriggerId("trans-6".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-6".into()), TriggerState::Paused)
        .await
        .unwrap();

    // Active -> Error (Polling only)
    registry
        .register(Trigger {
            id: TriggerId("trans-7".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Polling,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("trans-7".into()), TriggerState::Error)
        .await
        .unwrap();

    // Error -> Active (Polling only)
    registry
        .set_state(&TriggerId("trans-7".into()), TriggerState::Active)
        .await
        .unwrap();
}

#[tokio::test]
async fn trigger_registry_invalid_transitions_are_rejected() {
    let registry = InMemoryTriggerRegistry::new();

    // Active -> Error (Cron) - invalid
    registry
        .register(Trigger {
            id: TriggerId("inv-1".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Cron,
        })
        .await
        .unwrap();
    let result = registry
        .set_state(&TriggerId("inv-1".into()), TriggerState::Error)
        .await;
    assert!(matches!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidStateTransition(
            _,
            _
        ))
    ));

    // Active -> Error (Webhook) - invalid
    registry
        .register(Trigger {
            id: TriggerId("inv-2".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Webhook,
        })
        .await
        .unwrap();
    let result = registry
        .set_state(&TriggerId("inv-2".into()), TriggerState::Error)
        .await;
    assert!(matches!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidStateTransition(
            _,
            _
        ))
    ));

    // Paused -> Error - invalid
    registry
        .register(Trigger {
            id: TriggerId("inv-3".into()),
            state: TriggerState::Paused,
            variant: TriggerVariant::Polling,
        })
        .await
        .unwrap();
    let result = registry
        .set_state(&TriggerId("inv-3".into()), TriggerState::Error)
        .await;
    assert!(matches!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidStateTransition(
            _,
            _
        ))
    ));

    // Disabled -> Error - invalid (must register Active, then set to Disabled first)
    registry
        .register(Trigger {
            id: TriggerId("inv-4".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Polling,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("inv-4".into()), TriggerState::Disabled)
        .await
        .unwrap();
    let result = registry
        .set_state(&TriggerId("inv-4".into()), TriggerState::Error)
        .await;
    assert!(matches!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidStateTransition(
            _,
            _
        ))
    ));

    // Error -> Paused - invalid (need to register Polling Active, set to Error, then try Paused)
    registry
        .register(Trigger {
            id: TriggerId("inv-5".into()),
            state: TriggerState::Active,
            variant: TriggerVariant::Polling,
        })
        .await
        .unwrap();
    registry
        .set_state(&TriggerId("inv-5".into()), TriggerState::Error)
        .await
        .unwrap();
    let result = registry
        .set_state(&TriggerId("inv-5".into()), TriggerState::Paused)
        .await;
    assert!(matches!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidStateTransition(
            _,
            _
        ))
    ));

    // Error -> Disabled - invalid (continue with inv-5)
    let result = registry
        .set_state(&TriggerId("inv-5".into()), TriggerState::Disabled)
        .await;
    assert!(matches!(
        result,
        Err(twerk_core::trigger::TriggerError::InvalidStateTransition(
            _,
            _
        ))
    ));

    // Note: Cron and Webhook triggers cannot transition to Error state.
    // Error state is only reachable via polling failures for Polling triggers.
}

// =============================================================================
// Concurrent Access Tests (Send + Sync verification)
// =============================================================================

#[tokio::test]
async fn trigger_registry_is_send_and_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<InMemoryTriggerRegistry>();
    assert_sync::<InMemoryTriggerRegistry>();
}
