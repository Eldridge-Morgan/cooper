import { CooperError } from "./error.js";
import type { MiddlewareFn } from "./registry.js";

export interface RateLimitConfig {
  /** Time window — e.g. "1m", "10s", "1h" */
  window: string;
  /** Max requests allowed within the window */
  max: number;
  /** Function to derive the rate limit key from the request. Defaults to IP address. */
  key?: (req: any) => string;
}

function parseTTL(ttl: string): number {
  const match = ttl.match(/^(\d+)(s|m|h|d)$/);
  if (!match) return 60;
  const [, num, unit] = match;
  const multipliers: Record<string, number> = { s: 1, m: 60, h: 3600, d: 86400 };
  return parseInt(num) * (multipliers[unit] ?? 60);
}

function parseWindowMs(window: string): number {
  return parseTTL(window) * 1000;
}

let redis: any = null;

async function ensureRedis(): Promise<any> {
  if (redis) return redis;
  const Redis = (await import("ioredis")).default;
  const url = process.env.COOPER_VALKEY_URL ?? "redis://localhost:6379";
  redis = new Redis(url);
  return redis;
}

/**
 * Create a rate limiting middleware.
 *
 * Can be used globally:
 * ```ts
 * cooper.use(rateLimit({ window: "1m", max: 100 }));
 * ```
 *
 * Or per-route via middleware config:
 * ```ts
 * export default api({
 *   method: "POST",
 *   path: "/api/login",
 *   middleware: [rateLimit({ window: "1m", max: 5 })],
 *   handler: async (input) => { ... }
 * });
 * ```
 */
export function rateLimit(config: RateLimitConfig): MiddlewareFn {
  const { window, max, key: keyFn } = config;
  const ttlSec = parseTTL(window);
  const windowMs = parseWindowMs(window);

  return async (req: any, next: (req: any) => Promise<any>) => {
    const identifier = keyFn ? keyFn(req) : (req.ip ?? "unknown");
    const redisKey = `cooper:rl:${identifier}:${Math.floor(Date.now() / windowMs)}`;

    const r = await ensureRedis();
    const count = await r.incr(redisKey);

    // Set TTL on first increment
    if (count === 1) {
      await r.expire(redisKey, ttlSec);
    }

    if (count > max) {
      // Calculate retry-after in seconds
      const ttl = await r.ttl(redisKey);
      const retryAfter = ttl > 0 ? ttl : ttlSec;

      const error = new CooperError("RATE_LIMITED", `Rate limit exceeded. Try again in ${retryAfter}s.`);
      error.retryAfter = retryAfter;
      throw error;
    }

    // Attach rate limit headers info to request for downstream use
    req._rateLimit = {
      limit: max,
      remaining: max - count,
      reset: Math.ceil(Date.now() / 1000) + ttlSec,
    };

    return next(req);
  };
}
