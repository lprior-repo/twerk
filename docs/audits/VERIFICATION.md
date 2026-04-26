# Twerk Verification

**Date**: April 20, 2026
**Status**: Updated for the current standalone user journey

## Scope

This checklist covers the primary local flow:

1. Build the `twerk` binary
2. Start standalone mode with the repo-root `config.toml`
3. Verify `/health`
4. Submit `examples/hello-shell.yaml`
5. Inspect job state and logs

It does not claim that every historical example or every distributed deployment path has been re-verified.

## Local Verification Flow

```bash
cargo build -p twerk-cli
./target/debug/twerk run standalone
```

The checked-in `config.toml` uses:

- `broker.type = "inmemory"`
- `datastore.type = "inmemory"`
- `runtime.type = "shell"`

Health check:

```bash
curl -s http://localhost:8000/health
# Expected: {"status":"UP","version":"0.1.0"}
```

Submit a blocking job:

```bash
curl -X POST 'http://localhost:8000/jobs?wait=true' \
  -H "Content-Type: text/yaml" \
  --data-binary @examples/hello-shell.yaml
```

Inspect stored state:

```bash
curl http://localhost:8000/jobs
curl http://localhost:8000/jobs/<job-id>/log
```

## Important Notes

- The shipped CLI binary name is `twerk`.
- Config is loaded from `TWERK_CONFIG` or the default config file search paths. There is no `--config` flag.
- Distributed mode still requires Postgres, RabbitMQ, and a container runtime for image-based tasks.

## Related Docs

- `website/src/quick-start.md`
- `website/src/configuration.md`
- `examples/hello-shell.yaml`
- `configs/sample.config.toml`
