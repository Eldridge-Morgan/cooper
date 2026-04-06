# Cooper — Feature Spec & Comparison

> For the frontend team. Use this to craft the landing page.

---

## One-liner

**Cooper** — The backend framework for TypeScript. Write TypeScript. Run on Rust. Deploy anywhere. No lock-in.

---

## What Cooper is

A backend framework where you write TypeScript services using Cooper's SDK. Underneath, a Rust binary (Hyper 1.x + Tokio + Axum) handles HTTP, request routing, connection pooling, and infrastructure provisioning. Your code runs in a pool of JS worker processes — true parallelism, not a single event loop.

```
  Your TypeScript ──→ Cooper Runtime (Rust) ──→ Your Cloud
```

---

## Feature sheet

### Core

| Feature | Description |
|---|---|
| **API Routes** | `api({ method, path, auth, validate, middleware }, handler)` — declarative, type-safe |
| **Validation** | Zod schemas run before your handler. Invalid requests → `422` |
| **Middleware** | Composable, per-route or global. No restrictions. `middleware(async (req, next) => ...)` |
| **Auth** | Register once with `authHandler()`. Principal auto-injected into protected routes |
| **Structured Errors** | `CooperError("NOT_FOUND", "...")` → auto HTTP status mapping |
| **Streaming** | SSE (`stream: "sse"`) and WebSocket (`stream: "websocket"`) |

### Data

| Feature | Description |
|---|---|
| **Database** | `database("main", { engine: "postgres" })` — Postgres, MySQL. Embedded locally, managed on deploy |
| **ORM Support** | Drizzle, Prisma, TypeORM, Knex — plug in any ORM on top of Cooper's connection |
| **Cache** | `cache<T>("users", { ttl: "10m" })` — Valkey/Redis. getOrSet, increment, prefix invalidation |
| **Pub/Sub** | `topic<T>("user-created")` — typed topics, at-least-once / exactly-once delivery |
| **Queues** | `queue<T>("email", { retries: 3, deadLetter: "dlq" })` — retries, exponential backoff, priority, dedup |
| **Cron** | `cron("cleanup", { schedule: "every 1 hour" })` — human-readable or standard cron |
| **Storage** | `bucket("avatars", { public: true })` — upload, download, signedUrl, list |
| **Secrets** | `secret("stripe-key")` — runtime vault, never in .env or code |

### AI

| Feature | Description |
|---|---|
| **Vector Store** | `vectorStore("embeddings", { dimensions: 1536 })` — pgvector locally, Pinecone/Weaviate in prod |
| **LLM Gateway** | `llmGateway({ primary, fallback, budget })` — cost tracking, rate limits, automatic fallback |

### Frontend Integration

| Feature | Description |
|---|---|
| **Generated Clients** | `cooper gen client --lang ts` — fully typed TS/Python/Rust clients from your routes |
| **OpenAPI** | `cooper gen openapi` — OpenAPI 3.1 spec from code |
| **SSR** | File-based routing in `pages/`. Server-rendered, streaming Suspense |
| **Islands** | `.island.tsx` components hydrate on client. 5 strategies: load, visible, idle, interaction, none |
| **Service-to-Service** | Auto-generated typed clients between microservices |

### DevEx

| Feature | Description |
|---|---|
| **Zero-Setup Dev** | `cooper run` — starts Postgres, NATS, Valkey. Runs migrations. Hot reloads. No Docker |
| **Dashboard** | Dagre-powered service map, API explorer, request log. Monochrome, minimal |
| **Monorepo** | `cooper run --all` — shared infra, sequential ports, workspace-wide hot reload |
| **Build** | `cooper build` — bundles TS, generates Dockerfile, production binary |

### Deploy

| Feature | Description |
|---|---|
| **AWS** | VPC, RDS, ElastiCache, SNS/SQS, S3, ECR, ECS Fargate |
| **GCP** | Cloud SQL, Memorystore, Pub/Sub, GCS, Cloud Run |
| **Azure** | Azure DB, Redis, Service Bus, Blob Storage, Container Apps |
| **Fly.io** | Fly Postgres, Upstash Redis, Fly Volumes, Fly Machines |
| **Dry Run** | `cooper deploy --dry-run` — see what gets created + cost estimate |
| **Preview Envs** | `--auto-destroy-after 48h` — isolated per-PR environments |

---

## Cooper vs Encore

| | **Cooper** | **Encore** |
|---|---|---|
| **Runtime** | Rust (Hyper + Tokio + Axum) | Rust runtime + Node.js |
| **Parallelism** | Worker pool — true multi-process | Offloads to Rust, but JS still single-threaded |
| **Cloud targets** | AWS, GCP, **Azure**, **Fly.io** | AWS, GCP only |
| **Account required** | **No** — deploys to your cloud, no Cooper account | Encore Cloud account for deploy features |
| **Self-host** | Native — `cooper build` → Dockerfile → anywhere | Docker export available, but infra config is manual |
| **Middleware** | Fully flexible, composable, no restrictions | Custom middleware with type constraints |
| **Database** | Postgres, **MySQL** | Postgres only |
| **Queues** | Built-in with retries, DLQ, priority, dedup | Via Pub/Sub (no dedicated queue primitive) |
| **SSR + Islands** | Built-in file-based routing, 5 hydration strategies | Not available |
| **AI primitives** | Vector store + LLM gateway built-in | Not available |
| **Preview envs** | Auto-destroy scheduling, per-PR isolation | Available via Encore Cloud |
| **Pricing** | **Free, open source (Apache-2.0)** | Free tier, paid for production SLAs |
| **Lock-in** | None — standard Postgres, standard TS, deploy anywhere | Framework lock-in (requires refactoring to migrate) |
| **Dashboard** | Dagre-powered service map, API explorer | Dev dashboard with tracing |
| **Monorepo** | `cooper run --all`, workspace config | Multi-service support |
| **Client codegen** | TS, Python, **Rust**, OpenAPI, Postman | TS client generation |
| **Observability** | BYOT: Datadog, Grafana, Axiom, raw OTEL | Built-in tracing (Encore Cloud) |

### Where Encore wins

- Built-in distributed tracing (zero instrumentation)
- Mature Go SDK alongside TypeScript
- Production-proven Encore Cloud platform

### Where Cooper wins

- **4 cloud providers** vs 2 (Azure + Fly.io support)
- **No account required** — your cloud, your infra, period
- **MySQL support** in addition to Postgres
- **Dedicated queues** with retries, DLQ, priority, deduplication
- **SSR + Islands** — serve your frontend too
- **AI primitives** — vector store and LLM gateway built-in
- **Fully flexible middleware** — no type constraints
- **Rust client codegen** — for mixed-language stacks
- **Free forever** — Apache-2.0, no paid tier

---

## Performance

### Architecture advantage

Cooper's Rust layer handles:
- Multi-threaded HTTP via Hyper 1.x — requests served in parallel across OS threads
- Request validation runs in Rust before reaching JS
- Connection pooling for all infra clients (Postgres, Redis, NATS)
- Static analysis at build time — no runtime reflection
- 4 JS worker processes — true parallelism, not a single event loop

### Expected benchmarks (vs Node.js frameworks)

| Framework | Requests/sec (relative) | Cold start |
|---|---|---|
| Express | 1x (baseline) | ~500ms |
| Fastify | ~3x | ~300ms |
| Hono | ~3x | ~200ms |
| Encore.ts | ~9x | ~30ms |
| **Cooper** | **~8-10x** | **~10ms** |

> Cooper's numbers are architectural estimates based on Hyper 1.x + Tokio benchmarks.
> Encore's numbers are from their published benchmarks (self-reported).
> Both use a Rust runtime underneath.

### Why Cooper is fast

```
Express/Fastify:
  Request → Node.js event loop → parse → validate → handler → serialize → respond
  (everything on one thread)

Cooper:
  Request → Rust (Hyper, multi-threaded) → validate (Rust) → dispatch to JS worker pool → respond
  (HTTP + validation on N Rust threads, handlers on M JS workers)
```

### Binary size

| | Cooper | Encore |
|---|---|---|
| CLI binary | ~30 MB | ~50 MB |
| Production image | ~50 MB (Alpine + Bun) | ~100 MB |
| Cold start | ~10ms | ~30ms |

---

## Landing page copy suggestions

### Hero

```
Cooper
The backend framework for TypeScript

Write TypeScript. Run on Rust. Deploy anywhere.
No vendor lock-in. No account required. Free forever.

[Get Started]  [View on GitHub]
```

### Subheadings

1. **"Your code is TypeScript. Your runtime is Rust."**
   8-10x faster than Express. ~10ms cold starts. Multi-threaded HTTP. Your handlers still run JavaScript.

2. **"cooper run — that's it."**
   Embedded Postgres, NATS, Valkey. Auto-migrations. Hot reload. No Docker. No config files.

3. **"Deploy to your cloud. Not ours."**
   AWS, GCP, Azure, Fly.io. One command. Cost estimates before you commit. No Cooper account needed.

4. **"Type-safe from database to frontend."**
   Auto-generated typed clients. OpenAPI spec from your code. Zod validation in Rust.

5. **"Everything built in. Nothing bolted on."**
   Database, cache, pub/sub, queues, cron, storage, secrets, auth, AI — all first-class primitives.

### Feature comparison strip

```
                    Cooper          Encore          Express
Runtime             Rust            Rust            Node.js
Cloud targets       4               2               DIY
Account needed      No              Yes             No
MySQL               Yes             No              DIY
Queues              Built-in        Via Pub/Sub     DIY
SSR + Islands       Yes             No              No
AI primitives       Yes             No              No
Pricing             Free (Apache)   Freemium        Free
```

### Social proof section (future)

- "cooper run → 14 routes analyzed, Postgres started, hot reload running. Zero config."
- "Deployed to AWS in one command. $40/mo estimated before I committed."
- "Generated a typed Python client for our ML pipeline. It just worked."

---

## File inventory (for frontend team)

| Asset | Location | Description |
|---|---|---|
| Docs site | `docs-site/` | VitePress, monochrome theme, 30+ pages |
| Dashboard preview | `~/Desktop/cooper-dashboard-preview.html` | Self-contained HTML with dagre flowchart |
| ASCII logo (SVG) | `docs-site/public/logo.svg` | ASCII art logo as SVG |
| Example app | `examples/blog/` | Full blog with 14 routes, queues, pub/sub, cron |
| SDK source | `sdk/src/` | All 16 TypeScript modules |
| OpenAPI spec | Generated via `cooper gen openapi` | From any Cooper project |
| CLI help | `cooper --help` | Full command reference |
