# Project Structure

## Single app

```
my-app/
  services/           ← API services
    users/
      api.ts          ← route handlers
      events.ts       ← event subscribers
    posts/
      api.ts
    auth.ts           ← auth handler
  pages/              ← SSR pages (optional)
    index.tsx
    posts/[id].tsx
  islands/            ← client-hydrated components (optional)
    LikeButton.island.tsx
  migrations/         ← SQL migration files
    001_users.sql
    002_posts.sql
  shared/             ← shared types/utils
    types.ts
  cooper.config.ts    ← project configuration
  package.json
  tsconfig.json
```

## Monorepo

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
    shared-middleware/
  cooper.workspace.ts    ← workspace config
```

```ts
// cooper.workspace.ts
export default {
  apps: ["apps/api", "apps/workers"],
  shared: ["packages/*"],
};
```

```bash
cooper run --all        # runs all apps on sequential ports
cooper deploy --all     # deploys in dependency order
```

## Key files

| File | Purpose |
|---|---|
| `cooper.config.ts` | Project name, SSR framework, observability config |
| `cooper.workspace.ts` | Monorepo: lists apps and shared packages |
| `services/**/*.ts` | API routes, pub/sub, queues, crons |
| `pages/**/*.tsx` | SSR pages (file-based routing) |
| `islands/**/*.island.tsx` | Client-side interactive components |
| `migrations/*.sql` | Database migrations (run in order) |
