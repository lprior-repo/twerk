# Red Queen Report: TriggerRegistry Adversarial Testing

**Task:** drq-trigger-registry  
**Spec:** `crates/twerk-core/src/trigger.rs`  
**Date:** 2026-04-13  
**Agent:** red-queen  

---

## Executive Summary

The Red Queen ran adversarial testing against the `TriggerRegistry` implementation in `crates/twerk-core/src/trigger.rs`. 

**FINDING: The implementation is correct. The tests are buggy.**

9 test failures were discovered because the tests themselves violate the contract preconditions for `register()`.

---

## Contract Analysis

### Register Preconditions (from trait documentation)

```rust
/// Register a new trigger.
///
/// # Preconditions
/// - `trigger.id` MUST be a valid `TriggerId`
/// - `trigger.state` MUST be `Active` or `Paused`   <-- KEY PRECONDITION
/// - No trigger with the same `trigger.id` may already exist
```

### Implementation (lines 394-426)

```rust
async fn register(&self, trigger: Trigger) -> TriggerRegistryResult<()> {
    // ...
    // Check precondition: state must be Active or Paused
    match trigger.state {
        TriggerState::Active | TriggerState::Paused => {}
        TriggerState::Disabled => {
            return Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Disabled state".into(),
            ));
        }
        TriggerState::Error => {
            return Err(TriggerError::InvalidConfiguration(
                "new triggers cannot start in Error state".into(),
            ));
        }
    }
    // ...
}
```

The implementation CORRECTLY rejects registration with `Disabled` or `Error` state.

---

## Test Bugs Found

### Pattern: Tests violate `register` preconditions

All 9 failing tests attempt to register a trigger with `Disabled` or `Error` state, then use `.unwrap()` to assert success:

```rust
#[tokio::test]
async fn set_state_transitions_disabled_to_active_when_trigger_exists() {
    let registry = InMemoryTriggerRegistry::new();
    let trigger = Trigger {
        id: TriggerId("test-trigger".into()),
        state: TriggerState::Disabled,  // <-- VIOLATES PRECONDITION
        variant: TriggerVariant::Cron,
    };

    registry.register(trigger).await.unwrap();  // <-- .unwrap() on expected Err
    // ...
}
```

### Failing Tests (9 total)

| Test Name | Issue |
|-----------|-------|
| `set_state_transitions_disabled_to_active_when_trigger_exists` | Tries to register with `Disabled` state |
| `set_state_transitions_disabled_to_paused_when_trigger_exists` | Tries to register with `Disabled` state |
| `set_state_transitions_error_to_active_for_polling_trigger` | Tries to register with `Error` state |
| `set_state_rejects_disabled_to_error_transition` | Tries to register with `Disabled` state |
| `set_state_rejects_error_to_active_for_cron_trigger` | Tries to register with `Error` state |
| `set_state_rejects_error_to_disabled_transition` | Tries to register with `Error` state |
| `set_state_rejects_error_to_paused_transition` | Tries to register with `Error` state |
| `fire_returns_trigger_disabled_when_trigger_is_disabled` | Tries to register with `Disabled` state |
| `fire_returns_trigger_in_error_state_when_polling_trigger_is_in_error` | Tries to register with `Error` state |

---

## Commands Run

### Baseline: Run all trigger tests

```bash
cd /home/lewis/src/twerk && timeout 60 cargo test -p twerk-core trigger::
```

**Result:** 9 FAILED, 351 passed

### Individual test verification

```bash
cd /home/lewis/src/twerk && cargo test -p twerk-core trigger::tests::set_state_transitions_disabled_to_active_when_trigger_exists
```

**Output:**
```
---- trigger::tests::set_state_transitions_disabled_to_active_when_trigger_exists stdout ----
thread 'trigger::tests::set_state_transitions_disabled_to_active_when_trigger_exists' (816065) panicked at crates/twerk-core/src/trigger.rs:1378:42:
called `Result::unwrap()` on an `Err` value: InvalidConfiguration("new triggers cannot start in Disabled state")
```

---

## Root Cause

The tests were written incorrectly. They assume:
1. You can register a trigger in any state
2. Then test state transitions or fire behavior

But the contract says:
1. You can ONLY register with `Active` or `Paused` state
2. To test `Disabled` or `Error` behavior, first register with `Active`/`Paused`, then use `set_state()` to transition

---

## Correct Test Pattern

```rust
// WRONG (current buggy tests):
let trigger = Trigger {
    state: TriggerState::Disabled,  // <-- Violates precondition
    ...
};
registry.register(trigger).await.unwrap();  // <-- Panics

// CORRECT (what tests should do):
let trigger = Trigger {
    state: TriggerState::Active,  // <-- Valid initial state
    ...
};
registry.register(trigger).await.unwrap();
registry.set_state(&id, TriggerState::Disabled).await.unwrap();  // <-- Transition first
// Now test behavior in Disabled state
```

---

## Additional Testing Probed

### Edge Cases Tested (all passed)

1. **TriggerId length boundaries**: 3 chars (min), 64 chars (max), 65 chars (overflow)
2. **TriggerId character validation**: Alphanumeric, hyphen, underscore OK; space, @, #, ., control chars rejected
3. **State transitions**: All valid transitions work correctly
4. **Invalid transitions**: Correctly rejected per state machine
5. **Fire behavior**: Correctly rejects Paused, Disabled, Error states
6. **Datastore unavailability**: All operations correctly return `DatastoreUnavailable`
7. **Broker unavailability**: `fire()` correctly returns `BrokerUnavailable`
8. **Concurrency limiting**: `fire()` correctly returns `ConcurrencyLimitReached`

---

## Findings Summary

| ID | Severity | Dimension | Finding |
|----|----------|-----------|---------|
| GEN-1-1 | MAJOR | trigger-state-invariants | Test `set_state_transitions_disabled_to_active_when_trigger_exists` violates `register` precondition |
| GEN-1-2 | MAJOR | trigger-state-invariants | Test `set_state_transitions_disabled_to_paused_when_trigger_exists` violates `register` precondition |
| GEN-1-3 | MAJOR | trigger-state-invariants | Test `set_state_transitions_error_to_active_for_polling_trigger` violates `register` precondition |
| GEN-1-4 | MAJOR | trigger-state-invariants | Test `set_state_rejects_disabled_to_error_transition` violates `register` precondition |
| GEN-1-5 | MAJOR | trigger-state-invariants | Test `set_state_rejects_error_to_active_for_cron_trigger` violates `register` precondition |
| GEN-1-6 | MAJOR | trigger-state-invariants | Test `set_state_rejects_error_to_disabled_transition` violates `register` precondition |
| GEN-1-7 | MAJOR | trigger-state-invariants | Test `set_state_rejects_error_to_paused_transition` violates `register` precondition |
| GEN-1-8 | MAJOR | trigger-state-invariants | Test `fire_returns_trigger_disabled_when_trigger_is_disabled` violates `register` precondition |
| GEN-1-9 | MAJOR | trigger-state-invariants | Test `fire_returns_trigger_in_error_state_when_polling_trigger_is_in_error` violates `register` precondition |

---

## Red Queen Verdict

**CROWN CONTESTED** — Implementation is correct, but test suite has bugs that violate contract preconditions.

The `TriggerRegistry` implementation correctly enforces the contract. The 9 failing tests need to be fixed to:
1. Register triggers with `Active` or `Paused` state (per contract)
2. Use `set_state()` to transition to `Disabled` or `Error` before testing behavior in those states

---

## Recommendations

1. **Fix tests** to follow the correct pattern: register with `Active`/`Paused`, then `set_state()` to desired state
2. **Add integration tests** that verify the full lifecycle: register → set_state → fire
3. **Add concurrent access tests** using `arc_multiple_owners` pattern to verify thread safety
4. **Add property-based tests** for state machine transitions using proptest

---

## Commands for Validation

```bash
# Verify implementation is correct (should pass)
cd /home/lewis/src/twerk && cargo test -p twerk-core trigger::tests::set_state_transitions_active_to_paused

# Verify buggy test (should fail - but this is a test bug, not impl bug)
cd /home/lewis/src/twerk && cargo test -p twerk-core trigger::tests::set_state_transitions_disabled_to_active
```
