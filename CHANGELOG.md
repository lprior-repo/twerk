# Changelog

## [0.1.1] - 2026-05-09

### Added
- **`twerk-compiler` crate** — New compiler crate with public API for parsing, validation, and compilation.
- **Expression lexer** — Logos-based lexer for Twerk expression language with full token coverage (operators, literals, identifiers, functions) and source span tracking.

- **Timer scheduler** — Delay and scheduled workflow execution.
- **Performance tooling** — `twerk-bench` end-to-end throughput benchmark and `twerk-perf-kernels` with nightly toolchain.

### Fixed
- **Concurrent isolation test** — Worker engines no longer panic from invalid double-start in integration harness.
- **Distributed E2E tests** — Real-container PostgreSQL and RabbitMQ paths verified end-to-end.
- **OpenAPI annotations** — Undocumented endpoints added and response types corrected.
- **CLI output** — Metrics formatting fixed; `--quiet` flag added.
- **Security headers and XSS sanitization** — Trigger route consistency and default config hardening.

### Verified
- **PostgreSQL** — Real-container tests pass (`postgres_test`, schema, records, encryption).
- **RabbitMQ** — Real-container tests pass (`rabbitmq_test`, `rabbitmq_isolation_test`, distributed E2E with 28 passing tests).
- **Quality gates** — Zero clippy warnings with hard deny rules (`unwrap`, `expect`, `panic`, `dbg_macro`, `todo`, `unimplemented`).

## [0.1.0] - Initial Release

- Core workflow engine with Docker, Podman, and shell runtimes.
- In-memory and PostgreSQL datastores.
- In-memory and RabbitMQ brokers.
- REST API and CLI.
- OpenAPI spec generation.

