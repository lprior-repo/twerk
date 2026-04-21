# CLI Reference

## Commands

### `twerk run`

Start Twerk in a specific mode.

```bash
twerk run <MODE> [OPTIONS]
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

## Top-Level Flags

| Option | Description |
|--------|-------------|
| `--json` | Emit machine-readable help or health output |
| `--help` | Show CLI help |
| `--version` | Show the current version |

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
