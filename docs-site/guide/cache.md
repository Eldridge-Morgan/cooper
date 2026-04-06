# Cache

In-memory cache backed by Valkey/Redis. Embedded locally, managed in production.

## Declare

```ts
import { cache } from "cooper-stack/cache";

export const userCache = cache<User>("users", { ttl: "10m" });
```

## Use

```ts
// Get or set (most common pattern)
const user = await userCache.getOrSet(userId, async () => {
  return db.queryRow("SELECT * FROM users WHERE id = $1", [userId]);
});

// Manual
await userCache.set(userId, user);
const cached = await userCache.get(userId);
await userCache.delete(userId);

// Prefix invalidation
await userCache.invalidatePrefix("users:");

// Atomic counter (for rate limiting)
const count = await userCache.increment(`rate:${ip}`, { ttl: "1m" });
```

## TTL formats

`"30s"` · `"10m"` · `"1h"` · `"7d"`
