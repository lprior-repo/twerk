bead_id: twerk-pnq
bead_title: Fix datastore gaps: Page fields, scheduled job permissions, count queries
phase: 1
updated_at: 2026-03-24T00:00:00Z

# Contract Specification

## Context

- **Feature:** Datastore gap fixes for PostgreSQL implementation
- **Domain terms:**
  - `Page<T>`: Generic paginated result container
  - `ScheduledJob`: Cron-scheduled recurring job with permissions
  - `Permission`: Access control linking jobs/scheduled jobs to users or roles
  - `Job`: Executable unit with tasks and permissions
  - `with_tx`: Transaction helper for atomic read-modify-write operations
- **Assumptions:**
  - The `Page` struct in `datastore/mod.rs` uses correct field names (`total_items`, `number`)
  - All update operations must use `with_tx` for transaction safety
  - Scheduled job permissions must be inserted alongside the scheduled job record
  - Count queries for paginated results must respect permission filtering
- **Open questions:**
  - None identified; codebase analysis complete

---

## GAP1: Page Struct Field Names

### Current State
The `Page<T>` struct in `datastore/mod.rs` (lines 89-101) defines:
```rust
pub struct Page<T> {
    pub items: Vec<T>,
    pub number: i64,       // current page number
    pub size: i64,
    pub total_pages: i64,
    pub total_items: i64,  // total count of items
}
```

### Invariants
- [I1] `Page::number >= 1` (page numbers are 1-indexed)
- [I2] `Page::size >= 1` (page size must be positive)
- [I3] `Page::total_pages >= 0`
- [I4] `Page::total_items >= 0`
- [I5] If `Page::total_items > 0` then `Page::total_pages >= 1`
- [I6] `Page::items.len() <= Page::size`

### Contract
No changes required. The field names are correct.

---

## GAP6: Scheduled Job Permissions Not Inserted

### Current State
`create_scheduled_job` (postgres/mod.rs lines 1048-1123) inserts the scheduled job record but does NOT insert permissions into `scheduled_jobs_perms`. Compare to `create_job` (lines 781-806) which correctly inserts job permissions.

### Preconditions
- [PC1] `sj.id` must be `Some(String)` (non-empty scheduled job ID)
- [PC2] `sj.created_by` must be `Some(User)` where `user.id` is `Some(String)`
- [PC3] If `sj.permissions` is `Some(perms)`, each permission must have either `user.id` or `role.id` set

### Postconditions
- [PO1] A row is inserted into `scheduled_jobs` table with all scheduled job fields
- [PO2] For each permission in `sj.permissions`, a corresponding row is inserted into `scheduled_jobs_perms` with `scheduled_job_id` referencing the created scheduled job
- [PO3] Permission insertion is atomic with scheduled job insertion (same transaction)
- [PO4] `scheduled_jobs_perms.created_at` is set to current timestamp

### Error Taxonomy
- `Error::InvalidInput("scheduled job id is required")` if `sj.id` is None
- `Error::InvalidInput("created_by is required")` if `sj.created_by` is None
- `Error::InvalidInput("created_by.id is required")` if `sj.created_by.id` is None
- `Error::Database(String)` if SQL execution fails

### Contract Signatures
```rust
async fn create_scheduled_job(&self, sj: &ScheduledJob) -> DatastoreResult<()>
```

---

## GAP7: GetScheduledJobs Count Query Ignores Permissions

### Current State
`get_scheduled_jobs` (postgres/mod.rs lines 1152-1217) uses a CTE for permission-filtered record selection, but the count query at line 1202 is:
```sql
SELECT count(*) FROM scheduled_jobs
```
This ignores all permission filtering and returns the total count of ALL scheduled jobs.

### Preconditions
- [PC1] `current_user` must be a valid username string
- [PC2] `page >= 1`
- [PC3] `size >= 1`

### Postconditions
- [PO1] Returned `Page::total_items` equals the count of scheduled jobs the current user has permission to view
- [PO2] `Page::items` contains only scheduled jobs the current user has permission to view
- [PO3] `Page::number` equals the requested page
- [PO4] `Page::size` equals the number of items in `items` (not the requested size for the last page)
- [PO5] `Page::total_pages` is correctly computed as `ceil(total_items / requested_size)`

### Error Taxonomy
- `Error::Database(String)` if count query fails

### Contract Signatures
```rust
async fn get_scheduled_jobs(
    &self,
    current_user: &str,
    page: i64,
    size: i64,
) -> DatastoreResult<Page<ScheduledJobSummary>>
```

---

## GAP9: Column Order Mismatch in create_scheduled_job

### Current State
The INSERT statement in `create_scheduled_job` (lines 1097-1101):
```sql
INSERT INTO scheduled_jobs (id, name, description, cron_expr, state, tasks, inputs,
    defaults, webhooks, auto_delete, secrets, created_by, tags, created_at, output_)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
```

The `scheduled_jobs` table schema (schema.rs lines 58-74):
```sql
CREATE TABLE scheduled_jobs (
  id             varchar(32) not null primary key,
  name           varchar(64) not null,
  description    text        not null,
  tags           text[]      not null default '{}',
  cron_expr      varchar(64) not null,
  inputs         bytea       not null,
  output_        text        not null,
  tasks          bytea       not null,
  defaults       bytea,
  webhooks       bytea,
  auto_delete    bytea,
  secrets        bytea,
  created_at     timestamptz not null,
  created_by     varchar(32) not null references users(id),
  state          varchar(10) not null
);
```

### Column Position Mapping (Current Bug)
| Position | INSERT Column | Schema Column | Value Bound |
|----------|--------------|---------------|-------------|
| 4 | `cron_expr` | `tags` | `$4` = sj.cron |
| 5 | `state` | `cron_expr` | `$5` = sj.state |
| 14 | `created_at` | `created_by` | `$14` = sj.created_at |
| 15 | `output_` | `state` | `$15` = sj.output |

### Preconditions
- Same as GAP6 PC1-PC3

### Postconditions
- [PO1] All columns receive values matching their schema definitions
- [PO2] `cron_expr` receives `sj.cron` (the cron expression string)
- [PO3] `state` receives `sj.state` (the state string)
- [PO4] `created_at` receives `sj.created_at` (timestamp)
- [PO5] `output_` receives `sj.output` (output string)
- [PO6] `tags` receives `sj.tags` (string array)

### Correct INSERT Column Order
```sql
INSERT INTO scheduled_jobs (id, name, description, tags, cron_expr, inputs, output_,
    tasks, defaults, webhooks, auto_delete, secrets, created_at, created_by, state)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
```

### Contract Signatures
```rust
async fn create_scheduled_job(&self, sj: &ScheduledJob) -> DatastoreResult<()>
```

---

## GAP2/3/4: Update Methods Callback Pattern and WithTx

### Current State
The update methods (`update_task`, `update_node`, `update_job`, `update_scheduled_job`) all manually manage transactions:
```rust
let mut tx = self.pool.begin().await?;
let record = sqlx::query_as(...).fetch_optional(&mut *tx).await?;
let mut entity = record.to_...()?;
modify(&mut entity)?;
sqlx::query(...).execute(&mut *tx).await?;
tx.commit().await?;
```

A `with_tx` helper exists (lines 342-367) but is NOT used by update methods.

### Invariants
- [I1] Update operations MUST be atomic (read-modify-write in single transaction)
- [I2] `FOR UPDATE` lock MUST be acquired on the record before modification
- [I3] On error, transaction MUST be rolled back
- [I4] On success, transaction MUST be committed

### Preconditions
- [PC1] `id` must reference an existing entity
- [PC2] `modify` callback must return `Ok(())` for successful modification
- [PC3] `modify` callback may return `Err(DatastoreError)` to abort the transaction

### Postconditions
- [PO1] The entity is updated in the database with all modifications made by `modify`
- [PO2] If `modify` returns `Err`, no changes are persisted (transaction rolled back)
- [PO3] The `FOR UPDATE` lock is released on commit/rollback

### Design Note: WithTx Usage
Refactoring to use `with_tx` would improve consistency:
```rust
pub async fn update_job<F>(&self, id: &str, modify: F) -> DatastoreResult<()>
where
    F: FnOnce(&mut Job) -> DatastoreResult<()>,
{
    self.with_tx(|tx| async move {
        // fetch with FOR UPDATE
        // apply modify
        // update
        Ok(())
    }).await
}
```

This is a refactoring opportunity but does not change the contract semantics.

### Contract Signatures
```rust
async fn update_task<F>(&self, id: &str, modify: F) -> DatastoreResult<()>
where
    F: FnOnce(&mut Task) -> DatastoreResult<()>;

async fn update_node<F>(&self, id: &str, modify: F) -> DatastoreResult<()>
where
    F: FnOnce(&mut Node) -> DatastoreResult<()>;

async fn update_job<F>(&self, id: &str, modify: F) -> DatastoreResult<()>
where
    F: FnOnce(&mut Job) -> DatastoreResult<()>;

async fn update_scheduled_job<F>(&self, id: &str, modify: F) -> DatastoreResult<()>
where
    F: FnOnce(&mut ScheduledJob) -> DatastoreResult<()>;
```

---

## Error Taxonomy (Consolidated)

All datastore operations return `DatastoreResult<T>` = `Result<T, Error>` where `Error` variants are:

| Variant | When Raised |
|---------|-------------|
| `Error::TaskNotFound` | Task ID does not exist in database |
| `Error::NodeNotFound` | Node ID does not exist in database |
| `Error::JobNotFound` | Job ID does not exist in database |
| `Error::ScheduledJobNotFound` | Scheduled job ID does not exist in database |
| `Error::UserNotFound` | User ID/username does not exist in database |
| `Error::RoleNotFound` | Role ID/slug does not exist in database |
| `Error::ContextNotFound` | Context ID does not exist in database |
| `Error::Database(String)` | SQL execution failure (connection, constraint, etc.) |
| `Error::Serialization(String)` | JSON serialization/deserialization failure |
| `Error::Encryption(String)` | Secret encryption/decryption failure |
| `Error::InvalidInput(String)` | Precondition violation (missing required fields, invalid values) |
| `Error::Transaction(String)` | Transaction management failure (begin, commit, rollback) |

---

## Non-Goals
- [NG1] Schema changes to `scheduled_jobs` table structure
- [NG2] Changes to `Page::items` field semantics
- [NG3] Modifying `create_job` behavior (already correct)
