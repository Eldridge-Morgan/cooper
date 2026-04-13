# Changelog

All notable changes to Cooper will be documented in this file.

## [0.6.0] - 2026-04-13

### Added

- **Terraform-based deployment** — `cooper deploy` now generates real Terraform HCL instead of calling cloud CLIs directly:
  - Dynamically generates `main.tf`, `variables.tf`, `outputs.tf`, `provider.tf` based on project analysis
  - All Terraform files written locally to `.cooper/terraform/{env}/` for full user control
  - Users can review, edit, and version-control the generated infrastructure before applying

- **`--service` flag** — choose between container and function-based deployment:
  - `--service server` (default) — container-based: ECS Fargate, Cloud Run, Azure Container Apps
  - `--service serverless` — function-based: AWS Lambda + API Gateway, GCP Cloud Functions v2, Azure Functions
  - Database, cache, messaging, and storage resources are the same regardless of service type

- **Six cloud x service mappings** with full Terraform resource generation:
  - **AWS Server**: VPC, subnets, internet gateway, route tables, security group rules, IAM roles, ECR, ECS Cluster, Fargate task definition + service, CloudWatch log group, RDS, ElastiCache, SNS, SQS, S3
  - **AWS Serverless**: Same networking + data resources, but Lambda function, API Gateway HTTP API, integration/route/stage, Lambda permissions instead of ECS
  - **GCP Server**: VPC network, subnet, VPC Access Connector, Cloud Run v2 service, IAM (public invoker), Cloud SQL, Memorystore Redis, Pub/Sub topics, Cloud Tasks queues, GCS bucket, service account
  - **GCP Serverless**: Same infra, but Cloud Functions v2 with GCS source bucket instead of Cloud Run
  - **Azure Server**: Resource group, VNet, subnets, Log Analytics, Container Apps environment + app, PostgreSQL Flexible Server, Azure Redis Cache, Service Bus namespace + queues, Blob Storage
  - **Azure Serverless**: Same infra, but Azure Functions (Linux, Consumption plan) with dedicated storage account instead of Container Apps

- **Interactive deploy workflow** with four options after preview:
  - **Apply** — proceed with `terraform init` → `plan` → `apply`
  - **Edit files** — opens `.cooper/terraform/{env}/main.tf` in `$EDITOR`, then confirms
  - **Show full config** — displays all four `.tf` files inline
  - **Cancel** — aborts without changes

- **Credential detection and collection** (`credentials.rs`):
  - **AWS**: checks `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` env vars, then `~/.aws/credentials` / `AWS_PROFILE`, prompts interactively if missing
  - **GCP**: checks `GOOGLE_APPLICATION_CREDENTIALS`, then ADC at `~/.config/gcloud/`, resolves project ID from env or `gcloud config`
  - **Azure**: checks `ARM_CLIENT_ID`/`ARM_CLIENT_SECRET`/`ARM_TENANT_ID`/`ARM_SUBSCRIPTION_ID`, falls back to `az account show`, prompts for service principal if needed
  - **Fly.io**: checks `FLY_API_TOKEN`, falls back to `flyctl` auth

- **Terraform executor** — wraps the Terraform CLI lifecycle:
  - `terraform init` with no-color, no-input flags
  - `terraform plan -out=tfplan` with change count summary (adds/changes/destroys)
  - `terraform apply tfplan` with auto-approve on the saved plan
  - `terraform output -json` to extract connection strings and URLs
  - `terraform destroy` for environment teardown
  - All commands receive cloud credentials as environment variables

- **`--dry-run` no longer requires Terraform installed** — generates and previews `.tf` files without needing the Terraform binary, so users can inspect before installing anything

- **`cooper destroy` now uses Terraform** — detects `.cooper/terraform/{env}/terraform.tfstate`, runs `terraform destroy`, and cleans up local state. Falls back gracefully for environments deployed with the old direct provisioner.

- **Resource cost estimates** in preview — shows per-resource `~$X/mo` for known resource types (RDS, ElastiCache, Cloud SQL, Memorystore, Azure PostgreSQL, Azure Redis)

- **Database deduplication** — multiple services referencing the same `database("main", ...)` produce a single RDS/Cloud SQL instance instead of duplicates

- **HCL builder library** (`hcl_builder.rs`) — programmatic Terraform HCL generation:
  - `TerraformResource` with builder pattern (`.attr()`, `.attr_ref()`, `.attr_block()`)
  - `TerraformVariable` with type, description, default, sensitive support
  - `TerraformOutput` with value, description, sensitive support
  - `TerraformConfig` that writes all four `.tf` files to disk
  - Proper handling of Terraform references (`${...}`), nested blocks, arrays, and string escaping

### Changed

- `cooper deploy` CLI now accepts `--service <server|serverless>` (default: `server`)
- Deploy command flow: analyze → generate Terraform → preview → interactive menu → credentials → apply (was: analyze → plan → confirm → direct CLI provisioning)
- `cooper destroy` checks for Terraform state first, falls back to old behavior for pre-Terraform deployments
- Estimated cost for the blog example dropped from ~$68/mo to ~$40/mo (more accurate per-resource estimates)

### Architecture

```
crates/cooper-deploy/src/
├── terraform/
│   ├── mod.rs              # Public API: generate() and apply()
│   ├── generator.rs        # Orchestrator: mapping → TerraformConfig
│   ├── hcl_builder.rs      # HCL types and serialization
│   ├── executor.rs         # Terraform CLI wrapper
│   └── mappings/
│       ├── mod.rs           # ResourceMapping trait + dispatcher
│       ├── aws_server.rs    # ECS Fargate
│       ├── aws_lambda.rs    # Lambda + API Gateway
│       ├── gcp_server.rs    # Cloud Run
│       ├── gcp_lambda.rs    # Cloud Functions v2
│       ├── azure_server.rs  # Container Apps
│       └── azure_lambda.rs  # Azure Functions
└── credentials.rs           # Cloud credential detection + prompting
```

## [0.5.0] - 2026-04-06

### Added

- **Dashboard UI** at localhost:9500 — live service map powered by **dagre** auto-layout:
  - **Map tab**: SVG flowchart generated from project analysis — service boxes (route counts, auth, databases, crons), inverted black pills for topics/queues, animated dashed arrows showing data flow between nodes. Layout computed automatically by dagre (LR direction, auto node spacing).
  - **Routes tab**: compact table of all API routes with method, path, handler, source, AUTH badges
  - **Explorer tab**: split-pane API explorer — pick a route, set headers/body, send request, see formatted response with status code and latency. Prompts for path params.
  - **Log tab**: live request log populated from Explorer sends — timestamp, method, path, status, duration
  - **ASCII logo** with decrypt animation: scramble → resolve left-to-right → idle glitch every 5s
  - **Live status indicator**: pulsing dot, polls `/_cooper/health` every 5s
  - Self-contained single HTML page (~10KB) served from Rust, dagre loaded from CDN
  - Port fallback: tries 9500, then 9501–9509 if taken. Never blocks the main server.
  - Monochrome, monospace, sharp corners, minimal CSS

## [0.4.0] - 2026-04-06

### Added

- **Monorepo support** — `cooper run --all` detects and runs multi-app workspaces:
  - Parses `cooper.workspace.ts` with `apps` and `shared` arrays
  - Auto-detects workspaces by scanning `apps/`, `packages/`, `services/` for `cooper.config.ts`
  - Starts shared infrastructure once (Postgres, NATS, Valkey) across all apps
  - Each app runs on its own port (base_port + index): api:5000, workers:5001, etc.
  - Runs migrations from all apps against shared database
  - Shows shared packages in startup output
  - File watcher covers the entire workspace
  - E2E verified: 2-app workspace with separate routes, queues, crons running on ports 5100/5101

- **`cooper logs`** — tail logs from deployed or local environments:
  - Local: reads embedded Postgres log from `.cooper/data/postgres/postgres.log`
  - AWS: streams from CloudWatch Logs via `aws logs tail --follow`
  - GCP: streams from Cloud Logging via `gcloud logging tail`
  - Azure: streams from Container Apps via `az containerapp logs show --follow`
  - Fly: streams via `flyctl logs`
  - Optional `--service` filter for AWS CloudWatch

- **`cooper trace`** — open trace explorer:
  - Local: opens `localhost:9400/traces` in the browser
  - AWS: opens AWS X-Ray console
  - GCP: opens Cloud Trace console
  - Azure: opens Azure Monitor
  - Fly: opens Fly.io monitoring dashboard
  - Detects Datadog/Grafana config in `cooper.config.ts` and opens the appropriate UI

- **`cooper env ls`** — lists all environments with provider, resource count, and URL from deploy state
- **`cooper env url <env>`** — prints the URL of a deployed environment

- **`cooper build`** — production build pipeline:
  - Bundles TypeScript via Bun (falls back to file concatenation)
  - Copies SDK, bridge, migrations, pages into output directory
  - Bundles island components for client-side delivery with tree-shaking
  - Generates `cooper-manifest.json` with full project analysis
  - Generates `Dockerfile` (oven/bun:1-alpine based) and `entrypoint.sh`
  - Reports output size in MB

- **WebSocket route support** — routes with `stream: "websocket"` handle WS upgrades inline in the Axum router, relay messages to/from JS handlers

- **SSE streaming module** — `SseStream` adapter for Server-Sent Events with keep-alive

- **Service-to-service client generation** — `cooper_codegen::service_clients` generates typed TS clients per service, grouped by source directory, with auto-base-URL from env vars

- **COOPER.md auto-generation** — `cooper_codegen::cooper_md` generates a markdown file with route tables, database info, topics, queues, crons, pages, and ASCII architecture diagram

- **Workspace config parser** — `cooper_codegen::workspace` parses `cooper.workspace.ts`, resolves glob patterns for shared packages, analyzes all apps

## [0.3.1] - 2026-04-06

### Added

- **Example blog app** (`examples/blog/`) — full-stack example with:
  - User CRUD with Zod validation, caching, event publishing
  - Blog posts with auth-protected create/update/delete, search, pub/sub events
  - Notification service with job queues, dead-letter queue, DLQ replay endpoint
  - Background search indexer via queue workers
  - Auth handler with JWT placeholder
  - Cron jobs (session cleanup, daily digest)
  - SSR pages (homepage, post listing, dynamic post detail)
  - Island component (LikeButton with client hydration)
  - 14 API routes, 2 databases, 2 topics, 3 queues, 2 crons, 3 pages

### Fixed

- **SDK auto-injection** — `cooper run` now injects the Cooper SDK into `node_modules/cooper` automatically, overriding any conflicting npm package. JS handlers can now `import { api } from "cooper/api"` without manual npm installation.
- **Route conflict resolution** — SSR page routes that overlap with API routes are skipped (API takes priority), preventing Axum panics on overlapping paths.

### Verified E2E

- `GET /health` → real JS handler execution returning `{ status, timestamp, uptime }`
- `GET /users` → executes Postgres query via Cooper DB client
- `POST /posts` with auth → middleware chain validates token, returns structured error when no auth handler registered
- Static analysis finds all 14 routes, 2 databases, 2 topics, 3 queues, 2 crons, 3 pages
- SSR renders full HTML documents with `<!DOCTYPE html>`, meta tags, styles
- Client codegen generates 15 typed methods (TS), 14 methods (Rust), 14 methods (Python)
- Deploy dry-run: AWS $68/mo, GCP $30/mo, Azure $43/mo, Fly $0/mo

## [0.3.0] - 2026-04-06

### Added

- **Cloud Provisioning** — real infrastructure deployment for all four cloud targets:
  - **AWS**: VPC + subnets + security groups, RDS Postgres/MySQL, ElastiCache Redis, SNS topics, SQS queues, S3 buckets, ECR repositories, ECS Fargate services. Full networking setup with internet gateway, multi-AZ subnets, and security group rules.
  - **GCP**: Cloud SQL, Memorystore Redis, Pub/Sub topics, GCS buckets, Cloud Run services. Automatic API enablement.
  - **Azure**: Resource groups, Azure PostgreSQL Flexible Server, Azure Cache for Redis, Service Bus queues, Blob Storage accounts, Container Apps with auto-scaling ingress.
  - **Fly.io**: Fly Postgres clusters, Upstash Redis, Fly Volumes for storage, Fly Machines with auto-generated `fly.toml`.
  - All providers: credential verification, deployment state persistence (`.cooper/state/`), interactive confirmation prompt with `dialoguer`

- **SSR Rendering Engine** — server-side rendering pipeline in the Rust router:
  - Page routes from `pages/` directory serve full HTML documents
  - Proper HTML5 document structure with charset, viewport, base styles
  - Island hydration script generation — supports all 5 hydration strategies: `load`, `visible` (IntersectionObserver), `idle` (requestIdleCallback), `interaction` (click/focus/mouseover), `none`
  - Island registry that scans `islands/` directory for `.island.tsx` files
  - Graceful fallback rendering when JS bridge isn't connected

- **Deploy State Management** — tracks provisioned resources per environment in `.cooper/state/{env}/deploy.json`
- **Environment Listing** — `cooper env ls` reads from deploy state
- **Interactive Deploy** — `cooper deploy` prompts for confirmation before provisioning (skippable with `--dry-run`)

### Changed

- `cooper deploy` now actually provisions cloud resources (previously only showed the plan)
- Deploy command reads project name from directory and passes it to provisioners
- All cloud providers save timestamped deployment state for destroy/status operations

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
