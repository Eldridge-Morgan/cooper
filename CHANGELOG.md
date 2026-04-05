# Changelog

All notable changes to Cooper will be documented in this file.

## [0.2.0] - 2026-04-06

### Added

- **TypeScript SDK** (`sdk/`) — full implementation of all Cooper modules:
  - `cooper/api` — route definition with `api()` function
  - `cooper/db` — database client with Postgres and MySQL support via connection pooling
  - `cooper/middleware` — composable middleware with `middleware()` and `cooper.use()`
  - `cooper/auth` — auth handler with automatic principal injection into protected routes
  - `cooper/pubsub` — typed topics with `topic()`, publish/subscribe, delivery guarantees
  - `cooper/cron` — cron job scheduling with human-readable and cron expression support
  - `cooper/cache` — Valkey/Redis cache with `cache()`, getOrSet, TTL, prefix invalidation
  - `cooper/storage` — object storage with `bucket()`, upload/download/signedUrl/list
  - `cooper/secrets` — secret management with `secret()`, fetched at runtime from vault
  - `cooper/queue` — job queues with `queue()`, retries, exponential backoff, dead-letter, priority, deduplication
  - `cooper/ssr` — server-side rendering with `page()`, `layout()`, `pageLoader()`, streaming `Suspense`
  - `cooper/islands` — selective hydration with `island()`, hydration strategies (load/visible/idle/interaction/none)
  - `cooper/ai` — vector store with cosine similarity search, LLM gateway with fallbacks and budget limits
  - `cooper` — re-exports `CooperError` with structured error codes

- **JS Worker Pool** — Rust spawns a pool of 4 Bun/Node/Deno worker processes communicating via JSON-RPC over stdin/stdout. Requests are round-robin distributed across workers for true parallelism.

- **JS Bridge** (`sdk/src/bridge.ts`) — worker process that loads user TypeScript modules on demand, executes handlers with full middleware chain, Zod validation, and auth verification. Supports hot-reload via cache invalidation.

- **Embedded Infrastructure** — `cooper run` automatically starts:
  - Embedded Postgres via `pg_ctl` (or connects to existing instance)
  - NATS JetStream for pub/sub (falls back to in-process)
  - Valkey/Redis for caching (falls back to in-process)
  - Auto-migration runner for SQL files
  - Each service has a 10s startup timeout — server always starts

- **Cron Scheduler** — parses human-readable schedules ("every 1 hour", "every 30 minutes") and standard cron expressions, executes handlers via the JS worker pool

- **Client Code Generation** (`cooper gen client`) — generates fully typed API clients:
  - TypeScript — class with async methods, path param substitution, auth token injection
  - Python — httpx-based client with snake_case method names
  - Rust — reqwest-based client with proper error handling
  - Postman — full collection with environment variables

- **API Introspection Endpoints**:
  - `GET /_cooper/health` — always-on health check
  - `GET /_cooper/info` — returns full project analysis (routes, databases, topics, crons, queues, pages)

- **Richer Project Scaffolding** (`cooper new`) now generates:
  - Full CRUD user service with validation, caching, and event publishing
  - Auth handler with JWT placeholder
  - Health check service
  - Event subscriber example
  - Cron job example
  - Shared types module

### Changed

- Hot reload now ignores `.cooper/`, `node_modules/`, `dist/`, and `target/` directories
- Server reload invalidates JS module caches without restarting worker processes

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

### Not yet implemented

- Actual cloud provisioning (AWS SDK, GCP, Azure, Fly)
- SSR HTML rendering engine
- Dashboard UI
- MCP server
