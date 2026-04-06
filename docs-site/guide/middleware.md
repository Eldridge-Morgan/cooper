# Middleware

Composable middleware — no restrictions. Use per-route or globally.

## Define middleware

```ts
import { middleware } from "cooper-stack/middleware";
import { CooperError } from "cooper";

const rateLimiter = middleware(async (req, next) => {
  const count = await userCache.increment(`rate:${req.ip}`, { ttl: "1m" });
  if (count > 100) throw new CooperError("RATE_LIMITED");
  return next(req);
});

const logger = middleware(async (req, next) => {
  const start = Date.now();
  const res = await next(req);
  console.log(`${req.method} ${req.path} — ${Date.now() - start}ms`);
  return res;
});
```

## Per-route

```ts
export const deleteUser = api(
  { method: "DELETE", path: "/users/:id", middleware: [rateLimiter, logger] },
  async ({ id }) => { /* ... */ }
);
```

## Global

```ts
import { cooper } from "cooper-stack/middleware";

cooper.use(rateLimiter, logger);
```

Global middleware runs on every route, before per-route middleware.
