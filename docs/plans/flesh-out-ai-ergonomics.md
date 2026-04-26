# Plan: Flesh Out Twerk AI Ergonomics Beads

## Context

The 7 beads created for Twerk AI Ergonomics have skeleton content but are missing the full 16-section detail required for autonomous AI implementation. The beads pass CUE validation but lack the depth needed for a competent AI agent to implement without clarification.

## Current State

| Bead | Title | Current Quality |
|------|-------|-----------------|
| `tw-pyjt` | cli: Define exit code system and error taxonomy | Skeleton - needs fleshing |
| `tw-4vkr` | cli: Implement Viper-style long/short help system | Skeleton - needs fleshing |
| `tw-ttcb` | cli: Implement structured JSON output for all commands | Skeleton - needs fleshing |
| `tw-d2iw` | cli: Restructure commands enum with API 1:1 mapping | Skeleton - needs fleshing |
| `tw-ewk2` | cli: Implement job and scheduled-job command handlers | Skeleton - needs fleshing |
| `tw-avcw` | cli: Implement task, queue, and trigger command handlers | Skeleton - needs fleshing |
| `tw-trp5` | cli: Implement system commands and server start | Skeleton - needs fleshing |

## What Needs Fleshing (Per Enhanced Schema)

### Section 0: Clarifications
**Currently**: Only `clarification_status: "RESOLVED"`
**Needs**:
- Resolved clarifications with actual Q&A pairs about edge cases
- Any open questions explicitly called out
- Assumptions with validation methods

### Section 1: EARS Requirements
**Currently**: Has ubiquitous, event_driven, unwanted
**Needs**: ✓ Adequate, but could be more specific

### Section 2: KIRK Contracts
**Currently**: Has preconditions/postconditions/invariants
**Needs**:
- `required_inputs` array is empty - needs actual InputSpec entries
- `return_guarantees` array is empty - needs ReturnGuarantee entries
- Each input needs: field, type, constraints, example_valid, example_invalid

### Section 2.5: Research Requirements
**Currently**: Has files_to_read only
**Needs**:
- `patterns_to_find` - regex patterns to search in codebase
- `prior_art` - similar implementations to study
- `research_questions` - specific questions to answer with answers
- `research_complete_when` - concrete criteria

### Section 3: Inversions
**Currently**: Has one usability_failure only
**Needs**:
- security_failures with specific scenarios
- usability_failures (expand)
- data_integrity_failures
- integration_failures
- Each failure needs: failure, prevention, test_for_it

### Section 4: ATDD Tests
**Currently**: Tests have generic "Task scope: ..." text
**Needs**:
- Real Given/When/Then with actual example data
- `real_input` with actual CLI commands
- `expected_output` with actual JSON structure
- Edge case tests
- Contract tests verifying preconditions/postconditions/invariants

### Section 5: E2E Tests
**Currently**: Has skeleton pipeline_test with empty setup
**Needs**:
- Real `setup` with files_to_create and environment
- Real `execute` with actual commands
- Real `verify` with stdout_contains and stdout_matches_json
- `cleanup` with actual cleanup commands
- Additional e2e_scenarios

### Section 5.5: Verification Checkpoints
**Currently**: Has gates with generic checks
**Needs**:
- tests_json with format and location
- More specific checks per gate

### Section 6: Implementation Tasks
**Currently**: Has basic tasks
**Needs**:
- More granular tasks
- parallel_group markers
- depends_on relationships
- patterns_to_use
- actual commands for verification

### Section 7: Failure Modes
**Currently**: One generic failure mode
**Needs**:
- Multiple specific failure modes with:
  - symptom
  - likely_cause
  - where_to_look (file, function, line_range)
  - fix_pattern
- debugging_commands with scenario/run/look_for

### Section 7.5: Anti-Hallucination
**Currently**: Has read_before_write
**Needs**:
- `verify_before_reference` - prove APIs exist before using
- `apis_that_exist` - actual function signatures from codebase
- `apis_that_do_not_exist` - common hallucinations to avoid
- `no_placeholder_values` - rules like "Use REAL UUIDs from test fixtures"

### Section 7.6: Context Survival
**Currently**: Has progress_file path
**Needs**:
- tests_status_file with path and update_frequency
- research_notes_file with path and contents
- git_checkpoints with frequency and message_format
- More detailed recovery_instructions

### Section 8: Completion Checklist
**Currently**: Has generic items
**Needs**: More specific checklist items with actual verification commands

### Section 9: Context
**Currently**: Has related_files only
**Needs**:
- similar_implementations - reference similar code
- external_references - docs URLs
- codebase_patterns - patterns to use with example_location and how_to_apply

### Section 10: AI Hints
**Currently**: Has do/do_not/constitution
**Needs**:
- language_guidance (avoid/used_instead)
- action_guidance
- parallel_execution guidance
- incremental_progress guidance
- code_patterns with name/use_when/example

## Detailed Fleshing Plan for Each Bead

### 1. tw-pyjt: Exit Codes (1hr)
**Files to Read First**:
- `crates/twerk-cli/src/error.rs:1-144` - Current error enum
- `crates/twerk-cli/src/cli.rs:41-45` - ExitStatus enum
- `crates/twerk-cli/src/cli.rs:235-248` - handle_runtime_error

**Specific Fleshing Needed**:
- Input specs for: json flag, endpoint URL, job ID
- Return guarantees for: exit_code field mapping to i32
- API patterns: `reqwest::Error` handling, `EndpointError` handling
- Test: `twerk job get invalid-id --json` should return exit 2 with `{"type":"error","error":{"kind":"validation"}}`
- Exit code mapping:
  - 0 = Success
  - 1 = Runtime error (network, datastore, broker)
  - 2 = Parse/validation error (invalid JSON, missing args, bad endpoint)

### 2. tw-4vkr: Help System (2hr)
**Files to Read First**:
- `crates/twerk-cli/src/cli.rs:157-172` - render_help_for_path, render_top_level_help
- `crates/twerk-cli/src/commands.rs:1-64` - Commands enum

**Specific Fleshing Needed**:
- LONG_HELP constants for each command (job, task, queue, trigger, scheduled-job, node, metrics, user)
- Short help: 1-2 lines
- Long help structure:
  ```
  Description: [2-3 sentences]
  Usage: twerk job <subcommand> [options]
  Examples:
    twerk job list                    # List all jobs
    twerk job create < file.json      # Create job from file
  Input: JSON/YAML job definition
  Output: Job summary or full job object
  ```
- --help --long flag parsing in clap

### 3. tw-ttcb: JSON Output (1hr)
**Files to Read First**:
- `crates/twerk-cli/src/cli.rs:174-205` - json_error_payload, json_help_payload, json_version_payload

**Specific Fleshing Needed**:
- json_success_payload structure:
  ```json
  {
    "type": "success",
    "command": "twerk job list",
    "exit_code": 0,
    "version": "0.1.0",
    "commit": "abc123",
    "data": { ... }
  }
  ```
- Error JSON structure:
  ```json
  {
    "type": "error",
    "command": "twerk job get",
    "exit_code": 1,
    "error": {
      "kind": "not_found|runtime|validation",
      "message": "human readable message"
    }
  }
  ```
- All commands must wire to structured output

### 4. tw-d2iw: Commands Enum (4hr) - LARGEST BEAD
**Files to Read First**:
- `crates/twerk-cli/src/commands.rs` - Full file
- `crates/twerk-web/src/api/router.rs` - All routes
- `crates/twerk-web/src/api/handlers/` - All handlers

**Specific Fleshing Needed**:
Complete Commands enum with ALL variants:

```rust
pub enum Commands {
    // Server
    Server { subcommand: ServerCommands },

    // Job commands
    Job { subcommand: JobCommands },

    // Scheduled job commands
    ScheduledJob { subcommand: ScheduledJobCommands },

    // Task commands
    Task { subcommand: TaskCommands },

    // Queue commands
    Queue { subcommand: QueueCommands },

    // Trigger commands
    Trigger { subcommand: TriggerCommands },

    // System commands
    Health,
    Node { subcommand: NodeCommands },
    Metrics,
    User { subcommand: UserCommands },
}
```

Each subcommand enum has list/create/get/etc variants.

**Test Examples**:
- `twerk job list --json` parses to `Commands::Job { subcommand: JobCommands::List }`
- `twerk job create --json '{"name": "test"}'` parses with JSON body
- `twerk scheduled-job pause <id>` parses correctly

### 5. tw-ewk2: Job Handlers (4hr)
**Files to Read First**:
- `crates/twerk-web/src/api/handlers/jobs/create.rs`
- `crates/twerk-web/src/api/handlers/jobs/read.rs`
- `crates/twerk-web/src/api/handlers/jobs/mutation.rs`

**Specific Fleshing Needed**:
- Each handler maps to API endpoint
- HTTP client calls using reqwest
- Response parsing from JSON
- Error mapping to CliError

**Test Structure**:
```rust
#[tokio::test]
async fn test_job_list_success() {
    // Setup mock server
    // Run: twerk job list --json
    // Assert exit 0, stdout contains jobs array
}
```

### 6. tw-avcw: Task/Queue/Trigger Handlers (4hr)
**Files to Read First**:
- `crates/twerk-web/src/api/handlers/tasks.rs`
- `crates/twerk-web/src/api/handlers/queues.rs`
- `crates/twerk-web/src/api/trigger_api/handlers/`

**Specific Fleshing Needed**:
Same pattern as job handlers but for different resource types.

### 7. tw-trp5: System Commands (2hr)
**Files to Read First**:
- `crates/twerk-cli/src/health.rs`
- `crates/twerk-cli/src/run.rs`

**Specific Fleshing Needed**:
- `twerk health` → GET /health
- `twerk node list` → GET /nodes
- `twerk metrics get` → GET /metrics
- `twerk user create <json>` → POST /users
- `twerk server start <mode>` → Replace current `run` command

## Implementation Order

1. **tw-pyjt** (Exit Codes) - Foundation, other beads depend on this
2. **tw-ttcb** (JSON Output) - Foundation, used by all other beads
3. **tw-d2iw** (Commands Enum) - Structural, many beads depend on this
4. **tw-4vkr** (Help System) - Can parallel with tw-pyjt
5. **tw-ewk2**, **tw-avcw**, **tw-trp5** - Can parallel after above complete

## Verification

Each bead should pass:
1. `cue vet` against enhanced-bead.cue schema
2. All 16 sections present with non-placeholder content
3. Tests have real input/output, not "Task scope: ..."
4. Implementation tasks are granular and actionable

## Status
- [ ] Plan created
- [ ] Awaiting approval to flesh out beads
