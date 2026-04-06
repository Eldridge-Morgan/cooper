# Getting Started

## Install

```bash
# macOS / Linux
curl -L https://getcooper.dev | sh

# or Homebrew
brew install cooperdev/tap/cooper

# or Windows
winget install cooper
```

## Create a project

```bash
cooper new my-app
cd my-app
cooper run
```

This scaffolds a project with:

```
my-app/
  services/
    users/api.ts      ← CRUD with validation, cache, events
    health/api.ts      ← health check
    auth.ts            ← JWT auth handler
    cleanup.ts         ← cron job
  pages/
    index.tsx          ← SSR homepage
  migrations/
    001_users.sql      ← database schema
  shared/
    types.ts           ← shared types
  cooper.config.ts     ← project config
  package.json
  tsconfig.json
```

## What `cooper run` does

```
  → Analyzing project...
  ✓ Found 6 routes, 1 database, 1 cron
  → Starting local infrastructure...
  ✓ Postgres on port 5432
  ✓ NATS (in-process)
  ✓ Valkey on port 6379
  → Running migrations...
  ✓ Migrations up to date
  ⚡ Cooper is running at http://localhost:4000
  📊 Dashboard at http://localhost:9500
```

No Docker. No config files. No setup scripts.

## Your first route

```ts
// services/hello/api.ts
import { api } from "cooper-stack/api";

export const hello = api(
  { method: "GET", path: "/hello/:name" },
  async ({ name }: { name: string }) => {
    return { message: `Hello, ${name}!` };
  }
);
```

Save the file. Cooper hot-reloads. Hit `http://localhost:4000/hello/world`:

```json
{ "message": "Hello, world!" }
```

## Add a database

```ts
import { api } from "cooper-stack/api";
import { database } from "cooper-stack/db";

export const db = database("main", {
  migrations: "./migrations",
});

export const listUsers = api(
  { method: "GET", path: "/users" },
  async () => {
    const users = await db.query("SELECT * FROM users");
    return { users };
  }
);
```

Cooper detects the `database()` call, starts an embedded Postgres, and runs your migrations automatically.

## Generate a client

```bash
cooper gen client --lang ts
```

```ts
// In your frontend
import { CooperClient } from "./gen/client";

const api = new CooperClient({ baseUrl: "http://localhost:4000" });
const { users } = await api.listUsers();
```

## Deploy

```bash
cooper deploy --env prod --cloud aws --dry-run
```

```
+ Create: RDS Postgres (db.t3.medium)     ~$28/mo
+ Create: ElastiCache (cache.t3.micro)    ~$12/mo
+ Create: ECS Fargate Service             ~$0/mo
─────────────────────────────────────────
Estimated monthly delta: +$40/mo
```

Remove `--dry-run` to provision.
