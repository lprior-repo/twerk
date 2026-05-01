# Findings: tw-sm2 - ARCH-DRIFT: crates/twerk-cli/src/commands.rs

## Status: PERFECT (no code changes)

## Checks Performed

### 1. Line Count Check
- **File**: `crates/twerk-cli/src/commands.rs`
- **Lines**: 363
- **Limit**: 300
- **Status**: ❌ VIOLATION - exceeds 300 line limit by 63 lines

### 2. Module Structure
- Module exports: `Cli`, `Commands`, `RunMode`, `TaskCommand`, `QueueCommand`, `TriggerCommand`, `NodeCommand`, `MetricsCommand`, `UserCommand`
- Structure is clean: clap derive macros for CLI parsing, proper enum hierarchy
- No circular dependencies detected

### 3. Functional Rust Patterns
- Uses `#[derive(Debug, Clone, PartialEq, Eq)]` - good for composability
- `Default` impl for `Commands` is explicit and correct
- No panics, no unwrap in test code
- Pure data types (structs/enums) - aligns with Data layer

### 4. DDD / Scott Wlaschin Check
**PRIMITIVE OBSESSION VIOLATIONS**:
- `id: String` used directly for task IDs, trigger IDs, node IDs
- `name: String` used directly for queue names, usernames
- `body: String` used directly for JSON trigger bodies

These should be wrapped in NewTypes (e.g., `TaskId`, `QueueName`, `TriggerBody`).

**GOOD**:
- `RunMode` is a proper ValueEnum with explicit variants
- Subcommand enums are well-structured with clear variants

### 5. Build Status
- **ERROR**: `twerk-common` crate has missing module `slot` (file not found)
- This is a workspace-level issue that prevents building `twerk-cli`
- Not directly caused by `commands.rs` but blocks full workspace verification

## Recommendations

1. **Split commands.rs**: The file should be split into multiple submodules:
   - `commands/run_mode.rs` - RunMode enum
   - `commands/task.rs` - TaskCommand enum
   - `commands/queue.rs` - QueueCommand enum
   - `commands/trigger.rs` - TriggerCommand enum
   - `commands/node.rs` - NodeCommand enum
   - `commands/metrics.rs` - MetricsCommand enum
   - `commands/user.rs` - UserCommand enum
   - `commands/mod.rs` - Commands enum that re-exports all subcommands

2. **Fix twerk-common slot module**: This is blocking workspace builds

3. **Introduce NewType wrappers** for domain IDs when refactoring DDD patterns
