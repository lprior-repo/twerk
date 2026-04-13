# TriggerRegistry Contract

## Overview

This contract defines the behavior of the `TriggerRegistry` trait and `InMemoryTriggerRegistry` implementation in `crates/twerk-core/src/trigger.rs`.

## Core Types

### TriggerId
- **Validation**: 3-64 characters, alphanumeric, hyphen (`-`), underscore (`_`) only
- **Error**: `TriggerIdError::LengthOutOfRange(n)` or `TriggerIdError::InvalidCharacter(c)`

### TriggerState
- `Active` - Can fire, retains resources
- `Paused` - Cannot fire, retains resources  
- `Disabled` - Cannot fire, releases all resources
- `Error` - Terminal state for polling failures, requires manual resume

### TriggerVariant
- `Cron` - Cron expression-based scheduling
- `Webhook` - HTTP endpoint trigger
- `Polling` - Periodic HTTP polling with failure tracking

## TriggerRegistry Trait Contract

### register(trigger)

**Preconditions:**
- `trigger.id` MUST be valid `TriggerId` (3-64 chars, alphanumeric/-/_)
- `trigger.state` MUST be `Active` or `Paused`
- No trigger with `trigger.id` may already exist

**Postconditions:**
- Returns `Ok(())` on success
- Returns `Err(TriggerError::AlreadyExists)` if ID exists
- Returns `Err(TriggerError::InvalidConfiguration)` if state is `Disabled` or `Error`

### unregister(id)

**Preconditions:**
- Trigger with `id` MUST exist

**Postconditions:**
- Returns `Ok(())` on success
- Returns `Err(TriggerError::NotFound)` if ID doesn't exist

### set_state(id, new_state)

**Preconditions:**
- Trigger with `id` MUST exist
- State transition MUST be valid per state transition table

**Valid Transitions:**
| From | To | Valid For |
|------|----|-----------|
| Active | Paused | All |
| Active | Disabled | All |
| Active | Error | Polling only |
| Paused | Active | All |
| Paused | Disabled | All |
| Disabled | Active | All |
| Disabled | Paused | All |
| Error | Active | Polling only |
| Any | Self | All |

**Postconditions:**
- Returns `Ok(())` on success
- Returns `Err(TriggerError::NotFound)` if ID doesn't exist
- Returns `Err(TriggerError::InvalidStateTransition)` if transition not allowed

### get(id)

**Preconditions:** None (idempotent read)

**Postconditions:**
- Returns `Ok(Some(trigger))` if exists
- Returns `Ok(None)` if not found

### list()

**Preconditions:** None (idempotent read)

**Postconditions:**
- Returns `Ok(Vec<Trigger>)` containing all triggers

### list_by_state(target_state)

**Preconditions:** None (idempotent read)

**Postconditions:**
- Returns `Ok(Vec<Trigger>)` containing only triggers with `state == target_state`

### fire(ctx)

**Preconditions:**
- Trigger with `ctx.trigger_id` MUST exist
- Trigger MUST be in `Active` state
- Broker (job queue) must be available

**Postconditions:**
- Returns `Ok(JobId)` on success
- Returns `Err(TriggerError::NotFound)` if trigger doesn't exist
- Returns `Err(TriggerError::TriggerNotActive)` if trigger is `Paused`
- Returns `Err(TriggerError::TriggerDisabled)` if trigger is `Disabled`
- Returns `Err(TriggerError::TriggerInErrorState)` if Polling trigger is in `Error`
- Returns `Err(TriggerError::BrokerUnavailable)` if broker unavailable
- Returns `Err(TriggerError::ConcurrencyLimitReached)` if limit hit

## Thread Safety

All implementations MUST implement `Send + Sync`.

## Error Handling

All mutating operations check `datastore_available` and return `TriggerError::DatastoreUnavailable` if unavailable.
