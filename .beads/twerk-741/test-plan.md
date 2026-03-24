bead_id: twerk-741
bead_title: Fix middleware gaps: context types, expr evaluation, webhook events
phase: 1.5
updated_at: 2026-03-24T13:00:00Z

# Test Plan: Fix middleware gaps

## Summary

- Behaviors identified: 57 (inventory items)
- BDD scenarios: 63 (individual test cases)
- Trophy allocation: 38 unit / 15 integration / 4 e2e = 57 tests (some scenarios overlap layers)
- Proptest invariants: 8
- Fuzz targets: 4
- Kani harnesses: 2
- Mutation kill threshold: ≥90%
- **Density: 63 scenarios / 11 public functions = 5.73x (target ≥5x) ✓**

## 1. Behavior Inventory

### GAP 1: Task Context Type (`middleware/task/mod.rs`)

1. **Context::cancelled() returns Arc<Context>** when called with no arguments
2. **Context::cancelled() returns context where is_cancelled() is true**
3. **Context::cancelled() returns context where is_deadline_exceeded() is false**
4. **Context::deadline_exceeded() returns Arc<Context>** when called with no arguments
5. **Context::deadline_exceeded() returns context where is_deadline_exceeded() is true**
6. **Context::deadline_exceeded() returns context where is_cancelled() is false**
7. **Context::with_value chains new key-value** onto existing context
8. **Context::with_value returns new Arc<Context>** preserving original
9. **Context::get returns Some(value) when key exists**
10. **Context::get returns None when key does not exist**
11. **Context::get returns None when called on Cancelled context**
12. **Context::get returns None when called on DeadlineExceeded context**
13. **is_cancelled returns true for Context::Cancelled variant**
14. **is_cancelled returns false for Context::Values variant**
15. **is_cancelled returns false for Context::DeadlineExceeded variant**
16. **is_deadline_exceeded returns true for Context::DeadlineExceeded variant**
17. **is_deadline_exceeded returns false for Context::Cancelled variant**
18. **is_deadline_exceeded returns false for Context::Values variant**
19. **Context::Values variant contains HashMap with correct key-value pairs**
20. **Context::Values with empty key is rejected or handled gracefully**
21. **Context::Values with empty value is rejected or handled gracefully**
22. **Context::Values with special characters in key works correctly**

### GAP 5: Task Webhook get_job (`middleware/task/webhook.rs`)

23. **get_job returns Ok(Job) when ctx is valid Values and job exists**
24. **get_job returns Err(TaskMiddlewareError::JobNotFound) when job does not exist**
25. **get_job returns Err(TaskMiddlewareError::ContextCancelled) when ctx is Cancelled**
26. **get_job returns Err(TaskMiddlewareError::ContextDeadlineExceeded) when ctx is DeadlineExceeded**
27. **get_job returns Err(TaskMiddlewareError::ContextCancelled) before any I/O** (context check precedes datastore)
28. **get_job returns Err(TaskMiddlewareError::ContextDeadlineExceeded) before any I/O**
29. **get_job with empty job_id returns appropriate error**
30. **get_job with cache hit returns cached Job without datastore call**
31. **get_job with cache miss fetches from datastore and caches result**
32. **Datastore::get_job_by_id returns Ok(Job) when job exists**
33. **Datastore::get_job_by_id returns Err(TaskMiddlewareError::JobNotFound) when job not found**
34. **Datastore::get_job_by_id returns Err(TaskMiddlewareError::Datastore) on datastore error**

### GAP 8: Webhook Event Matching (`middleware/job/webhook.rs`)

35. **should_fire_webhook StateChange + None event returns true**
36. **should_fire_webhook StateChange + Some("") event returns true**
37. **should_fire_webhook StateChange + EVENT_DEFAULT returns true**
38. **should_fire_webhook StateChange + EVENT_STATE_CHANGE returns true**
39. **should_fire_webhook StateChange + EVENT_PROGRESS returns false**
40. **should_fire_webhook Progress + None event returns false** (GAP 8 fix)
41. **should_fire_webhook Progress + Some("") event returns false** (GAP 8 fix)
42. **should_fire_webhook Progress + EVENT_DEFAULT returns false** (GAP 8 fix)
43. **should_fire_webhook Progress + EVENT_PROGRESS returns true**
44. **should_fire_webhook Read + any event returns false**
45. **should_fire_webhook StateChange + task.* event returns false**

### GAP 3 & 4: Expression Evaluation (`eval/mod.rs`)

46. **evaluate_condition returns Ok(true) when expression evaluates to true**
47. **evaluate_condition returns Ok(false) when expression evaluates to false**
48. **evaluate_condition returns Err(EvalError::InvalidSyntax) for malformed expression**
49. **evaluate_condition returns Err(EvalError::NotBoolean) when expr returns non-boolean**
50. **evaluate_condition returns Err(EvalError::EvaluationFailed) on evalexpr error**
51. **evaluate_condition includes job_state in context**
52. **evaluate_condition includes job_id in context**
53. **evaluate_task_condition returns Ok(true) when expression evaluates to true**
54. **evaluate_task_condition returns Ok(false) when expression evaluates to false**
55. **evaluate_task_condition returns Err variant for invalid expression**
56. **evaluate_task_condition includes task and job keys in context**
57. **evaluate_template passthrough returns template unchanged when no {{ }} patterns**

## 2. Trophy Allocation

| Behavior | Layer | Justification |
|----------|-------|---------------|
| Context constructors (cancelled, deadline_exceeded, with_value) | Unit | Pure functions, deterministic construction |
| Context observers (is_cancelled, is_deadline_exceeded, get) | Unit | Pure state queries |
| Context edge cases (empty key/value, special chars) | Unit | Boundary value testing |
| get_job context propagation (Cancelled, DeadlineExceeded) | Unit | Branch coverage, context check before I/O |
| Datastore trait method (get_job_by_id) | Unit | Pure interface, mock/fake in tests |
| Error variant exact typing (all 5 variants) | Unit | Enum exhaustive coverage |
| evaluate_condition standalone (true/false/error paths) | Unit | Pure calc, multiple return paths |
| evaluate_task_condition standalone (true/false/error paths) | Unit | Pure calc, multiple return paths |
| should_fire_webhook truth table | Unit | 10 combinations, deterministic |
| evaluate_template variants | Unit | Pure string transformation |
| Middleware chain ordering | Integration | Component wiring with real deps |
| get_job cache hit/miss integration | Integration | Cache + datastore interaction |
| Webhook event filtering in context | Integration | evalexpr runtime integration |
| End-to-end webhook dispatch | E2E | Full async HTTP flow |

**Ratio: ~67% unit / 26% integration / 7% e2e**

Rationale: The contract specifies 11 public functions across context types, datastore trait, and expression evaluation. Unit tests provide exhaustive combinatorial coverage for all error variants and boundary conditions. Integration tests verify component wiring with real dependencies.

## 3. BDD Scenarios

### GAP 1: Context Type

---

### Behavior: Context::cancelled returns Arc<Context> where is_cancelled is true
Given: Nothing (no prior state required)
When: Calling `Context::cancelled()`
Then: Returns `Arc<Context>` where `is_cancelled()` returns `true`
And: `is_deadline_exceeded()` returns `false`

---

### Behavior: Context::deadline_exceeded returns Arc<Context> where is_deadline_exceeded is true
Given: Nothing
When: Calling `Context::deadline_exceeded()`
Then: Returns `Arc<Context>` where `is_deadline_exceeded()` returns `true`
And: `is_cancelled()` returns `false`

---

### Behavior: Context::with_value chains new key-value onto Values context
Given: A context created via `Context::with_value("key1", "value1")`
When: Calling `with_value("key2", "value2")` on the returned Arc
Then: Returns new Arc<Context> where `get("key1")` returns `Some("value1")`
And: `get("key2")` returns `Some("value2")`

---

### Behavior: Context::get returns Some when key exists in Values context
Given: A Context created via `Context::with_value("key", "value")`
When: Calling `context.get("key")`
Then: Returns `Some("value")`

---

### Behavior: Context::get returns None when key does not exist
Given: A Context created via `Context::with_value("other_key", "value")`
When: Calling `context.get("nonexistent")`
Then: Returns `None`

---

### Behavior: Context::get returns None when called on Cancelled context
Given: A Context via `Context::cancelled()`
When: Calling `context.get("any_key")`
Then: Returns `None`

---

### Behavior: Context::get returns None when called on DeadlineExceeded context
Given: A Context via `Context::deadline_exceeded()`
When: Calling `context.get("any_key")`
Then: Returns `None`

---

### Behavior: is_cancelled returns true for Context::Cancelled variant
Given: A Context via `Context::cancelled()`
When: Calling `context.is_cancelled()`
Then: Returns `true`

---

### Behavior: is_cancelled returns false for Context::Values variant
Given: A Context via `Context::with_value("key", "value")`
When: Calling `context.is_cancelled()`
Then: Returns `false`

---

### Behavior: is_cancelled returns false for Context::DeadlineExceeded variant
Given: A Context via `Context::deadline_exceeded()`
When: Calling `context.is_cancelled()`
Then: Returns `false`

---

### Behavior: is_deadline_exceeded returns true for Context::DeadlineExceeded variant
Given: A Context via `Context::deadline_exceeded()`
When: Calling `context.is_deadline_exceeded()`
Then: Returns `true`

---

### Behavior: is_deadline_exceeded returns false for Context::Cancelled variant
Given: A Context via `Context::cancelled()`
When: Calling `context.is_deadline_exceeded()`
Then: Returns `false`

---

### Behavior: is_deadline_exceeded returns false for Context::Values variant
Given: A Context via `Context::with_value("key", "value")`
When: Calling `context.is_deadline_exceeded()`
Then: Returns `false`

---

### Behavior: Context::Values variant stores HashMap with key-value pairs
Given: A Context created via `Context::with_value("name", "test")` then `.with_value("id", "123")`
When: Inspecting the context values
Then: Both key-value pairs are accessible via `get()`

---

### Behavior: Context::with_value handles empty string key
Given: A Context via `Context::with_value("key", "value")`
When: Calling `with_value("", "empty_key_value")`
Then: Behavior is documented (either rejected or accepted; test asserts documented behavior)

---

### Behavior: Context::with_value handles empty string value
Given: A Context via `Context::with_value("key", "value")`
When: Calling `with_value("empty_val_key", "")`
Then: Behavior is documented (either rejected or accepted; test asserts documented behavior)

---

### Behavior: Context::with_value handles special characters in key
Given: A Context via `Context::with_value("key", "value")`
When: Calling `with_value("key with spaces", "value")`
Then: `get("key with spaces")` returns `Some("value")`

---

### GAP 5: Task Webhook get_job

---

### Behavior: get_job returns Ok(Job) when ctx is valid Values and job exists
Given: A valid Context via `Context::with_value("key", "value")`
And: A job_id "job-123" that exists in the datastore
And: A cache (may or may not have the job)
When: Calling `get_job(ctx, "job-123", ds, cache)`
Then: Returns `Ok(job)` where `job.id == "job-123"`

---

### Behavior: get_job returns Err(ContextCancelled) before any I/O when ctx is Cancelled
Given: A Context via `Context::cancelled()`
And: A valid job_id "job-123"
And: A datastore that would return the job
When: Calling `get_job(ctx, "job-123", ds, cache)`
Then: Returns `Err(TaskMiddlewareError::ContextCancelled)`
And: Datastore is never called (no I/O occurs)

---

### Behavior: get_job returns Err(ContextDeadlineExceeded) before any I/O when ctx is DeadlineExceeded
Given: A Context via `Context::deadline_exceeded()`
And: A valid job_id "job-123"
And: A datastore that would return the job
When: Calling `get_job(ctx, "job-123", ds, cache)`
Then: Returns `Err(TaskMiddlewareError::ContextDeadlineExceeded)`
And: Datastore is never called

---

### Behavior: get_job returns Err(JobNotFound) when job does not exist
Given: A valid Context via `Context::with_value("key", "value")`
And: A job_id "nonexistent" that does not exist in the datastore
When: Calling `get_job(ctx, "nonexistent", ds, cache)`
Then: Returns `Err(TaskMiddlewareError::JobNotFound("nonexistent".to_string()))`

---

### Behavior: get_job returns Err(Datastore) when datastore errors
Given: A valid Context via `Context::with_value("key", "value")`
And: A datastore that returns an error
When: Calling `get_job(ctx, "job-123", ds, cache)`
Then: Returns `Err(TaskMiddlewareError::Datastore(_))`

---

### Behavior: get_job with empty job_id returns appropriate error
Given: A valid Context via `Context::with_value("key", "value")`
And: Empty string job_id ""
When: Calling `get_job(ctx, "", ds, cache)`
Then: Returns an error (specific variant per contract)

---

### Behavior: get_job with cache hit returns cached Job without datastore call
Given: A valid Context via `Context::with_value("key", "value")`
And: A cache that already contains job "job-123"
And: A datastore that would return a different job
When: Calling `get_job(ctx, "job-123", ds, cache)`
Then: Returns `Ok(cached_job)`
And: Datastore is never called

---

### Behavior: get_job with cache miss fetches from datastore and caches result
Given: A valid Context via `Context::with_value("key", "value")`
And: A cache that does not contain job "job-123"
And: A datastore that returns job "job-123"
When: Calling `get_job(ctx, "job-123", ds, cache)`
Then: Returns `Ok(job)`
And: Job is now in the cache

---

### Behavior: Datastore::get_job_by_id returns Ok(Job) when job exists
Given: A valid ctx Context via `Context::with_value("key", "value")`
And: A datastore that contains job with id "job-123"
When: Calling `ds.get_job_by_id(ctx, "job-123")`
Then: Returns `Ok(job)` where `job.id == "job-123"`

---

### Behavior: Datastore::get_job_by_id returns Err(JobNotFound) when job not found
Given: A valid ctx Context via `Context::with_value("key", "value")`
And: A datastore that does not contain the job
When: Calling `ds.get_job_by_id(ctx, "nonexistent")`
Then: Returns `Err(TaskMiddlewareError::JobNotFound("nonexistent".to_string()))`

---

### Behavior: Datastore::get_job_by_id returns Err(Datastore) on datastore error
Given: A valid ctx Context via `Context::with_value("key", "value")`
And: A datastore that errors (e.g., connection failure)
When: Calling `ds.get_job_by_id(ctx, "job-123")`
Then: Returns `Err(TaskMiddlewareError::Datastore(_))`

---

### GAP 8: Webhook Event Matching

---

### Behavior: should_fire_webhook StateChange + None event returns true
Given: EventType::StateChange and Webhook with event=None
When: Calling `should_fire_webhook(EventType::StateChange, &wh)`
Then: Returns `true`

---

### Behavior: should_fire_webhook StateChange + empty string event returns true
Given: EventType::StateChange and Webhook with event=Some("")
When: Calling `should_fire_webhook(EventType::StateChange, &wh)`
Then: Returns `true`

---

### Behavior: should_fire_webhook StateChange + EVENT_DEFAULT returns true
Given: EventType::StateChange and Webhook with event=Some(EVENT_DEFAULT)
When: Calling `should_fire_webhook(EventType::StateChange, &wh)`
Then: Returns `true`

---

### Behavior: should_fire_webhook StateChange + EVENT_STATE_CHANGE returns true
Given: EventType::StateChange and Webhook with event=Some("job.StateChange")
When: Calling `should_fire_webhook(EventType::StateChange, &wh)`
Then: Returns `true`

---

### Behavior: should_fire_webhook StateChange + EVENT_PROGRESS returns false
Given: EventType::StateChange and Webhook with event=Some("job.Progress")
When: Calling `should_fire_webhook(EventType::StateChange, &wh)`
Then: Returns `false`

---

### Behavior: should_fire_webhook Progress + None event returns false (GAP 8 fix)
Given: EventType::Progress and Webhook with event=None
When: Calling `should_fire_webhook(EventType::Progress, &wh)`
Then: Returns `false`

---

### Behavior: should_fire_webhook Progress + empty string event returns false (GAP 8 fix)
Given: EventType::Progress and Webhook with event=Some("")
When: Calling `should_fire_webhook(EventType::Progress, &wh)`
Then: Returns `false`

---

### Behavior: should_fire_webhook Progress + EVENT_DEFAULT returns false (GAP 8 fix)
Given: EventType::Progress and Webhook with event=Some(EVENT_DEFAULT)
When: Calling `should_fire_webhook(EventType::Progress, &wh)`
Then: Returns `false`

---

### Behavior: should_fire_webhook Progress + EVENT_PROGRESS returns true
Given: EventType::Progress and Webhook with event=Some("job.Progress")
When: Calling `should_fire_webhook(EventType::Progress, &wh)`
Then: Returns `true`

---

### Behavior: should_fire_webhook Read + any event returns false
Given: EventType::Read and any Webhook configuration (None, Some(""), Some("job.StateChange"), Some("job.Progress"))
When: Calling `should_fire_webhook(EventType::Read, &wh)`
Then: Returns `false` for all configurations

---

### Behavior: should_fire_webhook StateChange + task.* event returns false
Given: EventType::StateChange and Webhook with event=Some("task.StateChange")
When: Calling `should_fire_webhook(EventType::StateChange, &wh)`
Then: Returns `false`

---

### GAP 3 & 4: Expression Evaluation

---

### Behavior: evaluate_condition returns Ok(true) when expression evaluates to true
Given: Expression `"true"` and a JobSummary with state="PENDING"
When: Calling `evaluate_condition("true", &summary)`
Then: Returns `Ok(true)`

---

### Behavior: evaluate_condition returns Ok(false) when expression evaluates to false
Given: Expression `"false"` and a JobSummary
When: Calling `evaluate_condition("false", &summary)`
Then: Returns `Ok(false)`

---

### Behavior: evaluate_condition returns Err(InvalidSyntax) for malformed expression
Given: Expression `"1 +"` (incomplete) and a JobSummary
When: Calling `evaluate_condition("1 +", &summary)`
Then: Returns `Err(EvalError::InvalidSyntax(_))`

---

### Behavior: evaluate_condition returns Err(NotBoolean) when expression returns non-boolean
Given: Expression `"1 + 1"` (evaluates to number) and a JobSummary
When: Calling `evaluate_condition("1 + 1", &summary)`
Then: Returns `Err(EvalError::NotBoolean(_))`

---

### Behavior: evaluate_condition returns Err(EvaluationFailed) on evalexpr error
Given: Expression that causes evalexpr to fail and a JobSummary
When: Calling `evaluate_condition`
Then: Returns `Err(EvalError::EvaluationFailed(_))`

---

### Behavior: evaluate_condition includes job_state in context
Given: Expression `"job_state == \"COMPLETED\""` and JobSummary with state="COMPLETED"
When: Calling `evaluate_condition(expr, &summary)`
Then: Returns `Ok(true)`

---

### Behavior: evaluate_condition includes job_id in context
Given: Expression `"job_id == \"test-123\""` and JobSummary with id Some("test-123")
When: Calling `evaluate_condition(expr, &summary)`
Then: Returns `Ok(true)`

---

### Behavior: evaluate_task_condition returns Ok(true) when expression evaluates to true
Given: Expression `"true"` and TaskSummary + JobSummary
When: Calling `evaluate_task_condition("true", &task_summary, &job_summary)`
Then: Returns `Ok(true)`

---

### Behavior: evaluate_task_condition returns Ok(false) when expression evaluates to false
Given: Expression `"false"` and TaskSummary + JobSummary
When: Calling `evaluate_task_condition("false", &task_summary, &job_summary)`
Then: Returns `Ok(false)`

---

### Behavior: evaluate_task_condition returns Err variant for invalid expression
Given: Expression `"{{invalid"` and TaskSummary + JobSummary
When: Calling `evaluate_task_condition(expr, &task_summary, &job_summary)`
Then: Returns `Err(EvalError::InvalidSyntax(_))` or `Err(EvalError::EvaluationFailed(_))`

---

### Behavior: evaluate_task_condition includes task and job keys in context
Given: Expression `"task.state == \"PENDING\""` and TaskSummary with state="PENDING"
When: Calling `evaluate_task_condition(expr, &task_summary, &job_summary)`
Then: Returns `Ok(true)` (depends on Option A flat context or Option B JSON object access)

---

### Behavior: evaluate_template returns template unchanged when no expressions
Given: Template string `"hello world"` with empty context
When: Calling `evaluate_template("hello world", &context)`
Then: Returns `"hello world"`

---

### Behavior: evaluate_template replaces single expression
Given: Template `"hello {{name}}"` with context `{"name": "world"}`
When: Calling `evaluate_template("hello {{name}}", &context)`
Then: Returns `"hello world"`

---

### Behavior: evaluate_template replaces multiple expressions
Given: Template `"{{greeting}} {{target}}!"` with context `{"greeting": "hello", "target": "world"}`
When: Calling `evaluate_template`
Then: Returns `"hello world!"`

---

### Behavior: evaluate_template preserves text around expressions
Given: Template `"result: {{1 + 2}} done"`
When: Calling `evaluate_template`
Then: Returns `"result: 3 done"`

---

### Behavior: evaluate_template handles empty string input
Given: Empty template string `""`
When: Calling `evaluate_template("", &context)`
Then: Returns `""`

---

### Behavior: sanitize_expr strips mustache braces
Given: Expression string `"{{ 1 + 1 }}"`
When: Calling `sanitize_expr`
Then: Returns `"1 + 1"`

---

### Behavior: sanitize_expr passthrough for plain text
Given: Expression string `"1 + 1"` (no braces)
When: Calling `sanitize_expr`
Then: Returns `"1 + 1"`

---

### Behavior: sanitize_expr handles empty braces
Given: Expression string `"{{}}"`
When: Calling `sanitize_expr`
Then: Returns `""`

---

### Behavior: transform_operators converts boolean keywords
Given: Expression string `"true and false"`
When: Calling `transform_operators`
Then: Returns `"true && false"`

---

### Behavior: transform_operators converts or keyword
Given: Expression string `"true or false"`
When: Calling `transform_operators`
Then: Returns `"true || false"`

---

### Behavior: valid_expr rejects empty string
Given: Empty string `""`
When: Calling `valid_expr`
Then: Returns `false`

---

### Behavior: valid_expr rejects empty template
Given: String `"{{}}"`
When: Calling `valid_expr`
Then: Returns `false`

---

### Behavior: valid_expr accepts valid expressions
Given: String `"1 == 1"` and `"true and false"` and `"randomInt()"`
When: Calling `valid_expr`
Then: Returns `true` for each

---

## 4. Proptest Invariants

### Proptest: evaluate_condition is deterministic
Invariant: Calling `evaluate_condition(expr, &summary)` multiple times with the same inputs always returns the same result
Strategy: Generate random valid expressions and summaries, call twice, compare results

---

### Proptest: evaluate_task_condition is deterministic
Invariant: Calling `evaluate_task_condition(expr, &task_summary, &job_summary)` multiple times with the same inputs always returns the same result
Strategy: Generate random valid expressions and summaries, call twice, compare results

---

### Proptest: Context::with_value preserves all original values
Invariant: For any context with existing values, after calling `with_value(k, v)`, all original key-value pairs remain accessible via `get()`
Strategy: Generate context with 1-10 values, add new value, verify all original values via `get()`

---

### Proptest: evaluate_template idempotent on non-template text
Invariant: Calling `evaluate_template` on a string containing no `{{` characters returns the original string unchanged
Strategy: Generate string not containing "{{", call evaluate_template, assert equality

---

### Proptest: valid_expr consistency with evaluate_expr
Invariant: `valid_expr(e) == true` iff `evaluate_expr(e, &any_context)` returns `Ok(_)` (does not error)
Strategy: Generate random expressions, compare valid_expr result with evaluate_expr success

---

### Proptest: Context::get returns None for non-existent keys across all context variants
Invariant: For any Context (Cancelled, DeadlineExceeded, Values with N pairs), calling `get("nonexistent-key")` returns `None`
Strategy: Generate random context states and arbitrary non-existent key, verify `get()` returns `None`

---

### Proptest: sanitize_expr never adds characters
Invariant: `sanitize_expr(s).len() <= s.len()` for all inputs
Strategy: Generate random strings, verify output length constraint

---

### Proptest: should_fire_webhook Read always returns false
Invariant: `should_fire_webhook(EventType::Read, &wh)` returns `false` for any webhook configuration
Strategy: Generate arbitrary webhook configurations, verify always false

---

## 5. Fuzz Targets

### Fuzz Target: evaluate_template with random template strings
Input type: `String` (arbitrary template text)
Risk: Panic on malformed regex, logic error in template replacement, catastrophic backtracking
Corpus seeds:
- `""` (empty)
- `"plain text"` (no templates)
- `"{{1 + 1}}"` (simple expression)
- `"{{}}"` (empty expression - edge case)
- `"text {{expr}} more text"` (mixed)
- `"{{a}}{{b}}{{c}}"` (multiple consecutive)
- `"{{"` (incomplete mustache - tests regex)
- `"}}"` (trailing closing)
- `"}}{{"` (swapped)
- Very long strings for performance testing

---

### Fuzz Target: evaluate_expr with random expression strings
Input type: `String` (arbitrary expression)
Risk: Panic on malformed evalexpr input, infinite loop, memory explosion
Corpus seeds:
- `""` (empty)
- `"true"`
- `"1 + 2 * 3"`
- `"randomInt()"`
- `"sequence(1, 5)"`
- `"{{invalid syntax}}"` (will be sanitized)
- `"--"` (invalid operator)
- Unbalanced parentheses
- Very long expressions

---

### Fuzz Target: evaluate_condition with random expression + summary pairs
Input type: `(String, JobSummary)` (arbitrary expression + summary)
Risk: Context key mismatches, type errors in evalexpr, missing job_state/job_id keys
Corpus seeds:
- `("true", summary with any state)`
- `("job_state == \"COMPLETED\"", summary with state COMPLETED)`
- `("job_id == \"\"", summary with empty id)`
- `("invalid_var == \"value\"", summary)` (missing key)

---

### Fuzz Target: evaluate_task_condition with random expression + summaries
Input type: `(String, TaskSummary, JobSummary)` (arbitrary expression + summaries)
Risk: Missing task/job keys, type errors, evalexpr panics
Corpus seeds:
- `("true", task_summary, job_summary)`
- `("task.state == \"PENDING\"", task_summary, job_summary)`
- `("job.state == \"COMPLETED\"", task_summary, job_summary)`
- `("invalid_key == \"value\"", task_summary, job_summary)` (missing key)

---

## 6. Kani Harnesses

### Kani Harness: Context key-value lookup correctness
Property: For any Context with values `[(k1, v1), (k2, v2), ...]`, calling `get(ki)` returns `Some(vi)` iff `ki` exists in values
Bound: Context with up to 10 key-value pairs
Rationale: Linear search through Vec could have off-by-one errors or incorrect equality checks

---

### Kani Harness: Middleware chain applies in correct order
Property: For middleware vec `[mw1, mw2, mw3]` applied to handler `h`, execution order is always `mw3 → mw2 → mw1 → h`
Bound: Up to 5 middleware in chain
Rationale: The fold order determines execution order; incorrect associativity breaks Go parity

---

## 7. Mutation Testing Checkpoints

Critical mutations to survive:

| Mutation | Must be caught by |
|----------|-------------------|
| `Context::cancelled` returns Values instead of Cancelled | `context_cancelled_returns_arc_context_where_is_cancelled_is_true` |
| `Context::deadline_exceeded` returns Cancelled instead of DeadlineExceeded | `context_deadline_exceeded_returns_arc_context_where_is_deadline_exceeded_is_true` |
| `is_cancelled` returns `true` for DeadlineExceeded variant | `context_is_cancelled_returns_false_for_deadline_exceeded` |
| `is_deadline_exceeded` returns `true` for Cancelled variant | `context_is_deadline_exceeded_returns_false_for_cancelled` |
| `get_job` checks context AFTER datastore call | `get_job_returns_err_context_cancelled_before_any_io` |
| `should_fire_webhook` Progress + None returns `true` | `should_fire_webhook_progress_none_event_returns_false_gap8_fix` |
| `should_fire_webhook` Progress + EVENT_DEFAULT returns `true` | `should_fire_webhook_progress_default_event_returns_false_gap8_fix` |
| `evaluate_condition` always returns `Ok(true)` | `evaluate_condition_returns_ok_false_when_expression_evaluates_to_false` |
| `evaluate_task_condition` always returns `Ok(false)` | `evaluate_task_condition_returns_ok_true_when_expression_evaluates_to_true` |
| `sanitize_expr` returns original on `{{}}` instead of stripping | `sanitize_expr_handles_empty_braces` |
| `transform_operators` missing `and` → `&&` | `transform_operators_converts_boolean_keywords_and` |
| `transform_operators` missing `or` → `\|\|` | `transform_operators_converts_or_keyword` |

**Threshold: ≥90% mutation kill rate**

## 8. Combinatorial Coverage Matrix

### Context Constructors (unit)
| Scenario | Input | Expected |
|----------|-------|----------|
| cancelled | Context::cancelled() | Arc<Context> with is_cancelled=true |
| deadline_exceeded | Context::deadline_exceeded() | Arc<Context> with is_deadline_exceeded=true |
| with_value chaining | with_value("k1", "v1").with_value("k2", "v2") | both values accessible |

### Context Observers (unit)
| Scenario | Input | Expected |
|----------|-------|----------|
| is_cancelled: true case | Context::cancelled() | true |
| is_cancelled: false for Values | Context::with_value("k", "v") | false |
| is_cancelled: false for DeadlineExceeded | Context::deadline_exceeded() | false |
| is_deadline_exceeded: true case | Context::deadline_exceeded() | true |
| is_deadline_exceeded: false for Cancelled | Context::cancelled() | false |
| is_deadline_exceeded: false for Values | Context::with_value("k", "v") | false |
| get: key exists | Context::with_value("key", "value") + get("key") | Some("value") |
| get: key missing | Context::with_value("other", "v") + get("missing") | None |
| get: on Cancelled context | Context::cancelled() + get("any") | None |
| get: on DeadlineExceeded context | Context::deadline_exceeded() + get("any") | None |

### get_job Context Propagation (unit)
| Scenario | ctx | job_id | Expected |
|----------|-----|--------|----------|
| Valid ctx, existing job | Values("k","v") | "job-123" | Ok(Job) |
| Cancelled ctx | Cancelled | "job-123" | Err(ContextCancelled) |
| DeadlineExceeded ctx | DeadlineExceeded | "job-123" | Err(ContextDeadlineExceeded) |
| Valid ctx, missing job | Values("k","v") | "nonexistent" | Err(JobNotFound) |
| Valid ctx, datastore error | Values("k","v") | "job-123" | Err(Datastore) |
| Empty job_id | Values("k","v") | "" | Err(appropriate variant) |

### Datastore Trait (unit)
| Scenario | Input | Expected |
|----------|-------|----------|
| get_job_by_id: found | valid ctx, existing id | Ok(Job) |
| get_job_by_id: not found | valid ctx, missing id | Err(JobNotFound) |
| get_job_by_id: error | valid ctx, ds errors | Err(Datastore) |

### Webhook Event Matching (unit - 10 combinations)
| Scenario | Event | Webhook Event | Expected |
|----------|-------|--------------|----------|
| StateChange default | StateChange | None | true |
| StateChange empty | StateChange | Some("") | true |
| StateChange EVENT_DEFAULT | StateChange | Some(EVENT_DEFAULT) | true |
| StateChange explicit | StateChange | "job.StateChange" | true |
| StateChange wrong event | StateChange | "job.Progress" | false |
| Progress default | Progress | None | **false** |
| Progress empty | Progress | Some("") | **false** |
| Progress EVENT_DEFAULT | Progress | Some(EVENT_DEFAULT) | **false** |
| Progress explicit | Progress | "job.Progress" | true |
| Read any | Read | * | false |
| task event | StateChange | "task.StateChange" | false |

### evaluate_condition (unit)
| Scenario | Expression | Summary | Expected |
|----------|-----------|---------|----------|
| true expression | "true" | any | Ok(true) |
| false expression | "false" | any | Ok(false) |
| job_state comparison | `job_state == "COMPLETED"` | state=COMPLETED | Ok(true) |
| job_id comparison | `job_id == "test-123"` | id=Some("test-123") | Ok(true) |
| invalid syntax | "1 +" | any | Err(InvalidSyntax) |
| non-boolean result | "1 + 1" | any | Err(NotBoolean) |
| evalexpr failure | causing failure | any | Err(EvaluationFailed) |

### evaluate_task_condition (unit)
| Scenario | Expression | TaskSummary | JobSummary | Expected |
|----------|-----------|------------|------------|----------|
| true expression | "true" | any | any | Ok(true) |
| false expression | "false" | any | any | Ok(false) |
| task.state comparison | `task.state == "PENDING"` | state=PENDING | any | Ok(true) |
| job.state comparison | `job.state == "COMPLETED"` | any | state=COMPLETED | Ok(true) |
| invalid expression | "{{invalid" | any | any | Err variant |

### Error Variants (all 5 must be explicitly asserted)
| Variant | Scenario |
|---------|----------|
| TaskMiddlewareError::JobNotFound(String) | get_job with nonexistent job_id |
| TaskMiddlewareError::ContextCancelled | get_job with Context::cancelled() |
| TaskMiddlewareError::ContextDeadlineExceeded | get_job with Context::deadline_exceeded() |
| TaskMiddlewareError::Middleware(String) | (webhook middleware error path) |
| TaskMiddlewareError::Datastore(String) | Datastore returns error |

## Open Questions

1. **Context::with_value empty key/value handling**: Should empty keys or values be rejected at construction time or allowed? Contract doesn't specify. Tests will assert whatever behavior is implemented.

2. **evaluate_task_condition context format**: Option A (flat) uses `task.state`, Option B (JSON object) uses `task["state"]`. Tests use Option A per current contract documentation but should be updated if Option B is chosen.

3. **evalexpr vs rhai for expression engine**: Current contract stays with evalexpr (Option A). If rhai is chosen later, expression syntax tests must be updated.

(End of file - total 697 lines)
