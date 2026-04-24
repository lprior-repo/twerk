# Findings: tw-d2iw - cli: Restructure commands enum with API 1:1 mapping

## Summary

Successfully restructured the twerk CLI commands enum to have a 1:1 mapping with API endpoints.

## Changes Made

### 1. commands.rs

- Removed `Migration` command
- Renamed `Run` to `ServerStart` (parsed as `server-start` by clap)
- Added `JobCommand` with variants: `List`, `Create`, `Get`, `Log`, `Cancel`, `Restart`
- Added `ScheduledJobCommand` with variants: `List`, `Create`, `Get`, `Delete`, `Pause`, `Resume`
- Removed `NodeCommand::Get` variant (API only has `GET /nodes`, no `GET /nodes/{id}`)
- Updated all command variants to use consistent naming

### 2. dispatch.rs

- Updated `execute_command` to handle new `Job` and `ScheduledJob` commands
- Removed migration handling
- Updated command names

### 3. handlers/

- Created `handlers/job.rs` with implementations for: `job_list`, `job_create`, `job_get`, `job_log`, `job_cancel`, `job_restart`
- Created `handlers/scheduled_job.rs` with implementations for: `scheduled_job_list`, `scheduled_job_create`, `scheduled_job_get`, `scheduled_job_delete`, `scheduled_job_pause`, `scheduled_job_resume`
- Updated `handlers/mod.rs` to export new modules

### 4. Tests

- Updated all references from `Commands::Run` to `Commands::ServerStart`
- Removed all migration-related tests
- Added tests for new Job and ScheduledJob commands
- Updated test assertions to use `server-start` instead of `server start` (clap uses hyphens for multi-word variants)

## API Mapping

| CLI Command | API Endpoint |
|-------------|--------------|
| `twerk job list` | GET /jobs |
| `twerk job create <body>` | POST /jobs |
| `twerk job get <id>` | GET /jobs/{id} |
| `twerk job log <id>` | GET /jobs/{id}/log |
| `twerk job cancel <id>` | PUT /jobs/{id}/cancel |
| `twerk job restart <id>` | PUT /jobs/{id}/restart |
| `twerk scheduled-job list` | GET /scheduled-jobs |
| `twerk scheduled-job create <body>` | POST /scheduled-jobs |
| `twerk scheduled-job get <id>` | GET /scheduled-jobs/{id} |
| `twerk scheduled-job delete <id>` | DELETE /scheduled-jobs/{id} |
| `twerk scheduled-job pause <id>` | PUT /scheduled-jobs/{id}/pause |
| `twerk scheduled-job resume <id>` | PUT /scheduled-jobs/{id}/resume |

## Verification

- All 148 tests pass
- Build succeeds with only minor warnings about unused migration functions
- CLI help shows correct structure with new commands