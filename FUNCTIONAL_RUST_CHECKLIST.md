# Functional Rust Principles Audit

This document audits the codebase against the **Big 6 Core Constraints** and tracks implementation status.

## The Big 6 Core Constraints

### 1. Data → Calc → Actions Architecture ✅ PARTIALLY IMPLEMENTED

**Status:** The codebase has a reasonable separation, but many calculations are mixed with actions.

**Findings:**
- ✅ `twerk-core` contains pure domain types and calculations
- ✅ `twerk-common` contains shared utilities
- ⚠️ Some I/O logic mixed in `twerk-app/src/engine/coordinator/`
- ⚠️ Database operations in `twerk-infrastructure`

**Recommendations:**
- Push all business logic to `twerk-core` calculations
- Keep `twerk-app` as thin coordinator layer
- Ensure `twerk-infrastructure` only handles I/O

### 2. Zero Mutability ❌ NEEDS WORK

**Status:** 52 instances of `let mut` found in core logic

**Findings:**
```bash
$ grep -r "let mut" crates/twerk-core/src --include="*.rs" | grep -v "#[cfg(test)]" | wc -l
52
```

**Examples of mut usage:**
- HashMap insertions
- Vector pushes in loops
- Mutable iterator state

**Recommendations:**
- Replace `for` loops with iterator pipelines (`map`, `filter`, `fold`)
- Use `rpds` or `im` for persistent state
- Use `ArcSwap` for global state pointers instead of `RwLock`

### 3. Zero Panics/Unwraps ✅ FIXED

**Status:** All unwrap/expect/panic calls have been replaced with proper error handling.

**Findings:**
- ✅ Fixed `JobId::new()` unwrap in `trigger/in_memory.rs`
- ✅ Added `TriggerError::JobIdGenerationFailed` for proper error propagation
- ✅ All Result types handled with `?` operator

**Remaining concerns:**
- Check for any `unwrap()` in non-test code
- Verify all `expect()` calls are eliminated

### 4. Make Illegal States Unrepresentable ✅ PARTIALLY IMPLEMENTED

**Status:** Good foundations with ID newtypes and state enums, but significant gaps remain.

**Implemented:**
- ✅ `JobId`, `TaskId`, `NodeId`, `UserId`, `RoleId`, `TriggerId` - all properly typed
- ✅ `JobState`, `TaskState`, `ScheduledJobState`, `NodeStatus` - proper state enums
- ✅ `QueueName`, `CronExpression`, `GoDuration`, `Priority`, `RetryLimit` - some domain types

**Missing Critical Types:**
- ❌ `Url` / `WebhookUrl` - URLs should be validated
- ❌ `Hostname` - hostnames should be validated
- ❌ `Port` - port numbers should be validated (1-65535)
- ❌ `CronExpression` - cron expressions should be parsed
- ❌ `ImageName` - Docker image names should be validated
- ❌ `QueueName` - queue names should be validated
- ❌ `Password` / `Secrets` - credentials need zeroization
- ❌ `Progress` - progress should be bounded 0-100
- ❌ `Duration` / `Timeout` - durations should be parsed
- ❌ `TaskCount` / `TaskPosition` - counts should be validated

**HashMap<String, String> Anti-pattern:**
```rust
// BAD - No schema enforcement
pub inputs: Option<HashMap<String, String>>
pub secrets: Option<HashMap<String, String>>
pub env: Option<HashMap<String, String>>
```

**Recommendation:**
- Create typed wrappers: `JobInputs`, `TaskEnv`, `TaskSecrets`
- Each wrapper should have type-safe accessors

### 5. Expression-Based Programming ⚠️ NEEDS WORK

**Status:** Mixed usage of expression and statement-based code.

**Findings:**
- ✅ Many functions use iterator pipelines
- ⚠️ Some functions use imperative `for` loops
- ⚠️ Some `match` statements could be `if let`

**Recommendations:**
- Replace `for` loops with `for item in collection.iter().map(...).collect()`
- Use `if let` for single-pattern matches
- Prefer `map_or_else` over `match` for simple transformations

### 6. Clippy Flawless ⚠️ NEEDS WORK

**Status:** Core clippy checks pass, but pedantic warnings exist.

**Current Status:**
```bash
$ cargo clippy -- -D warnings
✓ PASS (after fixes)

$ cargo clippy -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
✓ PASS (after fixes)

$ cargo clippy -- -W clippy::pedantic
⚠ 70+ warnings (mostly documentation)
```

**Pedantic Warnings:**
- Documentation missing `# Errors` sections
- Missing backticks in documentation
- Casting precision warnings
- Some redundant closures

**Recommendations:**
- Add `# Errors` sections to all Result-returning functions
- Fix documentation backticks
- Address casting warnings with explicit conversions

## Perfect 10 Stack Usage Audit

### Core Layer (Pure Functions)

| Crate | Status | Usage |
|-------|--------|-------|
| `itertools` | ✅ | Used for iterator pipelines |
| `rayon` | ⚠️ | Some parallel pipelines, could be expanded |
| `rpds` | ❌ | Not used - consider for immutable state |
| `bytes` | ⚠️ | Used in some network code |
| `smallvec` | ❌ | Not used - consider for small collections |
| `thiserror` | ✅ | Used for domain errors |
| `tap` | ❌ | Not used - could improve pipeline readability |

### Shell Layer (I/O and State)

| Crate | Status | Usage |
|-------|--------|-------|
| `arc-swap` | ❌ | Not used - consider for global state |
| `dashmap` | ⚠️ | Used in some concurrent state |
| `anyhow` | ✅ | Used for boundary errors |

## Type Safety Opportunities (Priority List)

### HIGH PRIORITY - Immediate Implementation

1. **Credentials & Secrets** (Security Critical)
   - Create `Password` type with `zeroize`
   - Create `Secrets` type with automatic redaction
   - Update `User`, `Registry` structs

2. **URL & Hostname Validation**
   - Create `WebhookUrl` type with RFC 3986 validation
   - Create `Hostname` type with DNS validation
   - Update `Webhook`, `Node` structs

3. **Port Numbers**
   - Create `Port` type with 1-65535 validation
   - Update `Probe`, `Node` structs

### MEDIUM PRIORITY - Phase 2

4. **Cron Expression Parsing**
   - Create `CronExpression` type with cron parsing
   - Update `Job`, `ScheduledJob` structs

5. **Quantity Types**
   - Create `Progress` type (0-100)
   - Create `RetryLimit`, `RetryAttempt` types
   - Create `TaskCount`, `TaskPosition` types

6. **Typed HashMap Alternatives**
   - Create `JobInputs`, `TaskEnv`, `TaskSecrets`
   - Add type-safe accessors

### LOW PRIORITY - Future Improvements

7. **Image & Queue Names**
   - Create `ImageName` type
   - Create `QueueName` type

8. **Duration Types**
   - Create `Duration` / `Timeout` types
   - Replace String durations

9. **Command Types**
   - Create `Command`, `Entrypoint` types
   - Replace `Vec<String>` command vectors

## Implementation Roadmap

### Phase 1: Security (Week 1)
- [ ] Implement `Password` with zeroization
- [ ] Implement `Secrets` with redaction
- [ ] Update all credential fields
- [ ] Add zeroization tests

### Phase 2: Validation (Week 2)
- [ ] Implement `WebhookUrl`
- [ ] Implement `Hostname`
- [ ] Implement `Port`
- [ ] Update webhook, node types

### Phase 3: Quantities (Week 3)
- [ ] Implement `Progress`
- [ ] Implement `RetryLimit`, `RetryAttempt`
- [ ] Implement `TaskCount`, `TaskPosition`
- [ ] Update task, job types

### Phase 4: HashMap Wrappers (Week 4-5)
- [ ] Implement `JobInputs`, `TaskEnv`, `TaskSecrets`
- [ ] Update all HashMap<String, String> usages
- [ ] Add type-safe accessors

### Phase 5: Infrastructure Cleanup (Week 6-8)
- [ ] Update database record conversions
- [ ] Update Docker/Podman runtime types
- [ ] Update CLI and web layer types

## CI Verification Commands

```bash
# Core clippy checks (should pass)
cargo clippy -- -D warnings -D clippy::unwrap_used -D clippy::panic -D clippy::expect_used

# Formatting
cargo fmt --check

# Tests
cargo nextest run

# If moon is available
moon run :ci-source
```

## Success Metrics

- [ ] Zero `let mut` in core calculations (use persistent state)
- [ ] Zero `unwrap()`/`expect()` in non-test code
- [ ] All passwords use zeroizing types
- [ ] All URLs validated at construction
- [ ] All cron expressions validated
- [ ] All progress values bounded 0-100
- [ ] All retry counts non-negative
- [ ] HashMap<String, String> replaced with typed alternatives where schema is known
- [ ] All clippy warnings resolved (pedantic level)
- [ ] All tests passing

## Current CI Status

```
cargo clippy -- -D warnings
✓ PASS (exit 0)

cargo clippy -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic
✓ PASS (exit 0)

cargo clippy -- -W clippy::pedantic
⚠ 70+ warnings (documentation and minor issues)
```

## Notes

- The codebase has **excellent foundations** with ID newtypes and state enums
- Focus should be on filling gaps in type safety
- All newtypes should follow existing patterns (transparent serialization, Deref to str)
- Use `thiserror` for validation errors
- Add comprehensive tests for each newtype
