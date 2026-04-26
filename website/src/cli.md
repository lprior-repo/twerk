# CLI Reference

## Commands

### `twerk server start`

Start Twerk in a specific mode.

```bash
twerk server start <MODE> [OPTIONS]
```

| Mode | Description |
|------|-------------|
| `standalone` | All-in-one: Coordinator + Worker |
| `coordinator` | API server, requires separate workers |
| `worker` | Task executor, requires coordinator |

| Option | Description | Default |
|--------|-------------|---------|
| `--hostname <HOSTNAME>` | Coordinator hostname for workers | none |

Config is loaded from `TWERK_CONFIG` or the default config search paths. There is no `--config` CLI flag.

### `twerk migration`

Run database migrations.

```bash
twerk migration [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-y, --yes` | Skip confirmation prompt |

`twerk migration` reads the datastore type and Postgres DSN from config or `TWERK_*` environment variables.

### `twerk health`

Check coordinator health.

```bash
twerk health [OPTIONS]
```

| Option | Description | Default |
|--------|-------------|---------|
| `-e, --endpoint <URL>` | Coordinator endpoint | `http://localhost:8000` |

### `twerk version`

Show the current CLI version.

```bash
twerk version [--json]
```

Text-mode version discovery commands are clean endpoints: they print only the version line to stdout, keep stderr empty, and exit `0`.

Supported forms:

- `twerk --version` → `twerk <VERSION>`
- `twerk version` → `twerk <VERSION>`
- `twerk run --version` → `twerk-run <VERSION>`
- `twerk migration --version` → `twerk-migration <VERSION>`
- `twerk health --version` → `twerk-health <VERSION>`

## Top-Level Flags

| Option | Description |
|--------|-------------|
| `--json` | Emit machine-readable JSON for help, version, parse errors, and command failures on stdout |
| `--help` | Show CLI help |
| `--version` | Show the current version |

## JSON Behavior

- Help discovery commands such as `twerk --json`, `twerk --json --help`, `twerk help --json`, and `twerk run --json --help` return JSON with a rendered `content` field.
- Version discovery commands such as `twerk --json --version` and `twerk version --json` return JSON on stdout, keep stderr empty, and exit `0`.
- JSON parse failures keep Clap exit code `2` and write structured error JSON to stdout.
- JSON command validation and runtime failures exit `1`, write structured error JSON to stdout, and keep stderr empty.

## Environment Variables

All configuration can be set via environment variables:

```
TWERK_<SECTION>_<KEY>=value
```

Examples:
- `TWERK_LOGGING_LEVEL=debug`
- `TWERK_BROKER_TYPE=rabbitmq`
- `TWERK_DATASTORE_TYPE=postgres`
- `TWERK_RUNTIME_TYPE=docker`

See [Configuration](configuration.md) for full reference.
