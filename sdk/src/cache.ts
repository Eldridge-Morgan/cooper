export interface CacheConfig {
  ttl?: string; // e.g. "10m", "1h", "30s"
}

export interface CacheClient<T> {
  get(key: string): Promise<T | null>;
  set(key: string, value: T, opts?: { ttl?: string }): Promise<void>;
  getOrSet(key: string, factory: () => Promise<T>, opts?: { ttl?: string }): Promise<T>;
  delete(key: string): Promise<void>;
  invalidatePrefix(prefix: string): Promise<void>;
  increment(key: string, opts?: { ttl?: string }): Promise<number>;
}

function parseTTL(ttl: string): number {
  const match = ttl.match(/^(\d+)(s|m|h|d)$/);
  if (!match) return 600;
  const [, num, unit] = match;
  const multipliers: Record<string, number> = { s: 1, m: 60, h: 3600, d: 86400 };
  return parseInt(num) * (multipliers[unit] ?? 60);
}

/**
 * Declare a typed cache.
 *
 * ```ts
 * export const userCache = cache<User>("users", { ttl: "10m" });
 * const user = await userCache.getOrSet(userId, () => db.queryRow(...));
 * ```
 */
export function cache<T = any>(name: string, config?: CacheConfig): CacheClient<T> {
  const defaultTTL = config?.ttl ?? "10m";
  let redis: any = null;

  const ensureRedis = async () => {
    if (redis) return redis;
    const Redis = (await import("ioredis")).default;
    const url = process.env.COOPER_VALKEY_URL ?? "redis://localhost:6379";
    redis = new Redis(url);
    return redis;
  };

  const prefixed = (key: string) => `cooper:${name}:${key}`;

  return {
    async get(key: string): Promise<T | null> {
      const r = await ensureRedis();
      const val = await r.get(prefixed(key));
      return val ? JSON.parse(val) : null;
    },

    async set(key: string, value: T, opts?: { ttl?: string }): Promise<void> {
      const r = await ensureRedis();
      const ttlSec = parseTTL(opts?.ttl ?? defaultTTL);
      await r.setex(prefixed(key), ttlSec, JSON.stringify(value));
    },

    async getOrSet(key: string, factory: () => Promise<T>, opts?: { ttl?: string }): Promise<T> {
      const existing = await this.get(key);
      if (existing !== null) return existing;
      const value = await factory();
      await this.set(key, value, opts);
      return value;
    },

    async delete(key: string): Promise<void> {
      const r = await ensureRedis();
      await r.del(prefixed(key));
    },

    async invalidatePrefix(prefix: string): Promise<void> {
      const r = await ensureRedis();
      const keys = await r.keys(prefixed(prefix) + "*");
      if (keys.length > 0) await r.del(...keys);
    },

    async increment(key: string, opts?: { ttl?: string }): Promise<number> {
      const r = await ensureRedis();
      const k = prefixed(key);
      const val = await r.incr(k);
      if (val === 1 && opts?.ttl) {
        await r.expire(k, parseTTL(opts.ttl));
      }
      return val;
    },
  };
}
