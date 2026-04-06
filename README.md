```
   ___
  / __\___   ___  _ __   ___ _ __
 / /  / _ \ / _ \| '_ \ / _ \ '__|
/ /__| (_) | (_) | |_) |  __/ |
\____/\___/ \___/| .__/ \___|_|
                  |_|
```

**The backend framework for TypeScript — powered by Rust.**

Write TypeScript. Run on Rust. Deploy anywhere. No lock-in.

---

## What is Cooper?

Cooper is a backend framework where you write services in TypeScript and everything runs on a Rust runtime (Hyper 1.x + Tokio + Axum). One command starts your entire local environment — embedded Postgres, NATS, Valkey — no Docker required. One command deploys to AWS, GCP, Azure, or Fly.io.

```ts
import { api } from "cooper/api";
import { database } from "cooper/db";

export const db = database("main", { migrations: "./migrations" });

export const getUser = api(
  { method: "GET", path: "/users/:id", auth: true },
  async ({ id }, principal) => {
    const user = await db.queryRow("SELECT * FROM users WHERE id = $1", [id]);
    if (!user) throw new CooperError("NOT_FOUND", "User not found");
    return { user };
  }
);
```

## Quick start

```bash
# Install
curl -L https://install.cooperdev.io | sh

# Create a project
cooper new my-app
cd my-app

# Run — starts Postgres, NATS, Valkey, runs migrations, hot reloads
cooper run
```

```
  ✓ Found 6 routes, 1 database, 1 cron
  ✓ Postgres on port 5432
  ✓ NATS (in-process)
  ✓ Valkey on port 6379
  ⚡ Cooper is running at http://localhost:4000
  📊 Dashboard at http://localhost:9500
```

## Features

### Core

```ts
// API routes with validation
export const createUser = api(
  { method: "POST", path: "/users", validate: CreateUserSchema },
  async (req) => { ... }
);

// Middleware — fully flexible, no restrictions
const rateLimiter = middleware(async (req, next) => { ... });

// Auth — register once, injected everywhere
export const auth = authHandler(async (token) => {
  return { userId: payload.sub, role: payload.role };
});

// Structured errors
throw new CooperError("NOT_FOUND", "User not found");  // → 404
throw new CooperError("RATE_LIMITED", "Slow down");     // → 429
```

### Data

```ts
// Database — embedded Postgres locally, managed on deploy
export const db = database("main", { migrations: "./migrations" });

// Cache — Valkey/Redis with TTL
const userCache = cache<User>("users", { ttl: "10m" });
const user = await userCache.getOrSet(id, () => db.queryRow(...));

// Pub/Sub — typed topics
export const UserCreated = topic<{ userId: string }>("user-created");
await UserCreated.publish({ userId: "u_123" });

// Queues — retries, DLQ, priority
export const EmailQueue = queue<Email>("emails", {
  retries: 3, retryDelay: "exponential", deadLetter: "email-dlq"
});

// Cron
export const cleanup = cron("cleanup", {
  schedule: "every 1 hour",
  handler: async () => { ... }
});

// Storage
const avatars = bucket("avatars", { public: true });
await avatars.upload("user/avatar.png", buffer);

// Secrets — runtime vault, never in .env
const stripeKey = secret("stripe-api-key");
```

### Deploy

```bash
# See what gets created + cost estimate
cooper deploy --env prod --cloud aws --dry-run

# + RDS Postgres (db.t3.medium)     ~$28/mo
# + ElastiCache (cache.t3.micro)    ~$12/mo
# + ECS Fargate Service             ~$0/mo
# Estimated monthly delta: +$40/mo

# Deploy for real
cooper deploy --env prod --cloud aws

# Preview environments — auto-destroy after 48h
cooper deploy --env preview-pr-42 --cloud aws --auto-destroy-after 48h
```

| Cloud | Compute | Database | Cache | Messaging |
|---|---|---|---|---|
| **AWS** | ECS Fargate | RDS | ElastiCache | SNS/SQS |
| **GCP** | Cloud Run | Cloud SQL | Memorystore | Pub/Sub |
| **Azure** | Container Apps | Azure DB | Azure Redis | Service Bus |
| **Fly.io** | Fly Machines | Fly Postgres | Upstash Redis | Upstash QStash |

### Client generation

```bash
cooper gen client --lang typescript    # typed TS client
cooper gen client --lang python        # for ML pipelines
cooper gen client --lang rust          # for other services
cooper gen openapi                     # OpenAPI 3.1
cooper gen postman                     # Postman collection
```

```ts
// Frontend — fully typed, zero manual work
import { CooperClient } from "./gen/client";
const api = new CooperClient({ baseUrl: "http://localhost:4000" });
const { user } = await api.getUser("u_123");
```

### Monorepo

```ts
// cooper.workspace.ts
export default {
  apps: ["apps/api", "apps/workers"],
  shared: ["packages/*"],
};
```

```bash
cooper run --all
# ⚡ api on port 4000  (8 routes)
# ⚡ workers on port 4001  (3 routes)
# ✓ All 2 apps running
```

### Dashboard

Live service map at `localhost:9500` — dagre-powered flowchart, API explorer, request log. Black and white, monospace, minimal.

## Project structure

```
my-app/
  services/
    users/api.ts          ← routes, handlers
    auth.ts               ← auth handler
  pages/
    index.tsx             ← SSR pages
    posts/[id].tsx
  islands/
    LikeButton.island.tsx ← client-hydrated components
  migrations/
    001_users.sql
  cooper.config.ts
```

## CLI

```bash
cooper new <name>                        # scaffold
cooper run                               # dev server + infra + hot reload
cooper run --all                         # monorepo: all apps
cooper build                             # production build + Dockerfile
cooper deploy --env <e> --cloud <c>      # provision + deploy
cooper deploy --dry-run                  # cost estimate
cooper destroy --env <e>                 # tear down
cooper gen client --lang <l>             # typed client
cooper gen openapi                       # OpenAPI 3.1
cooper db migrate | seed | shell         # database ops
cooper secrets set <n> --env <e>         # secret management
cooper logs --env <e>                    # tail logs
cooper trace --env <e>                   # trace explorer
cooper env ls                            # list environments
```

## Why Cooper over Encore?

|  | Cooper | Encore |
|---|---|---|
| Cloud targets | AWS, GCP, **Azure, Fly.io** | AWS, GCP |
| Account required | **No** | Yes (Encore Cloud) |
| Database | Postgres, **MySQL** | Postgres only |
| Queues | **Built-in** (retries, DLQ) | Via Pub/Sub |
| SSR + Islands | **Yes** | No |
| AI primitives | **Yes** | No |
| Middleware | **Fully flexible** | Type-constrained |
| Pricing | **Free (Apache-2.0)** | Freemium |

## Architecture

```
  Client
    │
    ▼
  ┌────────────────────────────────┐
  │  Cooper Runtime (Rust)         │
  │  Hyper 1.x + Tokio + Axum     │
  │  Multi-threaded HTTP           │
  │  Request validation in Rust    │
  │  Static analysis at build time │
  └───────────────┬────────────────┘
                  │
          ┌───────┼───────┐
          ▼       ▼       ▼
       [JS Worker Pool — 4 processes]
       Bun / Node / Deno
          │       │       │
          ▼       ▼       ▼
       Postgres  NATS   Valkey
```

## License

Apache-2.0
