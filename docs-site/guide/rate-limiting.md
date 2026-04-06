# Rate Limiting

First-class rate limiting backed by Redis/Valkey. Use as global middleware or per-route.

## Per-route rate limiting

```ts
import { api } from "cooper-stack/api";
import { rateLimit } from "cooper-stack/rateLimit";

export const login = api(
  {
    method: "POST",
    path: "/auth/login",
    middleware: [rateLimit({ window: "1m", max: 5 })],
  },
  async (input) => {
    return await authenticate(input.email, input.password);
  }
);
```

This limits `/auth/login` to 5 requests per minute per IP address.

## Global rate limiting

Apply to all routes:

```ts
import { cooper } from "cooper-stack/middleware";
import { rateLimit } from "cooper-stack/rateLimit";

cooper.use(rateLimit({ window: "1m", max: 100 }));
```

## Custom key function

By default, rate limiting is keyed by IP address. Use `key` to customize:

```ts
// Rate limit by API key
rateLimit({
  window: "1m",
  max: 100,
  key: (req) => req.headers["x-api-key"] ?? "anonymous",
});

// Rate limit by authenticated user
rateLimit({
  window: "1h",
  max: 1000,
  key: (req) => req.headers["authorization"] ?? req.ip,
});
```

## Response

When the limit is exceeded, Cooper returns:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 45
Content-Type: application/json

{
  "error": {
    "code": "RATE_LIMITED",
    "message": "Rate limit exceeded. Try again in 45s.",
    "retryAfter": 45
  }
}
```

The `Retry-After` header tells clients exactly how long to wait.

## Sliding window algorithm

The rate limiter uses a sliding window based on Redis `INCR` with TTL:

1. Key format: `cooper:rl:{identifier}:{window_bucket}`
2. Each request increments the counter atomically
3. TTL is set on first increment (expires with the window)
4. When count exceeds `max`, request is rejected

This approach is simple, atomic, and accurate to the window granularity.

## Combining rate limits

Stack multiple rate limiters for tiered protection:

```ts
export const createPost = api(
  {
    method: "POST",
    path: "/posts",
    middleware: [
      rateLimit({ window: "1s", max: 2 }),    // burst protection
      rateLimit({ window: "1h", max: 100 }),   // hourly cap
    ],
  },
  async (input) => {
    return await createPost(input);
  }
);
```

## Config reference

| Option | Type | Default | Description |
|---|---|---|---|
| `window` | `string` | required | Time window (`"10s"`, `"1m"`, `"1h"`, `"1d"`) |
| `max` | `number` | required | Max requests within the window |
| `key` | `(req) => string` | IP address | Function to derive the rate limit key |
