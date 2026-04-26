# Twerk AI Ergonomics Implementation Plan

## Context

The current Twerk CLI has poor AI ergonomics:
- No 1:1 mapping between CLI commands and API endpoints
- `migration` command exists but user doesn't want it
- Inconsistent exit codes (only Success=0, Failure=1)
- No structured long-form help with examples
- Help system not designed for AI consumption

**Goal**: Make the CLI trivially parseable by AI agents - every API endpoint has a corresponding CLI command, consistent exit codes, rich help, and JSON output everywhere.

## Commands

### Remove
- `migration` - Delete entirely (not wanted)

### Rename
- `run` → `server start` (clearer intent)

### Add (1:1 API Mapping)

#### Job Commands
| CLI | API |
|-----|-----|
| `twerk job list` | `GET /jobs` |
| `twerk job create <json>` | `POST /jobs` |
| `twerk job get <id>` | `GET /jobs/{id}` |
| `twerk job log <id>` | `GET /jobs/{id}/log` |
| `twerk job cancel <id>` | `POST/PUT /jobs/{id}/cancel` |
| `twerk job restart <id>` | `PUT /jobs/{id}/restart` |

#### Scheduled Job Commands
| CLI | API |
|-----|-----|
| `twerk scheduled-job list` | `GET /scheduled-jobs` |
| `twerk scheduled-job create <json>` | `POST /scheduled-jobs` |
| `twerk scheduled-job get <id>` | `GET /scheduled-jobs/{id}` |
| `twerk scheduled-job delete <id>` | `DELETE /scheduled-jobs/{id}` |
| `twerk scheduled-job pause <id>` | `PUT /scheduled-jobs/{id}/pause` |
| `twerk scheduled-job resume <id>` | `PUT /scheduled-jobs/{id}/resume` |

#### Task Commands
| CLI | API |
|-----|-----|
| `twerk task get <id>` | `GET /tasks/{id}` |
| `twerk task log <id>` | `GET /tasks/{id}/log` |

#### Queue Commands
| CLI | API |
|-----|-----|
| `twerk queue list` | `GET /queues` |
| `twerk queue get <name>` | `GET /queues/{name}` |
| `twerk queue delete <name>` | `DELETE /queues/{name}` |

#### Trigger Commands
| CLI | API |
|-----|-----|
| `twerk trigger list` | `GET /api/v1/triggers` |
| `twerk trigger create <json>` | `POST /api/v1/triggers` |
| `twerk trigger get <id>` | `GET /api/v1/triggers/{id}` |
| `twerk trigger update <id> <json>` | `PUT /api/v1/triggers/{id}` |
| `twerk trigger delete <id>` | `DELETE /api/v1/triggers/{id}` |

#### System Commands
| CLI | API |
|-----|-----|
| `twerk health` | `GET /health` |
| `twerk node list` | `GET /nodes` |
| `twerk metrics get` | `GET /metrics` |
| `twerk user create <json>` | `POST /users` |

## Implementation Details

### Exit Codes
```rust
enum ExitCode {
    Success = 0,
    ValidationError = 2,  // Parse errors, bad input
    RuntimeError = 1,     // Operational failures
}
```

### JSON Output Format
```json
{
  "type": "success|error",
  "command": "twerk job list",
  "exit_code": 0,
  "version": "0.1.0",
  "commit": "abc123",
  "data": { ... }
}
```

### Error JSON Format
```json
{
  "type": "error",
  "command": "twerk job get",
  "exit_code": 1,
  "error": {
    "kind": "not_found",
    "message": "job not found: abc123"
  }
}
```

### Help System (Viper-style)
- `--help` short: Brief one-liner description
- `--help --long` (or `-h`): Rich help with:
  - Description
  - Examples with explanations
  - Input formats
  - Output formats
  - AI usage hints

Example long help:
```rust
const LONG_HELP: &str = r#"
Run a job in the Twerk workflow engine.

Examples:
  # Create a job from JSON file
  twerk job create --json < myjob.json

  # Create a job with inline JSON
  twerk job create '{"name": "my-job", "tasks": [...]}'

  # List all jobs with status
  twerk job list

  # Get specific job
  twerk job get abc123

Input: JSON or YAML job definition
Output: Job summary or full job object
"#;
```

## Files to Modify

1. `crates/twerk-cli/src/commands.rs` - Rewrite Commands enum
2. `crates/twerk-cli/src/cli.rs` - Update exit codes, help, JSON output
3. `crates/twerk-cli/src/error.rs` - Update error kinds
4. `crates/twerk-cli/src/main.rs` - May need updates
5. Add new handler modules as needed

## Verification

1. `cargo test` passes
2. `cargo clippy` passes
3. Each new command produces correct JSON output
4. Exit codes are correct for each error type
5. Help text is rich and AI-parseable
6. API endpoint mapping is 1:1

## Status
- [ ] Requirements gathered
- [ ] Plan approved
- [ ] Implementation
- [ ] Tests written
- [ ] Lint/format check
- [ ] Verified
