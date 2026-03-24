# Decision: bytea vs jsonb for JSON Columns

Date: 2026-03-23
Status: Decided - Keep bytea

## Context

The Go version of Tork (`runabol/tork`) uses `jsonb` columns for JSON data in PostgreSQL.
The Rust port (`twerk`) uses `bytea` columns for the same data.

This creates a data format divergence: Go and Rust databases cannot share data directly.

## Decision

**Keep `bytea` columns in the Rust implementation.**

## Rationale

### 1. sqlx 0.7 Compatibility

The original rationale (documented in `schema.rs`):
> `bytea` instead of `jsonb` for JSON payloads (sqlx binds `Vec<u8>` as `bytea`

While sqlx 0.7 provides `sqlx::types::Json<T>` which maps to jsonb, the current approach
using `Vec<u8>` + manual `serde_json::to_vec()`/`serde_json::from_slice()` works correctly
and doesn't require fighting the type system.

### 2. Current Implementation Works

All 152 datastore tests pass. The current implementation:
- Stores JSON as UTF-8 bytes in bytea
- Correctly serializes/deserializes via serde_json
- Maintains data integrity

### 3. Migration Complexity

Switching to jsonb would require:

1. **Schema migration**: ALTER TABLE statements to convert bytea → jsonb
2. **Record type changes**: All `Vec<u8>` fields in records.rs → `sqlx::types::Json<T>`
3. **Data migration**: Convert existing bytea data to jsonb format
4. **Testing**: Full regression testing

For 20+ JSON columns across 3 tables (scheduled_jobs, jobs, tasks), this is non-trivial.

### 4. Performance Considerations

| Aspect | bytea | jsonb |
|--------|-------|-------|
| Storage | Raw bytes | Binary JSON (normalized) |
| Read speed | Fast | Fast (no parsing needed) |
| Indexing | Cannot index content | Can index with GIN |
| Query operators | None | Rich JSON operators (?>, ?>>, etc.) |

For this workload, the JSON columns are:
- Not queried with JSON operators (filtered via SQL columns or in-memory)
- Read/written as complete blobs
- Small relative to other data

### 5. Data Portability Trade-off

The PORT-GAPS.md notes this as a divergence. However:
- This is a **new project** (twerk), not a migration from Go
- Users choosing twerk over tork are starting fresh
- Cross-database compatibility isn't a current requirement

## Alternatives Considered

### 1. Switch to jsonb
**Rejected** - Requires schema migration, code changes, and testing. Not worth the effort
for a working system.

### 2. Hybrid Approach
Keep bytea for internal use but add jsonb views for Go compatibility.
**Rejected** - Adds complexity without clear benefit.

## Future Considerations

If twerk gains traction and cross-compatibility with Go's tork becomes important,
consider:
1. A migration script to convert bytea → jsonb
2. Adding GIN indexes on jsonb columns if JSON querying is needed
3. Using `sqlx::types::Json<serde_json::Value>` for flexible typing

## References

- sqlx 0.7 `Json<T>` type: <https://docs.rs/sqlx/latest/sqlx/types/struct.Json.html>
- PostgreSQL jsonb docs: <https://www.postgresql.org/docs/current/datatype-json.html>
- PORT-GAPS.md issue #15: `bytea` vs `jsonb` Column Types
