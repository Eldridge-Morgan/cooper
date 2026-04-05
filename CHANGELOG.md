# Changelog

All notable changes to Cooper will be documented in this file.

## [0.1.0] - 2026-04-05

### Added

- **CLI binary** with full command structure: `new`, `run`, `build`, `deploy`, `destroy`, `db`, `secrets`, `gen`, `logs`, `trace`, `bench`, `env`, `docs`, `mcp`
- **Static TypeScript analyzer** — regex-based parser that extracts `api()`, `database()`, `topic()`, `cron()`, `queue()` declarations from source at build time
- **HTTP runtime** built on Axum/Hyper 1.x/Tokio — multi-threaded request routing with path parameter extraction
- **Project scaffolding** (`cooper new`) — generates services, pages, migrations, config, tsconfig, gitignore
- **Hot reload** via filesystem watcher — re-analyzes project and rebuilds route table on file changes
- **Structured errors** — `CooperError` with automatic HTTP status code mapping (NOT_FOUND → 404, UNAUTHORIZED → 401, etc.)
- **OpenAPI 3.1 generator** — produces spec from analyzed route declarations
- **Cloud deployment planner** — maps Cooper declarations to AWS/GCP/Azure/Fly resources with cost estimates
- **Deploy diffing engine** — dry-run mode shows resource creates/updates/deletes with monthly cost delta

### Architecture

- Cargo workspace with 4 crates: `cooper-cli`, `cooper-runtime`, `cooper-codegen`, `cooper-deploy`
- Zero external runtime dependencies — single static binary
- Cooper-style paths (`/users/:id`) auto-converted to framework paths at startup

### Not yet implemented

- JavaScript/TypeScript handler execution (V8/Deno Core integration)
- Embedded local infrastructure (Postgres, NATS, Valkey)
- Actual cloud provisioning (AWS SDK, GCP, Azure, Fly)
- SSR rendering and Islands hydration
- Typed client generation (TypeScript, Python, Rust)
- Dashboard UI
- Pub/Sub message delivery
- Queue worker execution
- Cron scheduler
- Cache and object storage primitives
- Secrets vault
- MCP server
