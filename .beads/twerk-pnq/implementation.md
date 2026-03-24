# Implementation Summary: Fix datastore gaps

## bead_id
`twerk-pnq`

## bead_title
Fix datastore gaps: Page fields, scheduled job permissions, count queries

## Date
2026-03-24

---

## Changes Made

### GAP6: Scheduled Job Permissions Not Inserted

**File:** `datastore/postgres/mod.rs`

**Problem:** `create_scheduled_job` inserted the scheduled job record but did NOT insert permissions into `scheduled_jobs_perms`.

**Fix:** Added permission insertion loop after the scheduled job INSERT (lines 1123-1145):

```rust
// GAP6: Insert scheduled job permissions
if let Some(perms) = &sj.permissions {
    for perm in perms {
        let (user_id, role_id) = match (&perm.user, &perm.role) {
            (Some(u), None) => (u.id.clone(), None),
            (None, Some(r)) => (None, r.id.clone()),
            _ => continue,
        };

        let perm_id = uuid::Uuid::new_v4().to_string().replace('-', "");

        sqlx::query(
            r"
            INSERT INTO scheduled_jobs_perms (id, scheduled_job_id, user_id, role_id)
            VALUES ($1, $2, $3, $4)
            ",
        )
        .bind(&perm_id)
        .bind(id)
        .bind(&user_id)
        .bind(&role_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            DatastoreError::Database(format!("create scheduled job perm failed: {e}"))
        })?;
    }
}
```

**Verification:** `integration_create_scheduled_job_inserts_permissions_when_provided` now passes.

---

### GAP7: GetScheduledJobs Count Query Ignores Permissions

**File:** `datastore/postgres/mod.rs`

**Problem:** The count query at line 1202 was `SELECT count(*) FROM scheduled_jobs` which ignored all permission filtering. The SELECT query used a CTE for permission filtering, but the count did not.

**Fix:** Replaced the count query with a permission-aware CTE that mirrors the SELECT query logic (lines 1231-1252):

```rust
// GAP7: Use permission-aware count query matching the SELECT CTE logic
let count: i64 = sqlx::query_scalar(
    r"
    WITH user_info AS (
        SELECT id AS user_id FROM users WHERE username_ = $1
    ),
    role_info AS (
        SELECT role_id FROM users_roles ur
        JOIN user_info ui ON ur.user_id = ui.user_id
    ),
    job_perms_info AS (
        SELECT scheduled_job_id FROM scheduled_jobs_perms jp
        WHERE jp.user_id = (SELECT user_id FROM user_info)
        OR jp.role_id IN (SELECT role_id FROM role_info)
    ),
    no_job_perms AS (
        SELECT j.id as scheduled_job_id FROM scheduled_jobs j
        WHERE NOT EXISTS (SELECT 1 FROM scheduled_jobs_perms jp WHERE j.id = jp.scheduled_job_id)
    )
    SELECT count(*) FROM scheduled_jobs j
    WHERE ($1 = '' OR EXISTS (SELECT 1 FROM no_job_perms njp WHERE njp.scheduled_job_id=j.id) 
        OR EXISTS (SELECT 1 FROM job_perms_info jpi WHERE jpi.scheduled_job_id = j.id))
    ",
)
.bind(current_user)
.fetch_one(&self.pool)
.await
.map_err(|e| DatastoreError::Database(format!("count scheduled jobs failed: {e}")))?;
```

**Verification:** `integration_get_scheduled_jobs_count_respects_permissions` now passes.

---

### GAP9: Column Order Mismatch in create_scheduled_job

**File:** `datastore/postgres/mod.rs`

**Problem:** The INSERT statement had incorrect column order. The bind values were misaligned with the schema columns:

| Position | Was Binding To | Schema Column |
|----------|---------------|---------------|
| 4 | `sj.cron` | `tags` (should be) |
| 5 | `sj.state` | `cron_expr` |
| 12 | `created_by_id` | `created_at` |
| 13 | `tags` | `created_by` |
| 14 | `sj.created_at` | `state` |
| 15 | `sj.output` | `output_` |

**Fix:** Corrected the INSERT column order and bind values (lines 1097-1121):

```rust
sqlx::query(
    r"
    INSERT INTO scheduled_jobs (id, name, description, tags, cron_expr, inputs, output_,
        tasks, defaults, webhooks, auto_delete, secrets, created_at, created_by, state)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
    ",
)
.bind(id)
.bind(&sj.name)
.bind(&sj.description)
.bind(tags)           // $4: tags
.bind(&sj.cron)       // $5: cron_expr
.bind(&inputs)        // $6: inputs
.bind(&sj.output)     // $7: output_
.bind(&tasks)         // $8: tasks
.bind(&defaults)      // $9: defaults
.bind(&webhooks)      // $10: webhooks
.bind(&auto_delete)   // $11: auto_delete
.bind(&secrets_bytes) // $12: secrets
.bind(sj.created_at)  // $13: created_at
.bind(created_by_id)  // $14: created_by
.bind(&sj.state)      // $15: state
```

**Verification:** All `integration_create_scheduled_job_stores_*` tests pass:
- `integration_create_scheduled_job_stores_cron_expr_correctly`
- `integration_create_scheduled_job_stores_state_correctly`
- `integration_create_scheduled_job_stores_created_at_correctly`
- `integration_create_scheduled_job_stores_output_correctly`
- `integration_create_scheduled_job_stores_tags_correctly`

---

## Tests Passing After Fixes

| Test | Status |
|------|--------|
| `integration_create_scheduled_job_inserts_permissions_when_provided` | ✅ PASS |
| `integration_create_scheduled_job_succeeds_without_permissions` | ✅ PASS |
| `integration_get_scheduled_jobs_count_respects_permissions` | ✅ PASS |
| `integration_create_scheduled_job_stores_cron_expr_correctly` | ✅ PASS |
| `integration_create_scheduled_job_stores_state_correctly` | ✅ PASS |
| `integration_create_scheduled_job_stores_created_at_correctly` | ✅ PASS |
| `integration_create_scheduled_job_stores_output_correctly` | ✅ PASS |
| `integration_create_scheduled_job_stores_tags_correctly` | ✅ PASS |

## Remaining Pre-existing Issues (Not Addressed)

These test failures existed before this fix and are unrelated to the GAPs addressed:

1. **`integration_update_job_*`** - Fails with `no column found for name: ts`. This is a pre-existing issue with the `jobs.ts` column (a `tsvector` generated column) that cannot be mapped to `Option<String>` in `JobRecord`. Requires type fix in `JobRecord` struct or explicit column selection instead of `SELECT *`.

2. **`integration_assign_role_*`** - Fails with FK constraint violations. Pre-existing test setup issue where referenced records don't exist.

3. **`integration_get_scheduled_jobs_returns_correct_page_metadata`** / **`integration_get_scheduled_jobs_page_overflow_returns_empty_items`** - Test expectations don't match contract PO4. Tests expect `Page::size` to be the requested page size, but PO4 states `size` equals `items.len()` (actual item count).

## Files Changed

- `datastore/postgres/mod.rs` - GAP6, GAP7, GAP9 fixes

## Constraint Adherence

- ✅ No `unwrap()`, `expect()`, or `panic!()` added
- ✅ Zero `mut` introduced in core logic
- ✅ All errors properly handled with `map_err` and `?` propagation
- ✅ Expression-based patterns used where applicable
- ✅ Clippy: Build succeeds with no new warnings
