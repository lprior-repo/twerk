bead_id: twerk-r4l
bead_title: action: Implement trigger update endpoint (PUT /api/v1/triggers/{id})
phase: state-1-contract
updated_at: 2026-04-13T12:13:13Z

# Contract Specification: Trigger Update Endpoint

## Scope

Define the Design-by-Contract for updating an existing trigger via:

- `PUT /api/v1/triggers/{id}`

This contract covers only update behavior for this endpoint.

## Domain Terms

- `TriggerId`: canonical identifier of a trigger.
- `Trigger`: persisted domain entity for an automation trigger.
- `TriggerUpdateRequest`: API payload for replacing mutable trigger fields.
- `TriggerView`: API response DTO after successful update.
- `Datastore`: persistence abstraction used by the handler.

## Type Contracts

```rust
pub struct TriggerId(String); // validated value object

pub struct TriggerUpdateRequest {
    pub name: String,
    pub enabled: bool,
    pub event: String,
    pub condition: Option<String>,
    pub action: String,
    pub metadata: Option<std::collections::HashMap<String, String>>,
    pub id: Option<String>, // optional in body; if present must equal path id
}

pub struct Trigger {
    pub id: TriggerId,
    pub name: String,
    pub enabled: bool,
    pub event: String,
    pub condition: Option<String>,
    pub action: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

pub struct TriggerView {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub event: String,
    pub condition: Option<String>,
    pub action: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}
```

## Invariants

- `Trigger.id` is immutable after creation.
- `Trigger.created_at` is immutable after creation.
- `Trigger.updated_at >= Trigger.created_at` always holds.
- `Trigger.name.trim().is_empty() == false`.
- `Trigger.event.trim().is_empty() == false`.
- `Trigger.action.trim().is_empty() == false`.
- `metadata` keys are unique, non-empty, and ASCII-safe.
- `PUT` is idempotent: applying same valid request repeatedly yields the same persisted mutable state.

## Preconditions

- Path parameter `{id}` is present and parses into `TriggerId`.
- Request `Content-Type` is supported (`application/json`).
- Body is valid JSON and deserializes into `TriggerUpdateRequest`.
- Required fields are present and valid:
  - `name` non-empty after trim
  - `event` non-empty after trim
  - `action` non-empty after trim
- If `body.id` is provided, `body.id == path.id`.
- Target trigger exists in datastore.

## Postconditions

On `Ok(TriggerView)`:

- Exactly one existing trigger with `id == path.id` is updated.
- Persisted `id` remains unchanged.
- Persisted `created_at` remains unchanged.
- Mutable fields (`name`, `enabled`, `event`, `condition`, `action`, `metadata`) equal normalized request values.
- `updated_at` is set to current UTC time and is `>=` prior `updated_at`.
- Returned `TriggerView` is a faithful projection of persisted state after commit.

On `Err`:

- No partial update is committed (atomicity).
- Existing persisted trigger remains unchanged.

## Error Taxonomy

```rust
pub enum TriggerUpdateError {
    // 400
    InvalidIdFormat(String),          // path id cannot parse into TriggerId
    UnsupportedContentType(String),   // non-json content type
    MalformedJson(String),            // body parse failure
    ValidationFailed(String),         // field-level contract violation
    IdMismatch { path_id: String, body_id: String },

    // 404
    TriggerNotFound(String),          // no trigger for path id

    // 409 (optional concurrency guard if enabled)
    VersionConflict(String),

    // 500
    Persistence(String),              // datastore update/read-back failure
    Serialization(String),            // response serialization failure
}
```

### HTTP Mapping Contract

- `InvalidIdFormat | UnsupportedContentType | MalformedJson | ValidationFailed | IdMismatch` -> `400 Bad Request`
- `TriggerNotFound` -> `404 Not Found`
- `VersionConflict` -> `409 Conflict` (when optimistic concurrency is enabled)
- `Persistence | Serialization` -> `500 Internal Server Error` (sanitized message)

## Contract Signatures

```rust
pub async fn update_trigger_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, ApiError>;

// Note: The implementation uses InMemoryTriggerDatastore directly.
// The TriggerDatastore trait abstraction is not required for this endpoint.

pub fn validate_trigger_update(
    path_id: &str,
    req: &TriggerUpdateRequest,
) -> Result<TriggerId, TriggerUpdateError>;

pub fn apply_trigger_update(
    current: Trigger,
    req: TriggerUpdateRequest,
    now_utc: time::OffsetDateTime,
) -> Result<Trigger, TriggerUpdateError>;
```

## Ownership and Mutation Contract

- Validation functions are pure (no I/O, deterministic for same inputs).
- Mutation is explicit: `apply_trigger_update` consumes `current` and `req`, returns new `Trigger`.
- Side effects (datastore write, response emission) occur only after validation success.
- No panics; all fallible operations use `Result<T, TriggerUpdateError>`.

## Non-goals

- Create trigger endpoint semantics.
- Delete trigger endpoint semantics.
- Trigger execution/runtime behavior.
- Bulk update of triggers.
