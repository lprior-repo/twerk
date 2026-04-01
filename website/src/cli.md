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

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `--config <PATH>` | Path to config.toml | See configuration docs |

### `twerk migration`

Run database migrations.

```bash
twerk migration [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-y, --yes` | Skip confirmation prompt |

### `twerk health`

Check coordinator health.

```bash
twerk health [OPTIONS]
```

| Option | Description | Default |
|--------|-------------|---------|
| `-e, --endpoint <URL>` | Coordinator endpoint | `http://localhost:8000` |

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
