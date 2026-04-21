# Repo Memory

Use this for durable repository contracts and recurring implementation facts.

## Core Contracts

- The shipped CLI binary is `twerk`, not `twerk-cli`.
- The repo-root local standalone flow is based on `config.toml`.
- The local-friendly default standalone config uses `inmemory` broker/datastore and `shell` runtime.
- Config content is TOML; legacy `.yaml` and `.yml` filenames may still be discovered, but their contents must still parse as TOML.
- This repo uses `bd` for issue tracking.

## CI And Testing

- `moon run :ci-source` is the main source-quality gate.
- Repeated logging bootstrap should be idempotent in tests and CLI setup.
- Process-global env vars in tests can race; env-mutating tests should be isolated or serialized.
- Typed IDs are strict; test payloads and fixtures must use valid `JobId`, `TriggerId`, and similar IDs instead of placeholder strings.

## Generated Artifacts

- OpenAPI tracked artifacts can drift after API contract changes.
- Regenerate tracked OpenAPI artifacts with `cargo run -p twerk-openapi-gen` when contract tests fail on spec drift.

## Update Rules

- Store facts that change how the repo should be built, tested, configured, or released.
- Skip details that are obvious from current source unless they are easy to forget and expensive to rediscover.
