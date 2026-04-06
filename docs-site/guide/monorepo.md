# Monorepo

Run multiple Cooper apps with shared infrastructure.

## Setup

```
my-platform/
  apps/
    api/
      cooper.config.ts
      services/
    workers/
      cooper.config.ts
      services/
  packages/
    shared-types/
  cooper.workspace.ts
```

```ts
// cooper.workspace.ts
export default {
  apps: ["apps/api", "apps/workers"],
  shared: ["packages/*"],
};
```

## Run all apps

```bash
cooper run --all
```

```
  ✓ 2 apps (api, workers), 1 shared packages
  ✓ Postgres on port 5432 (shared)
  ⚡ api on port 4000  (8 routes)
  ⚡ workers on port 4001  (3 routes)
  ✓ All 2 apps running
```

Each app gets its own port. Infrastructure is shared.

## Deploy

```bash
cooper deploy --all                  # deploy everything
cooper deploy --app apps/api        # deploy one app
```
