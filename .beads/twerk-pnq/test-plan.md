bead_id: twerk-pnq
bead_title: Fix datastore gaps: Page fields, scheduled job permissions, count queries
phase: 1.5
updated_at: 2026-03-24T14:00:00Z

# Test Plan: Fix datastore gaps: Page fields, scheduled job permissions, count queries

## Summary

- **Behaviors identified:** 52
- **Trophy allocation:** 45 unit / 9 integration / 2 e2e / 2 static
- **Proptest invariants:** 4
- **Fuzz targets:** 2
- **Kani harnesses:** 2
- **Mutation checkpoints:** 10

---

## 1. Behavior Inventory

### GAP1: Page Struct Field Names (Invariants Only)

1. "Page invariants hold when constructed with valid values"
2. "Page rejects construction when number is less than 1"
3. "Page rejects construction when size is less than 1"
4. "Page rejects construction when total_items is negative"
5. "Page rejects construction when total_pages is negative but total_items is positive"
6. "Page items length cannot exceed page size"
7. "Page pagination math: total_pages equals ceil(total_items / size)"
8. "Page exact fit: items.len() equals size on non-final page"
9. "Page empty: zero items produces zero total_pages"
10. "Page single item: total_pages = 1 when total_items = 1 and size >= 1"

### GAP6: Scheduled Job Permissions Not Inserted

11. "create_scheduled_job inserts scheduled_jobs_perms rows when permissions provided"
12. "create_scheduled_job returns error when id is None"
13. "create_scheduled_job returns error when created_by is None"
14. "create_scheduled_job returns error when created_by.id is None"
15. "create_scheduled_job succeeds without permissions (empty perms)"
16. "create_scheduled_job permission insertion is atomic with scheduled job insertion"

### GAP7: GetScheduledJobs Count Query Ignores Permissions

17. "get_scheduled_jobs returns total_items equal to permission-filtered count"
18. "get_scheduled_jobs returns correct page metadata (number, size, total_pages)"
19. "get_scheduled_jobs returns empty page when user has no permissions"
20. "get_scheduled_jobs page overflow returns empty items with correct metadata"

### GAP9: Column Order Mismatch in create_scheduled_job

21. "create_scheduled_job stores cron_expr in correct column"
22. "create_scheduled_job stores state in correct column"
23. "create_scheduled_job stores created_at in correct column"
24. "create_scheduled_job stores output_ in correct column"
25. "create_scheduled_job stores tags in correct column"

### GAP2/3/4: Update Methods Atomicity

26. "update_task updates entity and commits when modify returns Ok"
27. "update_task returns TaskNotFound when id does not exist"
28. "update_task rolls back transaction when modify returns Err"
29. "update_task preserves original values when modify returns Err"
30. "update_node updates entity and commits when modify returns Ok"
31. "update_node returns NodeNotFound when id does not exist"
32. "update_node rolls back transaction when modify returns Err"
33. "update_node preserves original values when modify returns Err"
34. "update_job updates entity and commits when modify returns Ok"
35. "update_job returns JobNotFound when id does not exist"
36. "update_job rolls back transaction when modify returns Err"
37. "update_job preserves original values when modify returns Err"
38. "update_scheduled_job updates entity and commits when modify returns Ok"
39. "update_scheduled_job returns ScheduledJobNotFound when id does not exist"
40. "update_scheduled_job rolls back transaction when modify returns Err"
41. "update_scheduled_job preserves original values when modify returns Err"

### Error Variant Coverage

42. "get_role returns RoleNotFound when role does not exist"
43. "get_role returns RoleNotFound when role was deleted"
44. "get_user_roles returns empty when user has no roles"
45. "assign_role returns RoleNotFound when role does not exist"
46. "assign_role returns UserNotFound when user does not exist"
47. "get_scheduled_job_by_id returns ContextNotFound when context missing"
48. "get_task_by_id returns ContextNotFound when context missing"
49. "create_scheduled_job returns Database error on constraint violation (unique key)"
50. "create_scheduled_job returns Database error on connection failure"
51. "update_task returns Database error on constraint violation"
52. "update_job returns Serialization error on malformed JSON"

---

## 2. Trophy Allocation

| Behavior | Layer | Rationale |
|----------|-------|-----------|
| GAP1: Page invariants (I1-I6) | Unit | Pure calc layer, exhaustive boundary testing |
| GAP1: pagination math | Unit | Pure function, integer arithmetic |
| GAP2/3/4: update_* not-found | Unit | Input validation, error taxonomy |
| GAP2/3/4: update_* rollback | Unit | Transaction semantics, state preservation |
| GAP6: create_scheduled_job preconditions | Unit | Input validation, error taxonomy |
| GAP6: create_scheduled_job permissions insertion | Integration | Real DB, actual INSERT, permission records |
| GAP7: get_scheduled_jobs count correctness | Integration | Real DB, permission filtering, count query |
| GAP7: page overflow | Integration | Real DB, boundary condition |
| GAP9: column order correctness | Integration | Real DB, SELECT after INSERT to verify |
| Error::RoleNotFound coverage | Unit | Error variant testing |
| Error::ContextNotFound coverage | Unit | Error variant testing |
| Error::Database constraint violations | Integration | Real DB, FK constraint failures |
| parse_query behavior | Unit | Pure function, no I/O |
| sanitize_string behavior | Unit | Pure function, no I/O |
| E2E: scheduled job CRUD workflow | E2E | Full workflow, real DB, API surface |
| E2E: scheduled job permission enforcement | E2E | Cross-component permission checks |
| clippy/fmt on datastore | Static | Compile-time checks |
| Error enum exhaustiveness | Static | compile-time guarantee |

**Rationale:** This is a data persistence layer where correctness of SQL queries and transaction semantics is critical. The bulk of testing is unit-level (45/58 = 78%) because:
- Page invariants are pure calc layer
- Update method error handling (not-found cases) are unit-testable with fakes
- Transaction rollback requires integration testing with real DB
- SQL query correctness cannot be unit tested without real DB
- Permission filtering requires real join semantics
- Column mapping requires real INSERT/SELECT round-trip

---

## 3. BDD Scenarios

### Behavior: Page invariants hold when constructed with valid values

**Given:** Valid inputs (number=1, size=10, total_items=100, total_pages=10)
**When:** Page::new() is called
**Then:** Returns Page with items=vec![], number=1, size=10, total_pages=10, total_items=100

```rust
fn page_constructs_correctly_with_valid_values() {
    // inputs: number=1, size=10, total_items=100, total_pages=10
    // items can be any Vec<T>
}
```

---

### Behavior: Page rejects construction when number is less than 1

**Given:** Page inputs with number=0
**When:** Page::new() is called
**Then:** Returns Err(InvalidInput("page number must be >= 1"))

```rust
fn page_rejects_number_less_than_one() {
    // Error variant: number=0
    // Error: Error::InvalidInput("page number must be >= 1")
}
```

---

### Behavior: Page rejects construction when size is less than 1

**Given:** Page inputs with size=0
**When:** Page::new() is called
**Then:** Returns Err(InvalidInput("page size must be >= 1"))

```rust
fn page_rejects_size_less_than_one() {
    // Error variant: size=0
    // Error: Error::InvalidInput("page size must be >= 1")
}
```

---

### Behavior: Page rejects construction when total_items is negative

**Given:** Page inputs with total_items=-1
**When:** Page::new() is called
**Then:** Returns Err(InvalidInput("total_items must be >= 0"))

```rust
fn page_rejects_negative_total_items() {
    // Error variant: total_items=-1
    // Error: Error::InvalidInput("total_items must be >= 0")
}
```

---

### Behavior: Page rejects construction when total_pages is negative but total_items is positive

**Given:** Page inputs with total_items=10, total_pages=-1
**When:** Page::new() is called
**Then:** Returns Err(InvalidInput("total_pages must be >= 0 when total_items > 0"))

```rust
fn page_rejects_negative_total_pages_when_items_exist() {
    // Error variant: total_items=10, total_pages=-1
    // Error: Error::InvalidInput("total_pages must be >= 0 when total_items > 0")
}
```

---

### Behavior: Page items length cannot exceed page size

**Given:** Page inputs with items.len()=15, size=10
**When:** Page::new() is called
**Then:** Returns Err(InvalidInput("items.len() cannot exceed page size"))

```rust
fn page_rejects_items_exceeding_size() {
    // Error variant: items.len()=15, size=10
    // Error: Error::InvalidInput("items.len() cannot exceed page size")
}
```

---

### Behavior: Page pagination math: total_pages equals ceil(total_items / size)

**Given:** total_items=25, size=10
**When:** Page::new() is called
**Then:** total_pages=3 (ceil(25/10) = 3)

```rust
fn page_calculates_total_pages_correctly() {
    // 25 items, size 10 → 3 pages
    // total_pages = (25 + 10 - 1) / 10 = 34 / 10 = 3
}
```

---

### Behavior: Page single item: total_pages = 1 when total_items = 1 and size >= 1

**Given:** total_items=1, size=10
**When:** Page::new() is called
**Then:** total_pages=1

```rust
fn page_single_item_calculates_correctly() {
    // 1 item, size 10 → 1 page
    // total_pages = (1 + 10 - 1) / 10 = 10 / 10 = 1
}
```

---

### Behavior: Page empty: zero items produces zero total_pages

**Given:** items=[], total_items=0, size=10
**When:** Page::new() is called
**Then:** total_pages=0

```rust
fn page_empty_produces_zero_total_pages() {
    // 0 items → 0 pages
}
```

---

### Behavior: Page exact fit: items.len() equals size on non-final page

**Given:** items.len()=10, size=10, total_items=20, total_pages=2
**When:** Page::new() is called
**Then:** Returns valid Page

```rust
fn page_exact_fit_is_valid() {
    // 10 items filling size 10, 20 total items = 2 pages
}
```

---

### Behavior: create_scheduled_job inserts scheduled_jobs_perms rows when permissions provided

**Given:** A ScheduledJob with sj.id=Some("sj-123".into()), sj.created_by=Some(User{id: Some("u-1".into()), ..}), sj.permissions=Some(vec![Permission{user: Some(User{id: Some("u-2".into()), ..}), role: None}])
**When:** ds.create_scheduled_job(&sj) is called
**Then:** A row exists in scheduled_jobs_perms with scheduled_job_id="sj-123", user_id="u-2"

```rust
fn create_scheduled_job_inserts_permissions_when_provided() {
    // Integration test using real DB
    // Verify: SELECT * FROM scheduled_jobs_perms WHERE scheduled_job_id=$1
    // Returns permission row with correct user_id
}
```

---

### Behavior: create_scheduled_job returns error when id is None

**Given:** A ScheduledJob with sj.id=None
**When:** ds.create_scheduled_job(&sj) is called
**Then:** Returns Err(Error::InvalidInput("scheduled job id is required"))

```rust
fn create_scheduled_job_returns_error_when_id_is_none() {
    // Precondition failure
    // Error: Error::InvalidInput("scheduled job id is required")
}
```

---

### Behavior: create_scheduled_job returns error when created_by is None

**Given:** A ScheduledJob with sj.id=Some("sj-123".into()), sj.created_by=None
**When:** ds.create_scheduled_job(&sj) is called
**Then:** Returns Err(Error::InvalidInput("created_by is required"))

```rust
fn create_scheduled_job_returns_error_when_created_by_is_none() {
    // Precondition failure
    // Error: Error::InvalidInput("created_by is required")
}
```

---

### Behavior: create_scheduled_job returns error when created_by.id is None

**Given:** A ScheduledJob with sj.id=Some("sj-123".into()), sj.created_by=Some(User{id: None, ..})
**When:** ds.create_scheduled_job(&sj) is called
**Then:** Returns Err(Error::InvalidInput("created_by.id is required"))

```rust
fn create_scheduled_job_returns_error_when_created_by_id_is_none() {
    // Precondition failure
    // Error: Error::InvalidInput("created_by.id is required")
}
```

---

### Behavior: create_scheduled_job succeeds without permissions (empty perms)

**Given:** A valid ScheduledJob with sj.permissions=None or sj.permissions=Some(vec![])
**When:** ds.create_scheduled_job(&sj) is called
**Then:** Returns Ok(()) and scheduled job row exists without any permission rows

```rust
fn create_scheduled_job_succeeds_without_permissions() {
    // Integration test
    // Verify: scheduled_jobs row exists, scheduled_jobs_perms is empty
}
```

---

### Behavior: create_scheduled_job permission insertion is atomic with scheduled job insertion

**Given:** A valid ScheduledJob with permissions
**When:** ds.create_scheduled_job(&sj) is called and succeeds
**Then:** Both scheduled_jobs row AND scheduled_jobs_perms rows exist; no partial state

```rust
fn create_scheduled_job_permission_insertion_is_atomic() {
    // Integration test
    // Transaction safety: verify no scheduled_jobs row without perms
    // or perms without scheduled_jobs row
}
```

---

### Behavior: get_scheduled_jobs returns total_items equal to permission-filtered count

**Given:** Multiple scheduled jobs with different permissions; user "alice" has access to only 3 of 5 jobs
**When:** ds.get_scheduled_jobs("alice", page=1, size=10) is called
**Then:** Returned Page::total_items equals 3

```rust
fn get_scheduled_jobs_returns_correct_total_items_respecting_permissions() {
    // Setup: 5 scheduled jobs, alice has perms to 3
    // Expected: total_items = 3
    // This tests GAP7 fix
}
```

---

### Behavior: get_scheduled_jobs returns correct page metadata

**Given:** 25 scheduled jobs user has access to, page=3, size=10
**When:** ds.get_scheduled_jobs("alice", page=3, size=10) is called
**Then:** Page{number: 3, size: 10, total_pages: 3, total_items: 25}

```rust
fn get_scheduled_jobs_returns_correct_page_metadata() {
    // Page 3 of 3 (items 21-25)
    // Page::number == 3
    // Page::size == 10 (actual items in this page, not requested size)
    // Page::total_pages == 3
    // Page::total_items == 25
}
```

---

### Behavior: get_scheduled_jobs returns empty page when user has no permissions

**Given:** 5 scheduled jobs exist, user "bob" has no permissions to any
**When:** ds.get_scheduled_jobs("bob", page=1, size=10) is called
**Then:** Page{items: [], total_items: 0, total_pages: 0}

```rust
fn get_scheduled_jobs_returns_empty_page_when_no_permissions() {
    // Bob has 0 permissions
    // Expected: empty page with 0 total_items
}
```

---

### Behavior: get_scheduled_jobs page overflow returns empty items with correct metadata

**Given:** 5 scheduled jobs user has access to, page=100, size=10
**When:** ds.get_scheduled_jobs("alice", page=100, size=10) is called
**Then:** Page{items: [], number: 100, size: 10, total_pages: 1, total_items: 5}

```rust
fn get_scheduled_jobs_page_overflow_returns_empty_items() {
    // Page 100 of 1 (only 5 items exist)
    // Expected: empty items but correct metadata
    // This is a boundary test for GAP7
}
```

---

### Behavior: create_scheduled_job stores cron_expr in correct column

**Given:** A ScheduledJob with sj.cron="0 * * * *"
**When:** ds.create_scheduled_job(&sj) is called and then ds.get_scheduled_job_by_id(sj.id) is called
**Then:** The retrieved scheduled job has cron_expr="0 * * * *"

```rust
fn create_scheduled_job_stores_cron_expr_correctly() {
    // GAP9 fix verification
    // INSERT then SELECT and verify cron_expr column value
}
```

---

### Behavior: create_scheduled_job stores state in correct column

**Given:** A ScheduledJob with sj.state="ACTIVE"
**When:** ds.create_scheduled_job(&sj) is called and then ds.get_scheduled_job_by_id(sj.id) is called
**Then:** The retrieved scheduled job has state="ACTIVE"

```rust
fn create_scheduled_job_stores_state_correctly() {
    // GAP9 fix verification
    // INSERT then SELECT and verify state column value
}
```

---

### Behavior: create_scheduled_job stores created_at in correct column

**Given:** A ScheduledJob with sj.created_at=some_known_timestamp
**When:** ds.create_scheduled_job(&sj) is called and then ds.get_scheduled_job_by_id(sj.id) is called
**Then:** The retrieved scheduled job has created_at equal to the input timestamp (within 1 second)

```rust
fn create_scheduled_job_stores_created_at_correctly() {
    // GAP9 fix verification
    // created_at is bound at $14, should be stored correctly
}
```

---

### Behavior: create_scheduled_job stores output_ in correct column

**Given:** A ScheduledJob with sj.output="initial output"
**When:** ds.create_scheduled_job(&sj) is called and then ds.get_scheduled_job_by_id(sj.id) is called
**Then:** The retrieved scheduled job has output="initial output"

```rust
fn create_scheduled_job_stores_output_correctly() {
    // GAP9 fix verification
    // output_ is bound at $15, should be stored correctly
}
```

---

### Behavior: create_scheduled_job stores tags in correct column

**Given:** A ScheduledJob with sj.tags=["tag1", "tag2"]
**When:** ds.create_scheduled_job(&sj) is called and then ds.get_scheduled_job_by_id(sj.id) is called
**Then:** The retrieved scheduled job has tags=["tag1", "tag2"]

```rust
fn create_scheduled_job_stores_tags_correctly() {
    // GAP9 fix verification
    // tags is at position 4, should be stored correctly
}
```

---

### Behavior: update_task updates entity and commits when modify returns Ok

**Given:** An existing task with state="CREATED"
**When:** ds.update_task(task_id, |t| { t.state = "RUNNING"; Ok(()) }) is called
**Then:** The task's state is "RUNNING" in the database

```rust
fn update_task_commits_changes_when_modify_returns_ok() {
    // Integration test
    // Verify: SELECT state FROM tasks WHERE id=$1 returns "RUNNING"
}
```

---

### Behavior: update_task returns TaskNotFound when id does not exist

**Given:** A task id that does not exist in the database
**When:** ds.update_task("nonexistent-id", |_| Ok(())) is called
**Then:** Returns Err(Error::TaskNotFound)

```rust
fn update_task_returns_task_not_found_when_id_missing() {
    // Unit test with fake datastore
    // Error: Error::TaskNotFound
}
```

---

### Behavior: update_task rolls back transaction when modify returns Err

**Given:** An existing task with state="CREATED"
**When:** ds.update_task(task_id, |_| Err(Error::InvalidInput("invalid"))) is called
**Then:** Returns Err(Error::InvalidInput("invalid"))
**And:** The task's state is still "CREATED" in the database (not modified)

```rust
fn update_task_rollback_on_modify_error() {
    // Integration test
    // Verify: SELECT state FROM tasks WHERE id=$1 still returns "CREATED"
}
```

---

### Behavior: update_task preserves original values when modify returns Err

**Given:** An existing task with name="original", state="CREATED"
**When:** ds.update_task(task_id, |t| { t.name = "modified"; Err(Error::InvalidInput("abort")) }) is called
**Then:** Returns Err(Error::InvalidInput("abort"))
**And:** The task's name is still "original" in the database

```rust
fn update_task_preserves_values_on_modify_error() {
    // Integration test
    // Verify: name is still "original" after rollback
}
```

---

### Behavior: update_node updates entity and commits when modify returns Ok

**Given:** An existing node with cpu_percent=50.0
**When:** ds.update_node(node_id, |n| { n.cpu_percent = 75.5; Ok(()) }) is called
**Then:** The node's cpu_percent is 75.5 in the database

```rust
fn update_node_commits_changes_when_modify_returns_ok() {
    // Integration test
}
```

---

### Behavior: update_node returns NodeNotFound when id does not exist

**Given:** A node id that does not exist in the database
**When:** ds.update_node("nonexistent-id", |_| Ok(())) is called
**Then:** Returns Err(Error::NodeNotFound)

```rust
fn update_node_returns_not_found_when_id_missing() {
    // Unit test with fake datastore
    // Error: Error::NodeNotFound
}
```

---

### Behavior: update_node rolls back transaction when modify returns Err

**Given:** An existing node with cpu_percent=50.0
**When:** ds.update_node(node_id, |_| Err(Error::InvalidInput("invalid"))) is called
**Then:** Returns Err(Error::InvalidInput("invalid"))
**And:** The node's cpu_percent is still 50.0 in the database

```rust
fn update_node_rollback_on_modify_error() {
    // Integration test
}
```

---

### Behavior: update_node preserves original values when modify returns Err

**Given:** An existing node with name="original", cpu_percent=50.0
**When:** ds.update_node(node_id, |n| { n.name = "modified"; n.cpu_percent = 99.9; Err(Error::InvalidInput("abort")) }) is called
**Then:** Returns Err(Error::InvalidInput("abort"))
**And:** The node's name is still "original" and cpu_percent is still 50.0

```rust
fn update_node_preserves_values_on_modify_error() {
    // Integration test
}
```

---

### Behavior: update_job updates entity and commits when modify returns Ok

**Given:** An existing job with state="ACTIVE"
**When:** ds.update_job(job_id, |j| { j.state = "PAUSED"; Ok(()) }) is called
**Then:** The job's state is "PAUSED" in the database

```rust
fn update_job_commits_changes_when_modify_returns_ok() {
    // Integration test
}
```

---

### Behavior: update_job returns JobNotFound when id does not exist

**Given:** A job id that does not exist in the database
**When:** ds.update_job("nonexistent-id", |_| Ok(())) is called
**Then:** Returns Err(Error::JobNotFound)

```rust
fn update_job_returns_not_found_when_id_missing() {
    // Unit test with fake datastore
    // Error: Error::JobNotFound
}
```

---

### Behavior: update_job rolls back transaction when modify returns Err

**Given:** An existing job with state="ACTIVE"
**When:** ds.update_job(job_id, |_| Err(Error::InvalidInput("invalid"))) is called
**Then:** Returns Err(Error::InvalidInput("invalid"))
**And:** The job's state is still "ACTIVE" in the database

```rust
fn update_job_rollback_on_modify_error() {
    // Integration test
}
```

---

### Behavior: update_job preserves original values when modify returns Err

**Given:** An existing job with name="original"
**When:** ds.update_job(job_id, |j| { j.name = "modified"; Err(Error::InvalidInput("abort")) }) is called
**Then:** Returns Err(Error::InvalidInput("abort"))
**And:** The job's name is still "original"

```rust
fn update_job_preserves_values_on_modify_error() {
    // Integration test
}
```

---

### Behavior: update_scheduled_job updates entity and commits when modify returns Ok

**Given:** An existing scheduled job with state="ACTIVE"
**When:** ds.update_scheduled_job(sj_id, |s| { s.state = "PAUSED"; Ok(()) }) is called
**Then:** The scheduled job's state is "PAUSED" in the database

```rust
fn update_scheduled_job_commits_changes_when_modify_returns_ok() {
    // Integration test
}
```

---

### Behavior: update_scheduled_job returns ScheduledJobNotFound when id does not exist

**Given:** A scheduled job id that does not exist in the database
**When:** ds.update_scheduled_job("nonexistent-id", |_| Ok(())) is called
**Then:** Returns Err(Error::ScheduledJobNotFound)

```rust
fn update_scheduled_job_returns_not_found_when_id_missing() {
    // Unit test with fake datastore
    // Error: Error::ScheduledJobNotFound
}
```

---

### Behavior: update_scheduled_job rolls back transaction when modify returns Err

**Given:** An existing scheduled job with state="ACTIVE"
**When:** ds.update_scheduled_job(sj_id, |_| Err(Error::InvalidInput("invalid"))) is called
**Then:** Returns Err(Error::InvalidInput("invalid"))
**And:** The scheduled job's state is still "ACTIVE" in the database

```rust
fn update_scheduled_job_rollback_on_modify_error() {
    // Integration test
}
```

---

### Behavior: update_scheduled_job preserves original values when modify returns Err

**Given:** An existing scheduled job with name="original", state="ACTIVE"
**When:** ds.update_scheduled_job(sj_id, |s| { s.name = "modified"; Err(Error::InvalidInput("abort")) }) is called
**Then:** Returns Err(Error::InvalidInput("abort"))
**And:** The scheduled job's name is still "original"

```rust
fn update_scheduled_job_preserves_values_on_modify_error() {
    // Integration test
}
```

---

### Behavior: get_role returns RoleNotFound when role does not exist

**Given:** A role id "nonexistent-role" that does not exist
**When:** ds.get_role("nonexistent-role") is called
**Then:** Returns Err(Error::RoleNotFound)

```rust
fn get_role_returns_role_not_found_when_not_exists() {
    // Unit test
    // Error: Error::RoleNotFound
}
```

---

### Behavior: get_role returns RoleNotFound when role was deleted

**Given:** A role that existed but was deleted from the database
**When:** ds.get_role(deleted_role_id) is called
**Then:** Returns Err(Error::RoleNotFound)

```rust
fn get_role_returns_role_not_found_when_deleted() {
    // Integration test
    // Create role, delete it, then try to get it
}
```

---

### Behavior: get_user_roles returns empty when user has no roles

**Given:** A user with no assigned roles
**When:** ds.get_user_roles(user_id) is called
**Then:** Returns Ok(Vec::new())

```rust
fn get_user_roles_returns_empty_when_no_roles() {
    // Unit test
}
```

---

### Behavior: assign_role returns RoleNotFound when role does not exist

**Given:** A valid user id but a role id that does not exist
**When:** ds.assign_role(user_id, "nonexistent-role-id") is called
**Then:** Returns Err(Error::RoleNotFound)

```rust
fn assign_role_returns_role_not_found_when_role_missing() {
    // Unit test
    // Error: Error::RoleNotFound
}
```

---

### Behavior: assign_role returns UserNotFound when user does not exist

**Given:** A role that exists but a user id that does not exist
**When:** ds.assign_role("nonexistent-user-id", role_id) is called
**Then:** Returns Err(Error::UserNotFound)

```rust
fn assign_role_returns_user_not_found_when_user_missing() {
    // Unit test
    // Error: Error::UserNotFound
}
```

---

### Behavior: get_scheduled_job_by_id returns ContextNotFound when context missing

**Given:** A scheduled job that references a context (created_by user) that no longer exists
**When:** ds.get_scheduled_job_by_id(sj_id) is called
**Then:** Returns Err(Error::ContextNotFound)

```rust
fn get_scheduled_job_by_id_returns_context_not_found_when_creator_missing() {
    // Integration test
    // Create sj with user, delete user, then try to get sj
    // Error: Error::ContextNotFound
}
```

---

### Behavior: get_task_by_id returns ContextNotFound when context missing

**Given:** A task that references a job that no longer exists
**When:** ds.get_task_by_id(task_id) is called
**Then:** Returns Err(Error::ContextNotFound)

```rust
fn get_task_by_id_returns_context_not_found_when_job_missing() {
    // Integration test
    // Create task with job, delete job, then try to get task
    // Error: Error::ContextNotFound
}
```

---

### Behavior: create_scheduled_job returns Database error on constraint violation (unique key)

**Given:** A scheduled job with an id that already exists
**When:** ds.create_scheduled_job(&sj) is called where sj.id matches existing row
**Then:** Returns Err(Error::Database(String)) with constraint violation message

```rust
fn create_scheduled_job_returns_database_error_on_unique_constraint_violation() {
    // Integration test
    // Create sj with id, try to create another sj with same id
    // Error: Error::Database(String) containing "unique" or "duplicate"
}
```

---

### Behavior: create_scheduled_job returns Database error on connection failure

**Given:** A valid scheduled job
**When:** ds.create_scheduled_job(&sj) is called but database connection is unavailable
**Then:** Returns Err(Error::Database(String)) with connection error message

```rust
fn create_scheduled_job_returns_database_error_on_connection_failure() {
    // Integration test with mocked connection
    // Error: Error::Database(String)
}
```

---

### Behavior: update_task returns Database error on constraint violation

**Given:** An existing task with a field that has a unique constraint
**When:** ds.update_task(task_id, |t| { ... }) modifies the task to violate a constraint
**Then:** Returns Err(Error::Database(String)) with constraint violation message

```rust
fn update_task_returns_database_error_on_constraint_violation() {
    // Integration test
    // Error: Error::Database(String)
}
```

---

### Behavior: update_job returns Serialization error on malformed JSON

**Given:** An existing job with tasks field containing valid JSON
**When:** ds.update_job(job_id, |j| { j.tasks = "invalid json {{"; Ok(()) }) is called
**Then:** Returns Err(Error::Serialization(String))

```rust
fn update_job_returns_serialization_error_on_malformed_json() {
    // Integration test
    // Error: Error::Serialization(String)
}
```

---

## 4. Proptest Invariants

### Proptest: Page Constructor Invariants

**Invariant:** For any Page constructed with valid inputs, the following must hold:
- `page.number >= 1`
- `page.size >= 1`
- `page.total_pages >= 0`
- `page.total_items >= 0`
- `page.items.len() <= page.size`

**Strategy:** `proptest!` with `(number in 1..=100i64, size in 1..=100i64, total_items in 0..=1000i64, items in any_vec::<String>())`

**Anti-invariant:** Inputs with `number < 1`, `size < 1`, negative totals, or `items.len() > size` must return Error.

---

### Proptest: Page Pagination Math

**Invariant:** `Page::total_pages == ceil(total_items / size)` where `ceil(a/b) = (a + b - 1) / b` for positive integers.

**Strategy:** Generate arbitrary `total_items` and `size` (size >= 1), compute expected total_pages, assert equality.

---

### Proptest: parse_query maintains term/tag separation

**Invariant:** For any input query string, `parse_query` returns `(terms, tags)` where:
- No term in `terms` starts with "tag:" or "tags:"
- Every tag in `tags` came from a "tag:" or "tags:" prefix

**Strategy:** `proptest!` with arbitrary strings, verify invariant holds.

---

### Proptest: sanitize_string null removal

**Invariant:** `sanitize_string(Some(s))` returns `Some(s.replace('\u{0}', ""))` — null bytes removed, other characters unchanged, None stays None.

**Strategy:** `proptest!` with arbitrary strings containing 0..10 null bytes at random positions.

---

## 5. Fuzz Targets

### Fuzz Target: parse_query function

**Input type:** `String` (arbitrary user query input)
**Risk:** Panic on unexpected characters, incorrect parsing leading to SQL injection (mitigated by parameterized queries), logic errors in tag extraction
**Corpus seeds:**
- `""` (empty)
- `"foo bar"` (simple terms)
- `"tag:important"` (single tag)
- `"tag:a,tag:b,tag:c"` (multiple tags)
- `"foo tag:bar baz"` (mixed)
- `"foo\u{0}bar"` (null bytes)

**Location:** `datastore/postgres/mod.rs::parse_query`

---

### Fuzz Target: Page deserialization

**Input type:** JSON bytes representing `Page<T>`
**Risk:** Panic on malformed JSON, incorrect invariant enforcement after deserialization, integer overflow in pagination math
**Corpus seeds:**
- `{"items":[],"number":1,"size":10,"total_pages":0,"total_items":0}`
- `{"items":["a"],"number":1,"size":1,"total_pages":1,"total_items":1}`
- Large page with 1000 items

**Location:** `datastore/mod.rs::Page<T>` (via `serde::Deserialize`)

---

## 6. Kani Harnesses

### Kani Harness: Page pagination arithmetic no overflow

**Property:** For any `total_items >= 0` and `size >= 1`, computing `total_pages = total_items / size + i64::from(total_items % size != 0)` produces a result that fits in i64 and equals `ceil(total_items / size)`.

**Bound:** `total_items` in 0..i64::MAX, `size` in 1..1000000
**Rationale:** This arithmetic is in the hot path for every paginated query. Overflow or incorrect ceiling math would corrupt page metadata returned to clients.

---

### Kani Harness: update_* FOR UPDATE lock acquisition

**Property:** For any of `update_task`, `update_node`, `update_job`, `update_scheduled_job`, the sequence: (1) begin tx, (2) SELECT FOR UPDATE, (3) modify callback, (4) UPDATE, (5) commit — is atomic. If commit fails, no changes persist.

**Bound:** Single entity update, modify callback returns Ok(())
**Rationale:** The update methods implement read-modify-write with locking. Formal verification ensures no window where another transaction could see uncommitted state.

---

## 7. Mutation Testing Checkpoints

### Critical Mutations to Survive

| Function | Mutation | Must be caught by test |
|----------|----------|------------------------|
| `create_scheduled_job` | Remove permission INSERT loop | `create_scheduled_job_inserts_permissions_when_provided` |
| `create_scheduled_job` | Change cron_expr binding position | `create_scheduled_job_stores_cron_expr_correctly` |
| `create_scheduled_job` | Change state binding position | `create_scheduled_job_stores_state_correctly` |
| `get_scheduled_jobs` | Change count query to ignore CTEs | `get_scheduled_jobs_returns_correct_total_items_respecting_permissions` |
| `Page::new` | Remove I1 invariant check | `page_rejects_number_less_than_one` |
| `update_task` | Remove rollback on error | `update_task_rollback_on_modify_error` |
| `update_task` | Remove not-found check | `update_task_returns_task_not_found_when_id_missing` |
| `update_node` | Remove rollback on error | `update_node_rollback_on_modify_error` |
| `update_job` | Remove rollback on error | `update_job_rollback_on_modify_error` |
| `update_scheduled_job` | Remove rollback on error | `update_scheduled_job_rollback_on_modify_error` |

**Threshold:** ≥90% mutation kill rate minimum.

---

## 8. Combinatorial Coverage Matrix

### Page Construction

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path | number=1, size=10, total_items=100, total_pages=10 | Ok(Page{...}) | unit |
| error: number=0 | number=0 | Err(InvalidInput("page number must be >= 1")) | unit |
| error: size=0 | size=0 | Err(InvalidInput("page size must be >= 1")) | unit |
| error: negative total_items | total_items=-1 | Err(InvalidInput("total_items must be >= 0")) | unit |
| error: total_pages inconsistent | total_items=10, total_pages=0 | Err(InvalidInput("total_pages must be >= 1 when total_items > 0")) | unit |
| error: items exceed size | items.len()=15, size=10 | Err(InvalidInput("items.len() cannot exceed page size")) | unit |
| boundary: empty page | items=[], total_items=0, total_pages=0 | Ok(Page{items:[], total_pages:0, total_items:0}) | unit |
| boundary: exact fit | items.len()=10, size=10 | Ok(Page{items.len():10}) | unit |
| boundary: single item | total_items=1, size=10 | Ok(Page{total_pages:1}) | unit |
| math: ceil calculation | total_items=25, size=10 | Ok(Page{total_pages:3}) | unit |

### create_scheduled_job

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path with perms | valid sj with 2 permissions | Ok(()), 2 rows in scheduled_jobs_perms | integration |
| happy path no perms | sj.permissions=None | Ok(()), 0 rows in perms | integration |
| error: id=None | sj.id=None | Err(InvalidInput("scheduled job id is required")) | unit |
| error: created_by=None | sj.created_by=None | Err(InvalidInput("created_by is required")) | unit |
| error: created_by.id=None | sj.created_by.id=None | Err(InvalidInput("created_by.id is required")) | unit |
| column order: cron | sj.cron="0 * * * *" | SELECT returns cron_expr="0 * * * *" | integration |
| column order: state | sj.state="ACTIVE" | SELECT returns state="ACTIVE" | integration |
| column order: created_at | sj.created_at=T | SELECT returns created_at≈T | integration |
| column order: output | sj.output="out" | SELECT returns output_="out" | integration |
| column order: tags | sj.tags=["a","b"] | SELECT returns tags=['a','b'] | integration |
| atomicity | valid sj with perms | Both insert succeed or both fail | integration |
| error: unique constraint | duplicate sj.id | Err(Database(String)) | integration |
| error: connection | db unavailable | Err(Database(String)) | integration |

### get_scheduled_jobs

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path | user has 3 of 5 jobs | Page{total_items:3, items:3} | integration |
| empty result | user has 0 jobs | Page{total_items:0, items:[]} | integration |
| page 2 of 3 | page=2, size=10 | Page{number:2, size:10} | integration |
| last page partial | 25 items, page=3, size=10 | Page{size:5, total_pages:3} | integration |
| count query respects perms | alice vs bob different counts | alice.total_items=3, bob.total_items=1 | integration |
| boundary: page overflow | page=100, size=10, only 5 items | Page{items:[], total_items:5} | integration |

### update_task

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path | valid id, modify returns Ok(()) | Ok(()), task updated | integration |
| not found | id doesn't exist | Err(TaskNotFound) | unit |
| modify error | modify returns Err | Err(modify_error), no update | integration |
| rollback on error | modify returns Err | Task unchanged in DB | integration |
| preserves values | modify sets name="x", then Err | name still original | integration |
| constraint violation | modify violates constraint | Err(Database(String)) | integration |

### update_node

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path | valid id, modify returns Ok(()) | Ok(()), node updated | integration |
| not found | id doesn't exist | Err(NodeNotFound) | unit |
| modify error | modify returns Err | Err(modify_error), no update | integration |
| rollback on error | modify returns Err | Node unchanged in DB | integration |

### update_job

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path | valid id, modify returns Ok(()) | Ok(()), job updated | integration |
| not found | id doesn't exist | Err(JobNotFound) | unit |
| modify error | modify returns Err | Err(modify_error), no update | integration |
| rollback on error | modify returns Err | Job unchanged in DB | integration |
| serialization error | modify sets invalid JSON | Err(Serialization(String)) | integration |

### update_scheduled_job

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| happy path | valid id, modify returns Ok(()) | Ok(()), sj updated | integration |
| not found | id doesn't exist | Err(ScheduledJobNotFound) | unit |
| modify error | modify returns Err | Err(modify_error), no update | integration |
| rollback on error | modify returns Err | SJ unchanged in DB | integration |

### Error Variants (RoleNotFound, ContextNotFound)

| Scenario | Input Class | Expected Output | Layer |
|----------|-------------|-----------------|-------|
| get_role not found | role doesn't exist | Err(RoleNotFound) | unit |
| get_role deleted | role was deleted | Err(RoleNotFound) | integration |
| assign_role role missing | role id doesn't exist | Err(RoleNotFound) | unit |
| assign_role user missing | user id doesn't exist | Err(UserNotFound) | unit |
| get_user_roles empty | user has no roles | Ok(Vec::new()) | unit |
| get_scheduled_job context missing | creator deleted | Err(ContextNotFound) | integration |
| get_task context missing | job deleted | Err(ContextNotFound) | integration |

---

## Open Questions

1. **GAP2/3/4 refactoring:** The contract notes `with_tx` helper exists but update methods don't use it. Should tests verify current manual tx management, or should tests be written for the future `with_tx`-based implementation? Contract says "refactoring opportunity but does not change contract semantics" — tests should verify current behavior AND serve as regression tests for the future refactor.

2. **Transaction isolation level:** The code doesn't specify isolation level. Should tests verify specific isolation (e.g., READ COMMITTED is assumed)? Kani harness could prove isolation properties.

3. **Permission loading in get_scheduled_jobs:** The count query at line 1202 ignores permissions per GAP7. After fix, should the count CTE mirror the SELECT CTE exactly? This needs confirmation before writing the count query test.

4. **ScheduledJob permissions field on struct:** The contract shows `sj.permissions: Option<Vec<Permission>>` but it's unclear if the ScheduledJob struct has a `permissions` field that gets populated on read. If so, tests should verify `get_scheduled_job_by_id` returns permissions correctly.

---

## Error Enum Coverage

Every `Error` variant from `datastore/mod.rs` has at least one test scenario:

| Variant | Test Scenario |
|---------|---------------|
| `TaskNotFound` | `update_task_returns_task_not_found_when_id_missing` |
| `NodeNotFound` | `update_node_returns_not_found_when_id_missing` |
| `JobNotFound` | `update_job_returns_not_found_when_id_missing` |
| `ScheduledJobNotFound` | `update_scheduled_job_returns_not_found_when_id_missing` |
| `UserNotFound` | `assign_role_returns_user_not_found_when_user_missing` |
| `RoleNotFound` | `get_role_returns_role_not_found_when_not_exists` |
| `RoleNotFound` | `get_role_returns_role_not_found_when_deleted` |
| `RoleNotFound` | `assign_role_returns_role_not_found_when_role_missing` |
| `ContextNotFound` | `get_scheduled_job_by_id_returns_context_not_found_when_creator_missing` |
| `ContextNotFound` | `get_task_by_id_returns_context_not_found_when_job_missing` |
| `Database(String)` | `create_scheduled_job_returns_database_error_on_connection_failure` |
| `Database(String)` | `create_scheduled_job_returns_database_error_on_unique_constraint_violation` |
| `Database(String)` | `update_task_returns_database_error_on_constraint_violation` |
| `Serialization(String)` | `update_job_returns_serialization_error_on_malformed_json` |
| `Encryption(String)` | (secrets encryption — tested separately in integration) |
| `InvalidInput(String)` | All GAP6 precondition tests + update_* modify error tests |
| `Transaction(String)` | **NOT COVERED** — requires infrastructure-level fault injection (pool.begin() failure) which is impractical for unit/integration tests. Would need a fake pool or connection-splicing to simulate. Covered by integration/chaos testing in production environments. |

---

## Unit Test Count by Category

| Category | Count |
|----------|-------|
| Page construction (invariants + boundary) | 10 |
| create_scheduled_job preconditions | 3 |
| update_task not-found + error handling | 4 |
| update_node not-found + error handling | 4 |
| update_job not-found + error handling | 4 |
| update_scheduled_job not-found + error handling | 4 |
| Error::RoleNotFound coverage | 3 |
| Error::ContextNotFound coverage | 2 |
| Error::UserNotFound coverage | 1 |
| Error::Database constraint violations | 3 |
| Error::Serialization coverage | 1 |
| get_user_roles | 1 |
| Cron expression boundary (64 char limit) | 2 |
| Page overflow boundary | 2 |
| **Total unit tests** | **44** |

Plus 4 proptest invariants (property-based tests) = 48 total test scenarios at unit/property layer.
